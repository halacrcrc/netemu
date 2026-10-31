//! 事件驱动逐包仿真引擎。
//!
//! - 虚拟时钟以纳秒推进；所有事件（报文到达、ping 发送/超时）进入
//!   以 (time, seq) 排序的最小堆，顺序确定。
//! - 发送一个帧 = 计算串行化时延（按带宽）+ 链路传播时延，排定到达事件。
//! - 交换机按 VLAN 转发：入方向归类、按 (VLAN, MAC) 学习、出方向按口型
//!   加/剥 802.1Q tag；SVI 作为三层接口参与 ARP/ICMP/路由。
//! - 引擎在每次 CLI 命令后运行至"静默"（队列清空或到达视野上限），
//!   因此 ping 这类跨多个事件的交互对用户表现为同步输出。

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::device::*;
use crate::net::Net;
use sim_packets::*;
use sim_packets::{BpduPacket, BridgeId};

/// 每条链路固定传播时延。
pub const PROP_DELAY_NS: u64 = 10_000;
/// 每跳设备入方向处理时延（近似真实设备转发时延量级）。
pub const INGRESS_DELAY_NS: u64 = 50_000;
/// 在途报文上限（性能护栏）。
pub const MAX_IN_FLIGHT: usize = 20_000;
/// 默认静默视野：每次驱动最多向前看 12 秒虚拟时间。
pub const PING_HORIZON_NS: u64 = 12_000_000_000;
/// STP 活跃时的静默视野（覆盖一次完整收敛：2×15s 转发时延 + 余量）。
pub const STP_HORIZON_NS: u64 = 60_000_000_000;
/// 单次驱动的最大事件数（广播风暴等失控场景的护栏）。
pub const MAX_EVENTS_PER_RUN: u64 = 200_000;
/// STP 定时器常量。
pub const STP_HELLO_NS: u64 = 2_000_000_000;
pub const STP_FORWARD_DELAY_NS: u64 = 15_000_000_000;
/// BPDU 老化时间（802.1D Max Age）：超时未收到邻居 BPDU 则淘汰缓存并重算。
pub const STP_MAX_AGE_NS: u64 = 20_000_000_000;

fn mac_is_broadcast(m: &Mac) -> bool {
    *m == BROADCAST_MAC
}

fn mac_is_multicast(m: &Mac) -> bool {
    m[0] & 1 == 1
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub virtual_ns: u64,
    pub events: u64,
    pub in_flight: usize,
    pub dropped: u64,
}

enum EKind {
    Arrive {
        to_dev: DevId,
        to_port: PortId,
        frame: EthernetFrame,
    },
    PingSend {
        dev: DevId,
        sid: u16,
        probe: u32,
    },
    PingTimeout {
        dev: DevId,
        sid: u16,
        probe: u32,
    },
    /// STP 周期性 hello（指派端口发 BPDU）
    StpHello {
        dev: DevId,
    },
    /// STP 端口状态按 forward delay 推进（带纪元防串扰）
    StpAdvance {
        dev: DevId,
        port: PortId,
        to: StpSt,
        epoch: u32,
    },
}

struct QEvent {
    time: u64,
    seq: u64,
    kind: EKind,
}

impl PartialEq for QEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.seq == other.seq
    }
}
impl Eq for QEvent {}
impl PartialOrd for QEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.time, self.seq).cmp(&(other.time, other.seq))
    }
}

enum PingRoute {
    /// 主机：单口直发（同网段 ARP 对端，否则 ARP 网关）
    Host {
        arp_for: Ipv4Addr,
    },
    /// 路由器：出物理口 + 下一跳
    Router {
        port: PortId,
        arp_for: Ipv4Addr,
    },
    /// 三层交换机：从 SVI 发出
    Switch {
        vlan: VlanId,
        arp_for: Ipv4Addr,
    },
    NoRoute,
}

pub struct Engine {
    pub net: Net,
    queue: BinaryHeap<Reverse<QEvent>>,
    next_seq: u64,
    now: u64,
    in_flight: usize,
    pub dropped: u64,
    pub events_processed: u64,
}

impl Engine {
    pub fn new(net: Net) -> Self {
        let mut eng = Self {
            net,
            queue: BinaryHeap::new(),
            next_seq: 0,
            now: 0,
            in_flight: 0,
            dropped: 0,
            events_processed: 0,
        };
        // 启动时初始化 STP：全量重算 + 调度 hello
        eng.stp_recompute_all();
        eng
    }

    pub fn now_ns(&self) -> u64 {
        self.now
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight
    }

    pub fn stats(&self) -> Stats {
        Stats {
            virtual_ns: self.now,
            events: self.events_processed,
            in_flight: self.in_flight,
            dropped: self.dropped,
        }
    }

    fn push(&mut self, time: u64, kind: EKind) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.queue.push(Reverse(QEvent { time, seq, kind }));
    }

    fn serialize_ns(size_bytes: u32, speed_bps: u64) -> u64 {
        (size_bytes as u64 * 8) * 1_000_000_000 / speed_bps.max(1)
    }

    /// 从某设备某端口发出一个帧。端口 down / 无链路 / 超限则丢弃。
    pub fn transmit_frame(&mut self, dev: DevId, port: PortId, frame: EthernetFrame) {
        let Some(d) = self.net.device(dev) else {
            return;
        };
        let Some(p) = d.port(port) else {
            return;
        };
        let Some(link_id) = p.link else {
            self.dropped += 1;
            return;
        };
        if !p.admin_up || !self.net.link_up(link_id) {
            self.dropped += 1;
            return;
        }
        if self.in_flight >= MAX_IN_FLIGHT {
            self.dropped += 1;
            return;
        }
        let (speed_bps, dir, to, capture_on) = {
            let Some(link) = self.net.link(link_id).cloned() else {
                return;
            };
            let dir = if link.a.0 == dev { 0u8 } else { 1u8 };
            let to = if link.a.0 == dev { link.b } else { link.a };
            // 对端端口管理性关闭 → 链路不通（近似真实的双端载波检测）
            let peer_up = self
                .net
                .device(to.0)
                .and_then(|d| d.port(to.1))
                .map(|p| p.admin_up)
                .unwrap_or(false);
            if !peer_up {
                self.dropped += 1;
                return;
            }
            (p.speed_bps, dir, to, link.capture)
        };
        if capture_on {
            self.net.record_capture(link_id, dir, &frame, self.now);
        }
        let arrival = self.now + Self::serialize_ns(frame.wire_size(), speed_bps) + PROP_DELAY_NS;
        self.in_flight += 1;
        self.push(
            arrival,
            EKind::Arrive {
                to_dev: to.0,
                to_port: to.1,
                frame,
            },
        );
    }

    /// 运行至静默（队列空）或超出视野/事件数上限。返回统计。
    pub fn run_to_quiescence(&mut self, horizon_ahead_ns: u64) -> Stats {
        self.run_capped(horizon_ahead_ns, MAX_EVENTS_PER_RUN)
    }

    pub fn run_capped(&mut self, horizon_ahead_ns: u64, max_events: u64) -> Stats {
        let deadline = self.now + horizon_ahead_ns;
        let mut processed: u64 = 0;
        while let Some(Reverse(ev)) = self.queue.peek() {
            if ev.time > deadline || processed >= max_events {
                break;
            }
            let Reverse(ev) = self.queue.pop().unwrap();
            self.now = ev.time;
            self.events_processed += 1;
            processed += 1;
            match ev.kind {
                EKind::Arrive {
                    to_dev,
                    to_port,
                    frame,
                } => {
                    self.in_flight -= 1;
                    self.deliver(to_dev, to_port, frame);
                }
                EKind::PingSend { dev, sid, probe } => self.on_ping_send(dev, sid, probe),
                EKind::PingTimeout { dev, sid, probe } => self.on_ping_timeout(dev, sid, probe),
                EKind::StpHello { dev } => self.on_stp_hello(dev),
                EKind::StpAdvance {
                    dev,
                    port,
                    to,
                    epoch,
                } => self.on_stp_advance(dev, port, to, epoch),
            }
        }
        self.stats()
    }

    /// 按当前网络状态给出合适的静默视野（STP 活跃时需覆盖完整收敛）。
    pub fn drain_horizon(&self) -> u64 {
        let stp_on = self.net.devices.iter().any(|d| match &d.state {
            DeviceState::Switch(sw) => sw.stp.enabled,
            _ => false,
        });
        if stp_on {
            STP_HORIZON_NS
        } else {
            PING_HORIZON_NS
        }
    }

    // ---------- 报文到达分发 ----------

    fn deliver(&mut self, dev_id: DevId, port: PortId, frame: EthernetFrame) {
        let Some(d) = self.net.device(dev_id) else {
            return;
        };
        match d.kind {
            DeviceKind::Switch => self.switch_ingress(dev_id, port, frame),
            _ => self.l3_ingress(dev_id, port, frame),
        }
    }

    /// 端口能否转发数据帧（STP 启用时仅 Forwarding 状态可转发）。
    fn can_forward(&self, dev_id: DevId, port: PortId) -> bool {
        let Some(d) = self.net.device(dev_id) else {
            return false;
        };
        match &d.state {
            DeviceState::Switch(sw) => {
                !sw.stp.enabled || sw.stp.state_of(port) == StpSt::Forwarding
            }
            _ => true,
        }
    }

    fn stp_on(&self, dev_id: DevId) -> bool {
        matches!(
            self.net.device(dev_id).map(|d| &d.state),
            Some(DeviceState::Switch(sw)) if sw.stp.enabled
        )
    }

    /// 三层交换机入方向：BPDU 截获 → VLAN 归类 → 学习 → SVI 截获 → 转发/洪泛。
    fn switch_ingress(&mut self, dev_id: DevId, in_port: PortId, frame: EthernetFrame) {
        // 0. BPDU：先于 VLAN/端口状态处理（STP 与 VLAN 无关，阻塞端口照常收）
        if frame.ethertype == ETHERTYPE_BPDU {
            if let L3::Bpdu(bpdu) = frame.payload {
                if self.stp_on(dev_id) {
                    self.stp_on_bpdu(dev_id, in_port, bpdu);
                } else {
                    // STP 关闭：BPDU 当作未知组播洪泛（广播风暴演示的一部分）
                    let n = self.net.device(dev_id).map(|d| d.ports.len()).unwrap_or(0);
                    for p in 0..n {
                        if p != in_port {
                            self.transmit_frame(dev_id, p, frame);
                        }
                    }
                }
            }
            return;
        }
        // STP 启用时，非 Forwarding 端口丢弃数据帧（不学习）
        if self.stp_on(dev_id) && !self.can_forward(dev_id, in_port) {
            self.dropped += 1;
            return;
        }
        // 1. 入方向 VLAN 归类
        let vlan = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            let Some(p) = d.port(in_port) else {
                return;
            };
            match p.classify_ingress(frame.vlan) {
                Some(v) => v,
                None => {
                    self.dropped += 1; // tag 未放行 / 不匹配
                    return;
                }
            }
        };
        // 2. 学习 + 查表
        let (dst_port, svi_ip, port_count) = {
            let Some(d) = self.net.device_mut(dev_id) else {
                return;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return;
            };
            sw.mac_table.insert((vlan, frame.src), in_port);
            let dst_port = if mac_is_broadcast(&frame.dst) || mac_is_multicast(&frame.dst) {
                None
            } else {
                sw.mac_table.get(&(vlan, frame.dst)).copied()
            };
            let svi_ip = sw.svi.get(&vlan).filter(|s| s.admin_up).map(|s| s.ip);
            (dst_port, svi_ip, d.ports.len())
        };
        let to_svi = svi_ip.is_some() && Some(svi_mac(vlan)) == Some(frame.dst);
        let broadcast_with_svi = mac_is_broadcast(&frame.dst) && svi_ip.is_some();
        // 3. 本机（SVI）处理
        if to_svi || broadcast_with_svi {
            self.now += INGRESS_DELAY_NS;
            self.switch_l3_ingress(dev_id, in_port, vlan, frame);
            if to_svi {
                return; // 目的是 SVI 的单播不再转发
            }
        }
        // 4. 二层转发 / 洪泛（出方向过滤非 Forwarding 端口——STP 阻塞的关键）
        let out_ports: Vec<PortId> = match dst_port {
            Some(p) if p != in_port && self.can_forward(dev_id, p) => vec![p],
            Some(_) => vec![], // 已知单播但源同端口或不可转发：丢弃
            None => (0..port_count)
                .filter(|&p| p != in_port && self.can_forward(dev_id, p))
                .collect(),
        };
        for p in out_ports {
            self.l2_egress(dev_id, p, vlan, frame);
        }
    }

    /// 出方向：按端口口型决定加/剥 tag 后发送；不发则静默跳过。
    fn l2_egress(&mut self, dev_id: DevId, port: PortId, vlan: VlanId, mut frame: EthernetFrame) {
        let Some(d) = self.net.device(dev_id) else {
            return;
        };
        let Some(p) = d.port(port) else {
            return;
        };
        if let Some(tag) = p.egress_tag(vlan) {
            frame.vlan = tag;
            self.transmit_frame(dev_id, port, frame);
        }
    }

    /// 在指定 VLAN 内把帧送达已知 MAC（查表或洪泛）。
    fn l2_send_in_vlan(&mut self, dev_id: DevId, vlan: VlanId, dst: Mac, frame: EthernetFrame) {
        let (learned, port_count) = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            (sw.mac_table.get(&(vlan, dst)).copied(), d.ports.len())
        };
        match learned {
            Some(p) => self.l2_egress(dev_id, p, vlan, frame),
            None => {
                for p in 0..port_count {
                    if self.can_forward(dev_id, p) {
                        self.l2_egress(dev_id, p, vlan, frame);
                    }
                }
            }
        }
    }

    fn flood_in_vlan(&mut self, dev_id: DevId, vlan: VlanId, frame: EthernetFrame) {
        let n = self.net.device(dev_id).map(|d| d.ports.len()).unwrap_or(0);
        for p in 0..n {
            if self.can_forward(dev_id, p) {
                self.l2_egress(dev_id, p, vlan, frame);
            }
        }
    }

    // ---------- 三层交换机（SVI） ----------

    fn switch_l3_ingress(
        &mut self,
        dev_id: DevId,
        in_port: PortId,
        vlan: VlanId,
        frame: EthernetFrame,
    ) {
        let my_ip = self.net.device(dev_id).and_then(|d| match &d.state {
            DeviceState::Switch(sw) => sw.svi.get(&vlan).filter(|s| s.admin_up).map(|s| s.ip),
            _ => None,
        });
        let Some(my_ip) = my_ip else {
            return;
        };
        let my_mac = svi_mac(vlan);
        match frame.payload {
            L3::Arp(arp) => {
                if arp.sender_ip != 0 {
                    self.arp_learn(dev_id, in_port, Some(vlan), arp.sender_ip, arp.sender_mac);
                    self.flush_pending(dev_id, in_port, Some(vlan), arp.sender_ip, arp.sender_mac);
                }
                if arp.op == ArpOp::Request && arp.target_ip == my_ip {
                    let reply = ArpPacket {
                        op: ArpOp::Reply,
                        sender_mac: my_mac,
                        sender_ip: my_ip,
                        target_mac: arp.sender_mac,
                        target_ip: arp.sender_ip,
                    };
                    let out = EthernetFrame {
                        dst: arp.sender_mac,
                        src: my_mac,
                        vlan: None,
                        ethertype: ETHERTYPE_ARP,
                        payload: L3::Arp(reply),
                    };
                    self.l2_egress(dev_id, in_port, vlan, out);
                }
            }
            L3::Ipv4(pkt) => {
                if Some(pkt.dst) == Some(my_ip) {
                    if pkt.icmp.is_reply {
                        self.on_ping_reply(dev_id, pkt);
                    } else {
                        let reply = Ipv4Packet {
                            src: pkt.dst,
                            dst: pkt.src,
                            ttl: DEFAULT_TTL,
                            icmp: IcmpPacket {
                                id: pkt.icmp.id,
                                seq: pkt.icmp.seq,
                                is_reply: true,
                            },
                        };
                        let out = EthernetFrame {
                            dst: frame.src,
                            src: my_mac,
                            vlan: None,
                            ethertype: ETHERTYPE_IPV4,
                            payload: L3::Ipv4(reply),
                        };
                        self.l2_egress(dev_id, in_port, vlan, out);
                    }
                } else {
                    self.switch_l3_forward(dev_id, pkt);
                }
            }
            L3::Bpdu(_) => {} // BPDU 在二层路径已处理，防御性忽略
            L3::Dhcp(_) => {} // 交换机 SVI 不参与 DHCP（M2b）
        }
    }

    fn switch_l3_forward(&mut self, dev_id: DevId, mut pkt: Ipv4Packet) {
        if pkt.ttl <= 1 {
            self.dropped += 1;
            return;
        }
        pkt.ttl -= 1;
        let Some((_, nexthop, Some(out_vlan))) = self.net.lookup_route(dev_id, pkt.dst) else {
            self.dropped += 1;
            return;
        };
        self.switch_send_ip(dev_id, out_vlan, pkt, nexthop);
    }

    /// 三层交换机从 SVI 发送 IPv4（含 VLAN 内 ARP 解析）。
    fn switch_send_ip(&mut self, dev_id: DevId, vlan: VlanId, pkt: Ipv4Packet, arp_for: Ipv4Addr) {
        let src_mac = svi_mac(vlan);
        let entry = self.net.device(dev_id).and_then(|d| match &d.state {
            DeviceState::Switch(sw) => sw.arp.get(&(vlan, arp_for)).copied(),
            _ => None,
        });
        if let Some(e) = entry {
            let frame = EthernetFrame {
                dst: e.mac,
                src: src_mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV4,
                payload: L3::Ipv4(pkt),
            };
            self.l2_send_in_vlan(dev_id, vlan, e.mac, frame);
            return;
        }
        let svi_ip = self.net.device(dev_id).and_then(|d| match &d.state {
            DeviceState::Switch(sw) => sw.svi.get(&vlan).map(|s| s.ip),
            _ => None,
        });
        let need_request = {
            let Some(d) = self.net.device_mut(dev_id) else {
                return;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return;
            };
            sw.pending.push(PendingFrame {
                port: 0,
                vlan: Some(vlan),
                dst_ip: arp_for,
                src_mac,
                pkt,
            });
            sw.arping.insert((vlan, arp_for))
        };
        if need_request {
            let req = ArpPacket {
                op: ArpOp::Request,
                sender_mac: src_mac,
                sender_ip: svi_ip.unwrap_or(0),
                target_mac: ZERO_MAC,
                target_ip: arp_for,
            };
            let bcast = EthernetFrame {
                dst: BROADCAST_MAC,
                src: src_mac,
                vlan: None,
                ethertype: ETHERTYPE_ARP,
                payload: L3::Arp(req),
            };
            self.flood_in_vlan(dev_id, vlan, bcast);
        }
    }

    // ---------- 路由器 / 主机 ----------

    /// 路由器/主机：仅处理目的 MAC 为本机端口 MAC 或广播的 untagged 帧。
    fn l3_ingress(&mut self, dev_id: DevId, port: PortId, frame: EthernetFrame) {
        let (my_mac, my_ip) = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            let Some(p) = d.port(port) else {
                return;
            };
            (p.mac, p.ip)
        };
        if frame.dst != my_mac && frame.dst != BROADCAST_MAC {
            return; // 非本机帧，L3 设备不透明转发（无桥接）
        }
        if frame.vlan.is_some() {
            // M1：物理口不收 tagged 帧（dot1q 子接口属 M2）
            self.dropped += 1;
            return;
        }
        self.now += INGRESS_DELAY_NS;
        match frame.payload {
            L3::Arp(arp) => self.arp_ingress(dev_id, port, my_mac, my_ip, arp),
            L3::Ipv4(pkt) => self.ipv4_ingress(dev_id, port, my_mac, frame, pkt),
            L3::Bpdu(_) => {} // 主机/路由器不处理 802.1D BPDU
            L3::Dhcp(dhcp) => {
                // DHCP 广播：主机客户端 / 路由器服务器各自处理
                match d_kind(&self.net, dev_id) {
                    DeviceKind::Host => self.dhcp_client_ingress(dev_id, port, dhcp),
                    DeviceKind::Router => self.dhcp_server_ingress(dev_id, port, dhcp),
                    DeviceKind::Switch => {}
                }
            }
        }
    }

    fn arp_ingress(
        &mut self,
        dev_id: DevId,
        port: PortId,
        my_mac: Mac,
        my_ip: Option<Ipv4Addr>,
        arp: ArpPacket,
    ) {
        // 学习发送方（请求/应答都学），并放出等待中的报文
        if arp.sender_ip != 0 {
            self.arp_learn(dev_id, port, None, arp.sender_ip, arp.sender_mac);
            self.flush_pending(dev_id, port, None, arp.sender_ip, arp.sender_mac);
        }
        if arp.op == ArpOp::Request && my_ip == Some(arp.target_ip) {
            let reply = ArpPacket {
                op: ArpOp::Reply,
                sender_mac: my_mac,
                sender_ip: arp.target_ip,
                target_mac: arp.sender_mac,
                target_ip: arp.sender_ip,
            };
            let frame = EthernetFrame {
                dst: arp.sender_mac,
                src: my_mac,
                vlan: None,
                ethertype: ETHERTYPE_ARP,
                payload: L3::Arp(reply),
            };
            self.transmit_frame(dev_id, port, frame);
        }
    }

    fn arp_learn(
        &mut self,
        dev_id: DevId,
        port: PortId,
        vlan: Option<VlanId>,
        ip: Ipv4Addr,
        mac: Mac,
    ) {
        let Some(d) = self.net.device_mut(dev_id) else {
            return;
        };
        match &mut d.state {
            DeviceState::Router(rs) => {
                rs.arp.insert((port, vlan, ip), ArpEntry { mac, port });
                rs.arping.remove(&(port, vlan, ip));
            }
            DeviceState::Host(hs) => {
                hs.arp.insert(ip, ArpEntry { mac, port });
                hs.arping.remove(&(port, ip));
            }
            DeviceState::Switch(sw) => {
                if let Some(v) = vlan {
                    sw.arp.insert((v, ip), ArpEntry { mac, port });
                    sw.arping.remove(&(v, ip));
                }
            }
        }
    }

    /// ARP 已解析：把等待中的报文真正发出去。
    fn flush_pending(
        &mut self,
        dev_id: DevId,
        port: PortId,
        vlan: Option<VlanId>,
        ip: Ipv4Addr,
        mac: Mac,
    ) {
        let ready: Vec<PendingFrame> = {
            let Some(d) = self.net.device_mut(dev_id) else {
                return;
            };
            match &mut d.state {
                DeviceState::Router(rs) => {
                    let (ok, rest): (Vec<_>, Vec<_>) = rs
                        .pending
                        .drain(..)
                        .partition(|p| p.port == port && p.vlan == vlan && p.dst_ip == ip);
                    rs.pending = rest;
                    ok
                }
                DeviceState::Host(hs) => {
                    let (ok, rest): (Vec<_>, Vec<_>) =
                        hs.pending.drain(..).partition(|p| p.dst_ip == ip);
                    hs.pending = rest;
                    ok
                }
                DeviceState::Switch(sw) => {
                    let (ok, rest): (Vec<_>, Vec<_>) = sw
                        .pending
                        .drain(..)
                        .partition(|p| p.vlan == vlan && p.dst_ip == ip);
                    sw.pending = rest;
                    ok
                }
            }
        };
        for p in ready {
            let frame = EthernetFrame {
                dst: mac,
                src: p.src_mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV4,
                payload: L3::Ipv4(p.pkt),
            };
            match d_kind(&self.net, dev_id) {
                DeviceKind::Switch => {
                    if let Some(v) = vlan {
                        self.l2_send_in_vlan(dev_id, v, mac, frame);
                    }
                }
                _ => self.transmit_frame(dev_id, p.port, frame),
            }
        }
    }

    fn ipv4_ingress(
        &mut self,
        dev_id: DevId,
        port: PortId,
        my_mac: Mac,
        frame: EthernetFrame,
        pkt: Ipv4Packet,
    ) {
        let my_ip = self
            .net
            .device(dev_id)
            .and_then(|d| d.port(port))
            .and_then(|p| p.ip);
        // 入方向 ACL（先于一切转发与本机处理；DHCP 不走 Ipv4 路径天然豁免）
        if d_kind(&self.net, dev_id) == DeviceKind::Router {
            let acl_no = self
                .net
                .device(dev_id)
                .and_then(|d| d.port(port))
                .and_then(|p| p.acl_in);
            if !self.acl_decide(dev_id, acl_no, pkt.src, pkt.dst) {
                self.dropped += 1;
                return;
            }
        }
        // 路由器：NAT 入向还原优先（outside 口收到目的为出口 IP 的报文 → 还原目的后转发）
        if d_kind(&self.net, dev_id) == DeviceKind::Router {
            let mut pkt2 = pkt;
            if self.nat_check_ingress(dev_id, port, &mut pkt2) {
                self.router_forward(dev_id, pkt2);
                return;
            }
            if Some(pkt.dst) == my_ip {
                if pkt.icmp.is_reply {
                    self.on_ping_reply(dev_id, pkt);
                } else {
                    // echo request → 回应答；应答目的 MAC 直接用请求方的帧源 MAC
                    let reply = Ipv4Packet {
                        src: pkt.dst,
                        dst: pkt.src,
                        ttl: DEFAULT_TTL,
                        icmp: IcmpPacket {
                            id: pkt.icmp.id,
                            seq: pkt.icmp.seq,
                            is_reply: true,
                        },
                    };
                    let out = EthernetFrame {
                        dst: frame.src,
                        src: my_mac,
                        vlan: None,
                        ethertype: ETHERTYPE_IPV4,
                        payload: L3::Ipv4(reply),
                    };
                    self.transmit_frame(dev_id, port, out);
                }
                return;
            }
            self.router_forward(dev_id, pkt);
            return;
        }
        // 主机：本机交付或丢弃
        if Some(pkt.dst) == my_ip {
            if pkt.icmp.is_reply {
                self.on_ping_reply(dev_id, pkt);
            } else {
                let reply = Ipv4Packet {
                    src: pkt.dst,
                    dst: pkt.src,
                    ttl: DEFAULT_TTL,
                    icmp: IcmpPacket {
                        id: pkt.icmp.id,
                        seq: pkt.icmp.seq,
                        is_reply: true,
                    },
                };
                let out = EthernetFrame {
                    dst: frame.src,
                    src: my_mac,
                    vlan: None,
                    ethertype: ETHERTYPE_IPV4,
                    payload: L3::Ipv4(reply),
                };
                self.transmit_frame(dev_id, port, out);
            }
        } else {
            self.dropped += 1;
        }
    }

    // ---------- IPv4 发送（含 ARP） ----------

    /// 路由器/主机发送 IPv4（ARP 解析、挂起、请求）。
    pub fn device_send_ip(
        &mut self,
        dev_id: DevId,
        port: PortId,
        pkt: Ipv4Packet,
        arp_for: Ipv4Addr,
    ) {
        let (src_mac, src_ip) = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            let Some(p) = d.port(port) else {
                return;
            };
            (p.mac, p.ip)
        };
        if let Some(e) = self.arp_lookup(dev_id, port, None, arp_for) {
            let frame = EthernetFrame {
                dst: e.mac,
                src: src_mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV4,
                payload: L3::Ipv4(pkt),
            };
            self.transmit_frame(dev_id, port, frame);
            return;
        }
        // 无 ARP 表项：挂起，必要时发 ARP 请求
        let need_request = {
            let Some(d) = self.net.device_mut(dev_id) else {
                return;
            };
            let pending = PendingFrame {
                port,
                vlan: None,
                dst_ip: arp_for,
                src_mac,
                pkt,
            };
            match &mut d.state {
                DeviceState::Router(rs) => {
                    rs.pending.push(pending);
                    rs.arping.insert((port, None, arp_for))
                }
                DeviceState::Host(hs) => {
                    hs.pending.push(pending);
                    hs.arping.insert((port, arp_for))
                }
                DeviceState::Switch(_) => return,
            }
        };
        if need_request {
            let req = ArpPacket {
                op: ArpOp::Request,
                sender_mac: src_mac,
                sender_ip: src_ip.unwrap_or(0),
                target_mac: ZERO_MAC,
                target_ip: arp_for,
            };
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: src_mac,
                vlan: None,
                ethertype: ETHERTYPE_ARP,
                payload: L3::Arp(req),
            };
            self.transmit_frame(dev_id, port, frame);
        }
    }

    fn arp_lookup(
        &self,
        dev_id: DevId,
        port: PortId,
        vlan: Option<VlanId>,
        ip: Ipv4Addr,
    ) -> Option<ArpEntry> {
        let d = self.net.device(dev_id)?;
        match &d.state {
            DeviceState::Router(rs) => rs.arp.get(&(port, vlan, ip)).copied(),
            DeviceState::Host(hs) => hs.arp.get(&ip).copied(),
            DeviceState::Switch(sw) => vlan.and_then(|v| sw.arp.get(&(v, ip)).copied()),
        }
    }

    fn router_forward(&mut self, dev_id: DevId, mut pkt: Ipv4Packet) {
        if pkt.ttl <= 1 {
            self.dropped += 1;
            return;
        }
        pkt.ttl -= 1;
        let Some((port, nexthop, _vlan)) = self.net.lookup_route(dev_id, pkt.dst) else {
            self.dropped += 1;
            return;
        };
        // 出方向 ACL（后于路由，先于 NAT——与真实设备一致）
        let acl_out = self
            .net
            .device(dev_id)
            .and_then(|d| d.port(port))
            .and_then(|p| p.acl_out);
        if !self.acl_decide(dev_id, acl_out, pkt.src, pkt.dst) {
            self.dropped += 1;
            return;
        }
        // NAT 出向改写（EasyIP/静态）
        self.nat_egress(dev_id, port, &mut pkt);
        self.device_send_ip(dev_id, port, pkt, nexthop);
    }

    /// ACL 判定：存在编号且命中规则按首条决定；无命中 = 隐式拒绝；编号未定义 = 放行。
    fn acl_decide(
        &mut self,
        dev_id: DevId,
        acl_no: Option<u32>,
        src: Ipv4Addr,
        dst: Ipv4Addr,
    ) -> bool {
        let Some(no) = acl_no else { return true };
        let Some(d) = self.net.device_mut(dev_id) else {
            return true;
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return true;
        };
        match rs.acls.iter_mut().find(|a| a.number == no) {
            Some(acl) => acl.decide(src, dst, true).unwrap_or(false), // M2b 载荷均为 ICMP
            None => true,
        }
    }

    // ---------- ping 会话 ----------

    /// 在路由器/交换机/主机上发起 ping，返回会话 id。首个探针随后即发。
    pub fn start_ping(&mut self, dev: DevId, target: Ipv4Addr) -> Option<u16> {
        let kind = d_kind(&self.net, dev);
        if !matches!(
            kind,
            DeviceKind::Router | DeviceKind::Switch | DeviceKind::Host
        ) {
            return None;
        }
        let sid = {
            let d = self.net.device_mut(dev)?;
            let (pings, next_id) = match &mut d.state {
                DeviceState::Router(rs) => (&mut rs.pings, &mut rs.next_ping_id),
                DeviceState::Switch(sw) => (&mut sw.pings, &mut sw.next_ping_id),
                DeviceState::Host(hs) => (&mut hs.pings, &mut hs.next_ping_id),
            };
            let id = (*next_id & 0xffff) as u16;
            *next_id += 1;
            pings.push(PingSession::new(id, target));
            id
        };
        self.push(self.now, EKind::PingSend { dev, sid, probe: 0 });
        Some(sid)
    }

    fn pings_mut(&mut self, dev: DevId) -> Option<&mut Vec<PingSession>> {
        let d = self.net.device_mut(dev)?;
        match &mut d.state {
            DeviceState::Router(rs) => Some(&mut rs.pings),
            DeviceState::Switch(sw) => Some(&mut sw.pings),
            DeviceState::Host(hs) => Some(&mut hs.pings),
        }
    }

    fn ping_session_mut(&mut self, dev: DevId, sid: u16) -> Option<&mut PingSession> {
        self.pings_mut(dev)?.iter_mut().find(|s| s.id == sid)
    }

    fn route_for_ping(&self, dev: DevId, target: Ipv4Addr) -> PingRoute {
        let Some(d) = self.net.device(dev) else {
            return PingRoute::NoRoute;
        };
        match &d.state {
            DeviceState::Host(h) => {
                let p0 = &d.ports[0];
                let Some(my_ip) = p0.ip else {
                    return PingRoute::NoRoute;
                };
                let mask = mask_from_prefix(p0.prefix);
                let arp_for = if (target & mask) == (my_ip & mask) {
                    target
                } else {
                    h.gw.unwrap_or(target)
                };
                PingRoute::Host { arp_for }
            }
            DeviceState::Router(_) => match self.net.lookup_route(dev, target) {
                Some((port, nh, _)) => PingRoute::Router { port, arp_for: nh },
                None => PingRoute::NoRoute,
            },
            DeviceState::Switch(_) => match self.net.lookup_route(dev, target) {
                Some((_, nh, Some(vlan))) => PingRoute::Switch { vlan, arp_for: nh },
                _ => PingRoute::NoRoute,
            },
        }
    }

    fn on_ping_send(&mut self, dev: DevId, sid: u16, probe: u32) {
        // 阶段 1：占用探针并取目标
        let now = self.now;
        let target = {
            let Some(s) = self.ping_session_mut(dev, sid) else {
                return;
            };
            if s.done || probe != s.next_probe || probe >= s.total {
                return;
            }
            s.next_probe += 1;
            s.probes[probe as usize].sent_ns = now;
            s.target
        };
        // 阶段 2：选路
        let decision = self.route_for_ping(dev, target);
        // 阶段 3：发送
        match decision {
            PingRoute::Host { arp_for } => {
                if let Some(src_ip) = self
                    .net
                    .device(dev)
                    .and_then(|d| d.port(0))
                    .and_then(|p| p.ip)
                {
                    let pkt = Ipv4Packet {
                        src: src_ip,
                        dst: target,
                        ttl: DEFAULT_TTL,
                        icmp: IcmpPacket {
                            id: sid,
                            seq: probe as u16,
                            is_reply: false,
                        },
                    };
                    self.device_send_ip(dev, 0, pkt, arp_for);
                } else {
                    self.mark_probe(dev, sid, probe, PingProbeState::Timeout);
                }
            }
            PingRoute::Router { port, arp_for } => {
                if let Some(src_ip) = self
                    .net
                    .device(dev)
                    .and_then(|d| d.port(port))
                    .and_then(|p| p.ip)
                {
                    let pkt = Ipv4Packet {
                        src: src_ip,
                        dst: target,
                        ttl: DEFAULT_TTL,
                        icmp: IcmpPacket {
                            id: sid,
                            seq: probe as u16,
                            is_reply: false,
                        },
                    };
                    self.device_send_ip(dev, port, pkt, arp_for);
                } else {
                    self.mark_probe(dev, sid, probe, PingProbeState::Timeout);
                }
            }
            PingRoute::Switch { vlan, arp_for } => {
                let src_ip = self.net.device(dev).and_then(|d| match &d.state {
                    DeviceState::Switch(sw) => sw.svi.get(&vlan).map(|s| s.ip),
                    _ => None,
                });
                if let Some(src_ip) = src_ip {
                    let pkt = Ipv4Packet {
                        src: src_ip,
                        dst: target,
                        ttl: DEFAULT_TTL,
                        icmp: IcmpPacket {
                            id: sid,
                            seq: probe as u16,
                            is_reply: false,
                        },
                    };
                    self.switch_send_ip(dev, vlan, pkt, arp_for);
                } else {
                    self.mark_probe(dev, sid, probe, PingProbeState::Timeout);
                }
            }
            PingRoute::NoRoute => self.mark_probe(dev, sid, probe, PingProbeState::Timeout),
        }
        // 阶段 4：调度超时与下一探针
        self.push(
            self.now + PingSession::TIMEOUT_NS,
            EKind::PingTimeout { dev, sid, probe },
        );
        if probe + 1 < PingSession::TOTAL {
            self.push(
                self.now + PingSession::INTERVAL_NS,
                EKind::PingSend {
                    dev,
                    sid,
                    probe: probe + 1,
                },
            );
        }
    }

    fn on_ping_timeout(&mut self, dev: DevId, sid: u16, probe: u32) {
        let (finished, target) = {
            let Some(s) = self.ping_session_mut(dev, sid) else {
                return;
            };
            if let Some(p) = s.probes.get_mut(probe as usize) {
                if p.state == PingProbeState::Pending {
                    p.state = PingProbeState::Timeout;
                }
            }
            (s.all_resolved(), s.target)
        };
        // 超时清掉 ARP 等待状态与挂起报文，让后续 ping 可以重新解析
        self.purge_arp_wait(dev, target);
        if finished {
            if let Some(s) = self.ping_session_mut(dev, sid) {
                s.done = true;
            }
        }
    }

    fn on_ping_reply(&mut self, dev: DevId, pkt: Ipv4Packet) {
        let now = self.now;
        let finished = {
            let Some(pings) = self.pings_mut(dev) else {
                return;
            };
            let Some(s) = pings
                .iter_mut()
                .find(|s| s.target == pkt.src && s.id == pkt.icmp.id && !s.done)
            else {
                return;
            };
            let Some(p) = s.probes.get_mut(pkt.icmp.seq as usize) else {
                return;
            };
            if p.state == PingProbeState::Pending {
                p.state = PingProbeState::Replied {
                    rtt_ns: now.saturating_sub(p.sent_ns),
                    ttl: pkt.ttl,
                };
            }
            s.all_resolved()
        };
        if finished {
            if let Some(s) = self.ping_session_mut(dev, pkt.icmp.id) {
                s.done = true;
            }
        }
    }

    fn mark_probe(&mut self, dev: DevId, sid: u16, probe: u32, state: PingProbeState) {
        let finished = {
            let Some(s) = self.ping_session_mut(dev, sid) else {
                return;
            };
            if let Some(p) = s.probes.get_mut(probe as usize) {
                if p.state == PingProbeState::Pending {
                    p.state = state;
                }
            }
            s.all_resolved()
        };
        if finished {
            if let Some(s) = self.ping_session_mut(dev, sid) {
                s.done = true;
            }
        }
    }

    fn purge_arp_wait(&mut self, dev: DevId, target: Ipv4Addr) {
        let Some(d) = self.net.device_mut(dev) else {
            return;
        };
        match &mut d.state {
            DeviceState::Router(rs) => {
                rs.arping.retain(|(_, _, ip)| *ip != target);
                rs.pending.retain(|p| p.dst_ip != target);
            }
            DeviceState::Switch(sw) => {
                sw.arping.retain(|(_, ip)| *ip != target);
                sw.pending.retain(|p| p.dst_ip != target);
            }
            DeviceState::Host(hs) => {
                hs.arping.retain(|(_, ip)| *ip != target);
                hs.pending.retain(|p| p.dst_ip != target);
            }
        }
    }

    /// 取设备上所有"已结束但未输出"的 ping 会话（供 CLI 层渲染）。
    pub fn take_unreported_pings(&mut self, dev: DevId) -> Vec<PingSession> {
        let Some(d) = self.net.device_mut(dev) else {
            return vec![];
        };
        let pings = match &mut d.state {
            DeviceState::Router(rs) => &mut rs.pings,
            DeviceState::Switch(sw) => &mut sw.pings,
            DeviceState::Host(hs) => &mut hs.pings,
        };
        let mut out = vec![];
        for s in pings.iter_mut() {
            if s.done && !s.reported {
                s.reported = true;
                out.push(s.clone());
            }
        }
        pings.retain(|s| !(s.done && s.reported));
        out
    }

    // ---------- DHCP（DORA 简化：Discover/Offer/Request/Ack） ----------

    /// 主机发起 DHCP：广播 Discover（源 0.0.0.0）。
    pub fn dhcp_discover(&mut self, dev: DevId) {
        if d_kind(&self.net, dev) != DeviceKind::Host {
            return;
        }
        let mac = self
            .net
            .device(dev)
            .and_then(|d| d.port(0))
            .map(|p| p.mac)
            .unwrap_or([0; 6]);
        let txid = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            let DeviceState::Host(hs) = &mut d.state else {
                return;
            };
            hs.next_ping_id = hs.next_ping_id.wrapping_add(1);
            hs.dhcp = Some(HostDhcp { discovering: true });
            (hs.next_ping_id * 2654435761) | 1
        };
        let pkt = Ipv4Packet {
            src: 0,
            dst: 0xffff_ffff,
            ttl: DEFAULT_TTL,
            icmp: IcmpPacket {
                id: 0,
                seq: 0,
                is_reply: false,
            },
        };
        let dhcp = DhcpPacket {
            op: DhcpOp::Discover,
            transaction_id: txid,
            client_mac: mac,
            offered_ip: 0,
            server_ip: 0,
            gateway: 0,
            prefix: 24,
            requested_ip: 0,
        };
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: mac,
            vlan: None,
            ethertype: ETHERTYPE_IPV4,
            payload: L3::Dhcp(dhcp),
        };
        let _ = pkt;
        self.transmit_frame(dev, 0, frame);
    }

    /// 路由器 DHCP 服务器：处理 Discover/Request。
    fn dhcp_server_ingress(&mut self, dev_id: DevId, port: PortId, dhcp: DhcpPacket) {
        // 找接收接口网段匹配的池
        let rx_subnet = self
            .net
            .device(dev_id)
            .and_then(|d| d.port(port))
            .and_then(|p| p.ip)
            .map(|ip| {
                (
                    ip,
                    self.net
                        .device(dev_id)
                        .and_then(|d| d.port(port))
                        .map(|p| p.prefix)
                        .unwrap_or(24),
                )
            });
        let Some((my_ip, prefix)) = rx_subnet else {
            return; // 接口未配地址，不做服务器
        };
        let mask = mask_from_prefix(prefix);
        let subnet = my_ip & mask;
        let Some(d) = self.net.device_mut(dev_id) else {
            return;
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return;
        };
        let pool = rs
            .dhcp_pools
            .iter()
            .find(|p| (p.network & mask_from_prefix(p.prefix)) == subnet && p.prefix == prefix)
            .cloned();
        let Some(pool) = pool else { return };

        match dhcp.op {
            DhcpOp::Discover => {
                // 分配：池内第一个未被租约占用的地址
                let start = pool.network & mask_from_prefix(pool.prefix);
                let size = 1u32 << (32 - pool.prefix).min(24);
                let used: Vec<u32> = rs
                    .dhcp_leases
                    .iter()
                    .map(|l| l.ip)
                    .chain(std::iter::once(pool.gateway))
                    .collect();
                let mut offer = None;
                for i in 1..size.saturating_sub(1) {
                    let cand = start + i;
                    if !used.contains(&cand) && (cand & mask) == subnet {
                        offer = Some(Ipv4Addr::from(cand));
                        break;
                    }
                }
                let Some(offered_ip) = offer else { return };
                let out = DhcpPacket {
                    op: DhcpOp::Offer,
                    transaction_id: dhcp.transaction_id,
                    client_mac: dhcp.client_mac,
                    offered_ip,
                    server_ip: my_ip,
                    gateway: pool.gateway,
                    prefix: pool.prefix,
                    requested_ip: 0,
                };
                let frame = EthernetFrame {
                    dst: dhcp.client_mac,
                    src: port_mac(dev_id, port),
                    vlan: None,
                    ethertype: ETHERTYPE_IPV4,
                    payload: L3::Dhcp(out),
                };
                self.transmit_frame(dev_id, port, frame);
            }
            DhcpOp::Request => {
                // 确认租约（若请求地址仍在池内且空闲或已被它租用）
                let requested = if dhcp.requested_ip != 0 {
                    dhcp.requested_ip
                } else {
                    dhcp.offered_ip
                };
                let in_pool = (requested & mask_from_prefix(pool.prefix))
                    == (pool.network & mask_from_prefix(pool.prefix));
                if !in_pool {
                    return;
                }
                let already = rs
                    .dhcp_leases
                    .iter()
                    .any(|l| l.mac == dhcp.client_mac && l.ip == requested);
                if !already {
                    // 地址被其他客户端占用则拒绝
                    if rs
                        .dhcp_leases
                        .iter()
                        .any(|l| l.ip == requested && l.mac != dhcp.client_mac)
                    {
                        return;
                    }
                    rs.dhcp_leases.push(DhcpLease {
                        mac: dhcp.client_mac,
                        ip: requested,
                    });
                }
                let out = DhcpPacket {
                    op: DhcpOp::Ack,
                    transaction_id: dhcp.transaction_id,
                    client_mac: dhcp.client_mac,
                    offered_ip: requested,
                    server_ip: my_ip,
                    gateway: pool.gateway,
                    prefix: pool.prefix,
                    requested_ip: 0,
                };
                let frame = EthernetFrame {
                    dst: dhcp.client_mac,
                    src: port_mac(dev_id, port),
                    vlan: None,
                    ethertype: ETHERTYPE_IPV4,
                    payload: L3::Dhcp(out),
                };
                self.transmit_frame(dev_id, port, frame);
            }
            _ => {}
        }
    }

    /// 主机 DHCP 客户端：处理 Offer/Request-Ack。
    fn dhcp_client_ingress(&mut self, dev_id: DevId, _port: PortId, dhcp: DhcpPacket) {
        // 阶段 1：在借用内读取/更新客户端状态
        enum ClientAction {
            /// 广播 Request（续 DORA 第 3 步）
            SendRequest,
            /// 应用 Ack 的地址
            Apply,
            None,
        }
        let action = {
            let Some(d) = self.net.device_mut(dev_id) else {
                return;
            };
            let DeviceState::Host(hs) = &mut d.state else {
                return;
            };
            if !hs.dhcp_enable {
                return;
            }
            match dhcp.op {
                DhcpOp::Offer => {
                    if !hs.dhcp.as_ref().map(|x| x.discovering).unwrap_or(false) {
                        return; // 没在请求中
                    }
                    if let Some(x) = hs.dhcp.as_mut() {
                        x.discovering = false;
                    }
                    ClientAction::SendRequest
                }
                DhcpOp::Ack => {
                    hs.gw = if dhcp.gateway != 0 {
                        Some(dhcp.gateway)
                    } else {
                        None
                    };
                    hs.dhcp = Some(HostDhcp { discovering: false });
                    ClientAction::Apply
                }
                _ => ClientAction::None,
            }
        };
        // 阶段 2：借用已结束，执行动作
        match action {
            ClientAction::SendRequest => {
                let mac = self
                    .net
                    .device(dev_id)
                    .and_then(|d| d.port(0))
                    .map(|p| p.mac)
                    .unwrap_or([0; 6]);
                let out = DhcpPacket {
                    op: DhcpOp::Request,
                    transaction_id: dhcp.transaction_id,
                    client_mac: mac,
                    offered_ip: 0,
                    server_ip: dhcp.server_ip,
                    gateway: 0,
                    prefix: dhcp.prefix,
                    requested_ip: dhcp.offered_ip,
                };
                let frame = EthernetFrame {
                    dst: BROADCAST_MAC,
                    src: mac,
                    vlan: None,
                    ethertype: ETHERTYPE_IPV4,
                    payload: L3::Dhcp(out),
                };
                self.transmit_frame(dev_id, 0, frame);
            }
            ClientAction::Apply => {
                if let Some(d2) = self.net.device_mut(dev_id) {
                    if let Some(p) = d2.port_mut(0) {
                        p.ip = Some(dhcp.offered_ip);
                        p.prefix = dhcp.prefix;
                    }
                }
            }
            ClientAction::None => {}
        }
    }

    // ---------- NAT（EasyIP PAT + 静态） ----------

    /// 出方向 NAT 改写：进→出方向且出口为 outside 时改写源。
    fn nat_egress(&mut self, dev_id: DevId, egress: PortId, pkt: &mut Ipv4Packet) {
        // 阶段 1：读取 NAT 配置与出口 IP（只读借用）
        let (is_outside, has_inside, public_ip, static_map, entries) = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            let DeviceState::Router(rs) = &d.state else {
                return;
            };
            (
                rs.nat_outside.contains(&egress),
                !rs.nat_inside.is_empty(),
                self.net
                    .device(dev_id)
                    .and_then(|x| x.port(egress))
                    .and_then(|p| p.ip),
                rs.nat_static.clone(),
                rs.nat_entries.clone(),
            )
        };
        if !is_outside || !has_inside {
            return;
        }
        let Some(public_ip) = public_ip else { return };
        // 静态 NAT 优先
        if let Some((_, out_ip)) = static_map.iter().find(|(i, _)| *i == pkt.src) {
            pkt.src = *out_ip;
            return;
        }
        // EasyIP PAT：记录 (src, id) → public_ip
        let entry = NatEntry {
            inside_ip: pkt.src,
            inside_id: pkt.icmp.id,
            outside_ip: public_ip,
            outside_id: pkt.icmp.id,
        };
        if !entries.contains(&entry) {
            if let Some(d) = self.net.device_mut(dev_id) {
                if let DeviceState::Router(rs) = &mut d.state {
                    rs.nat_entries.push(entry);
                }
            }
        }
        pkt.src = public_ip;
    }

    /// 入方向 NAT 还原：到达 outside 接口、目的为出口 IP 且匹配转换表 → 还原目的。
    fn nat_ingress(&mut self, dev_id: DevId, _port: PortId, pkt: &mut Ipv4Packet) -> bool {
        let Some(d) = self.net.device(dev_id) else {
            return false;
        };
        let DeviceState::Router(rs) = &d.state else {
            return false;
        };
        if rs.nat_outside.is_empty() {
            return false;
        }
        // 静态 NAT 还原
        if let Some((in_ip, _)) = rs.nat_static.iter().find(|(_, o)| *o == pkt.dst) {
            pkt.dst = *in_ip;
            return true;
        }
        // EasyIP 还原：目的 = 任一 outside 接口 IP
        let is_public_ip = self.net.device(dev_id).is_some_and(|d| {
            d.ports
                .iter()
                .any(|p| rs.nat_outside.contains(&p.idx) && p.ip == Some(pkt.dst))
        });
        if is_public_ip {
            if let Some(e) = rs
                .nat_entries
                .iter()
                .find(|e| e.outside_ip == pkt.dst && e.outside_id == pkt.icmp.id)
            {
                pkt.dst = e.inside_ip;
                pkt.icmp.id = e.inside_id;
                return true;
            }
        }
        false
    }
}

/// STP（802.1D 简化：单实例、无 TCN/老化）辅助的占位注释结束标记
impl Engine {
    /// 在 l3_ingress 的路由器分支里对进入 outside 口的报文做 NAT 还原。
    /// 返回 true 表示已改写（继续按新目的转发）。
    fn nat_check_ingress(&mut self, dev_id: DevId, port: PortId, pkt: &mut Ipv4Packet) -> bool {
        let is_outside = self
            .net
            .device(dev_id)
            .map(|d| match &d.state {
                DeviceState::Router(rs) => rs.nat_outside.contains(&port),
                _ => false,
            })
            .unwrap_or(false);
        if !is_outside {
            return false;
        }
        self.nat_ingress(dev_id, port, pkt)
    }

    /// 端口路径成本：覆盖值优先，否则按速率（1Gbps → 4）。
    fn port_cost(&self, dev: DevId, port: PortId) -> u32 {
        self.net
            .device(dev)
            .and_then(|d| d.port(port))
            .map(|p| p.stp_cost.unwrap_or(STP_COST_1G))
            .unwrap_or(STP_COST_1G)
    }

    /// 拓扑变化后由桥接层调用：清理死端口 BPDU、全量重算、调度 hello。
    pub fn stp_topology_changed(&mut self) {
        // 活跃端口集合：链路存在且两端端口均管理性开启
        let mut live: std::collections::HashSet<(DevId, PortId)> = Default::default();
        for l in &self.net.links {
            if !l.up {
                continue;
            }
            let a_up = self
                .net
                .device(l.a.0)
                .and_then(|d| d.port(l.a.1))
                .map(|p| p.admin_up)
                .unwrap_or(false);
            let b_up = self
                .net
                .device(l.b.0)
                .and_then(|d| d.port(l.b.1))
                .map(|p| p.admin_up)
                .unwrap_or(false);
            if a_up && b_up {
                live.insert(l.a);
                live.insert(l.b);
            }
        }
        let devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::Switch)
            .map(|d| d.id)
            .collect();
        for dev in devs {
            // 非活跃端口的 BPDU 缓存失效（对端关机/链路拆除立即感知）
            let dead: Vec<PortId> = {
                let Some(d) = self.net.device(dev) else {
                    continue;
                };
                let DeviceState::Switch(sw) = &d.state else {
                    continue;
                };
                sw.stp
                    .port_bpdus
                    .keys()
                    .copied()
                    .filter(|p| !live.contains(&(dev, *p)))
                    .collect()
            };
            let Some(d) = self.net.device_mut(dev) else {
                continue;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                continue;
            };
            for p in dead {
                sw.stp.port_bpdus.remove(&p);
            }
        }
        self.stp_recompute_all();
    }

    /// 全量重算所有交换机的 STP，并保证 hello 定时器在跑。
    pub fn stp_recompute_all(&mut self) {
        let devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::Switch)
            .map(|d| d.id)
            .collect();
        for dev in devs {
            self.stp_recompute(dev);
            let enabled = self
                .net
                .device(dev)
                .map(|d| match &d.state {
                    DeviceState::Switch(sw) => sw.stp.enabled,
                    _ => false,
                })
                .unwrap_or(false);
            if enabled {
                let pending = {
                    let Some(d) = self.net.device_mut(dev) else {
                        continue;
                    };
                    let DeviceState::Switch(sw) = &mut d.state else {
                        continue;
                    };
                    if sw.stp.hello_pending {
                        true
                    } else {
                        sw.stp.hello_pending = true;
                        false
                    }
                };
                if !pending {
                    self.push(self.now + STP_HELLO_NS, EKind::StpHello { dev });
                }
            }
        }
    }

    /// 收到 BPDU：记录并触发本机重算。
    fn stp_on_bpdu(&mut self, dev: DevId, port: PortId, bpdu: BpduPacket) {
        if let Some(d) = self.net.device_mut(dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.stp.port_bpdus.insert(port, bpdu);
                sw.stp.last_bpdu_ns.insert(port, self.now);
            }
        }
        self.stp_recompute(dev);
    }

    /// 核心算法：根据各端口最优 BPDU 重选根桥、根端口与端口角色，并驱动状态机。
    fn stp_recompute(&mut self, dev: DevId) {
        // 阶段 1：读取现场
        let (my_bridge, enabled, n_ports, bpdus) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            if !sw.stp.enabled {
                return;
            }
            (
                sw.stp.bridge_id(dev),
                sw.stp.enabled,
                d.ports.len(),
                sw.stp.port_bpdus.clone(),
            )
        };
        if !enabled {
            return;
        }
        let _ = enabled;

        // 阶段 2：选举根桥
        let mut best_root = my_bridge;
        for bp in bpdus.values() {
            if bp.root < best_root {
                best_root = bp.root;
            }
        }

        // 阶段 3：根端口与根成本（仅统计公告根 == 选中根的端口）
        let mut root_port: Option<PortId> = None;
        let mut root_cost: u32 = 0;
        if best_root != my_bridge {
            let mut best: Option<(u32, BridgeId, PortId)> = None;
            for (port, bp) in &bpdus {
                if bp.root != best_root {
                    continue;
                }
                let c = bp.cost + self.port_cost(dev, *port);
                let cand = (c, bp.sender, *port);
                if best.map(|b| cand < b).unwrap_or(true) {
                    best = Some(cand);
                }
            }
            if let Some((c, _, port)) = best {
                root_port = Some(port);
                root_cost = c;
            }
        }

        // 阶段 4：逐端口角色
        let mut roles: Vec<(PortId, StpRole)> = Vec::with_capacity(n_ports);
        for port in 0..n_ports {
            let has_link = self
                .net
                .device(dev)
                .and_then(|d| d.port(port))
                .map(|p| p.link.is_some() && p.admin_up)
                .unwrap_or(false);
            if !has_link {
                roles.push((port, StpRole::Disabled));
                continue;
            }
            if root_port == Some(port) {
                roles.push((port, StpRole::Root));
                continue;
            }
            match bpdus.get(&port) {
                Some(bp) if bp.root == best_root => {
                    // 我方对网段的报价：(到根成本, 网桥 ID)；对端公告：(cost, sender)
                    let my_offer = (root_cost, my_bridge);
                    let their_offer = (bp.cost, bp.sender);
                    if my_offer <= their_offer {
                        roles.push((port, StpRole::Designated));
                    } else {
                        roles.push((port, StpRole::Alternate));
                    }
                }
                _ => roles.push((port, StpRole::Designated)), // 无 BPDU 的边缘网段 → 指定
            }
        }

        // 根端口变化 = 拓扑变化 → 全网 MAC 表冲刷（802.1D TCN 的近似实现）
        let old_root_port = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            sw.stp.root_port
        };
        // 阶段 5：应用角色 → 状态转移 + 调度推进
        for (port, role) in roles {
            let old_role = /* 读旧角色 */ {
                let Some(d) = self.net.device(dev) else { continue };
                let DeviceState::Switch(sw) = &d.state else { continue };
                sw.stp.role_of(port)
            };
            let old_state = {
                let Some(d) = self.net.device(dev) else {
                    continue;
                };
                let DeviceState::Switch(sw) = &d.state else {
                    continue;
                };
                sw.stp.state_of(port)
            };
            let mut epoch_bump = false;
            {
                let Some(d) = self.net.device_mut(dev) else {
                    continue;
                };
                let DeviceState::Switch(sw) = &mut d.state else {
                    continue;
                };
                if old_role != role {
                    let e = sw.stp.epochs.entry(port).or_insert(0);
                    *e += 1;
                    epoch_bump = true;
                }
                let state = match role {
                    StpRole::Disabled => StpSt::Blocking,
                    StpRole::Alternate => StpSt::Blocking,
                    StpRole::Root | StpRole::Designated => {
                        // 新进入转发路径的端口从 Listening 开始
                        if old_state == StpSt::Blocking || epoch_bump {
                            StpSt::Listening
                        } else {
                            old_state
                        }
                    }
                };
                sw.stp.set_port(port, role, state);
                // 写回根信息
                sw.stp.root = best_root;
                sw.stp.root_cost = root_cost;
                sw.stp.root_port = root_port;
            }
            // 调度推进（仅 RP/DP 且处于 Listening）
            let (cur_state, cur_role, cur_epoch) = {
                let Some(d) = self.net.device(dev) else {
                    continue;
                };
                let DeviceState::Switch(sw) = &d.state else {
                    continue;
                };
                (
                    sw.stp.state_of(port),
                    sw.stp.role_of(port),
                    sw.stp.epoch_of(port),
                )
            };
            if cur_state == StpSt::Listening
                && matches!(cur_role, StpRole::Root | StpRole::Designated)
            {
                let e = cur_epoch;
                self.push(
                    self.now + STP_FORWARD_DELAY_NS,
                    EKind::StpAdvance {
                        dev,
                        port,
                        to: StpSt::Learning,
                        epoch: e,
                    },
                );
                self.push(
                    self.now + 2 * STP_FORWARD_DELAY_NS,
                    EKind::StpAdvance {
                        dev,
                        port,
                        to: StpSt::Forwarding,
                        epoch: e,
                    },
                );
            }
        }
        if old_root_port != root_port {
            for d2 in &mut self.net.devices {
                if let DeviceState::Switch(sw2) = &mut d2.state {
                    sw2.mac_table.clear();
                }
            }
        }
    }

    /// STP 端口状态推进事件。
    fn on_stp_advance(&mut self, dev: DevId, port: PortId, to: StpSt, epoch: u32) {
        let Some(d) = self.net.device_mut(dev) else {
            return;
        };
        let DeviceState::Switch(sw) = &mut d.state else {
            return;
        };
        if !sw.stp.enabled || sw.stp.epoch_of(port) != epoch {
            return;
        }
        let role = sw.stp.role_of(port);
        if !matches!(role, StpRole::Root | StpRole::Designated) {
            return;
        }
        // 仅允许沿 Listening→Learning→Forwarding 方向推进
        let cur = sw.stp.state_of(port);
        let ok = matches!(
            (cur, to),
            (StpSt::Listening, StpSt::Learning) | (StpSt::Learning, StpSt::Forwarding)
        );
        if ok {
            if let Some(p) = sw.stp.ports.get_mut(&port) {
                p.state = to;
            }
        }
    }

    /// STP 周期 hello：先做 BPDU 老化检查，再从所有指派端口发配置 BPDU，并重排下一次。
    fn on_stp_hello(&mut self, dev: DevId) {
        if !self.stp_on(dev) {
            return;
        }
        // Max-Age 老化：缓存了 BPDU 但超过 20s 没收到邻居报文 → 淘汰并重算
        {
            let expired: Vec<PortId> = {
                let Some(d) = self.net.device(dev) else {
                    return;
                };
                let DeviceState::Switch(sw) = &d.state else {
                    return;
                };
                sw.stp
                    .port_bpdus
                    .keys()
                    .copied()
                    .filter(|p| {
                        let last = sw.stp.last_bpdu_ns.get(p).copied().unwrap_or(0);
                        self.now.saturating_sub(last) > STP_MAX_AGE_NS
                    })
                    .collect()
            };
            if !expired.is_empty() {
                let Some(d) = self.net.device_mut(dev) else {
                    return;
                };
                let DeviceState::Switch(sw) = &mut d.state else {
                    return;
                };
                for p in expired {
                    sw.stp.port_bpdus.remove(&p);
                    sw.stp.last_bpdu_ns.remove(&p);
                }
                self.stp_recompute(dev);
            }
        }
        let (my_bridge, root, root_cost) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            (sw.stp.bridge_id(dev), sw.stp.root, sw.stp.root_cost)
        };
        let n = self.net.device(dev).map(|d| d.ports.len()).unwrap_or(0);
        let designated: Vec<PortId> = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            (0..n)
                .filter(|&p| sw.stp.role_of(p) == StpRole::Designated)
                .collect()
        };
        for port in designated {
            if self
                .net
                .device(dev)
                .and_then(|d| d.port(port))
                .map(|p| p.link.is_some() && p.admin_up)
                .unwrap_or(false)
            {
                let bpdu = BpduPacket {
                    root,
                    cost: root_cost,
                    sender: my_bridge,
                    port: port as u32,
                };
                let frame = EthernetFrame {
                    dst: STP_MULTICAST_MAC,
                    src: self
                        .net
                        .device(dev)
                        .and_then(|d| d.port(port))
                        .map(|p| p.mac)
                        .unwrap_or([0; 6]),
                    vlan: None,
                    ethertype: ETHERTYPE_BPDU,
                    payload: L3::Bpdu(bpdu),
                };
                self.transmit_frame(dev, port, frame);
            }
        }
        // 重排 hello
        {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.stp.hello_pending = false;
            }
        }
        if self.stp_on(dev) {
            let pending = {
                let Some(d) = self.net.device_mut(dev) else {
                    return;
                };
                let DeviceState::Switch(sw) = &mut d.state else {
                    return;
                };
                if sw.stp.hello_pending {
                    true
                } else {
                    sw.stp.hello_pending = true;
                    false
                }
            };
            if !pending {
                self.push(self.now + STP_HELLO_NS, EKind::StpHello { dev });
            }
        }
    }
}

fn d_kind(net: &Net, dev: DevId) -> DeviceKind {
    net.device(dev)
        .map(|d| d.kind)
        .unwrap_or(DeviceKind::Router)
}
