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
/// LACP 周期（快速率 1s；教学场景优先收敛速度）。
pub const LACP_INTERVAL_NS: u64 = 1_000_000_000;
/// LACP 邻居超时（3 个周期未收到即失效）。
pub const LACP_TIMEOUT_NS: u64 = 3_500_000_000;
/// OSPF Hello 周期与邻居失效时间（简化）。
pub const OSPF_HELLO_NS: u64 = 2_000_000_000;
pub const OSPF_DEAD_NS: u64 = 6_000_000_000;
pub const OSPFV3_HELLO_NS: u64 = 2_000_000_000;
pub const OSPFV3_DEAD_NS: u64 = 6_000_000_000;
/// VRRP 通告周期与 Master 失效时间。
pub const VRRP_ADVERT_NS: u64 = 1_000_000_000;
pub const VRRP_MASTER_DOWN_NS: u64 = 3_500_000_000;
/// BGP Keepalive 周期与邻居失效时间（简化）。
pub const BGP_KEEPALIVE_NS: u64 = 1_000_000_000;
pub const BGP_HOLD_NS: u64 = 5_000_000_000;
/// IS-IS Hello 周期与邻居失效时间（简化）。
pub const ISIS_HELLO_NS: u64 = 2_000_000_000;
pub const ISIS_DEAD_NS: u64 = 6_000_000_000;
/// EVPN 路由通告周期（简化）。
pub const EVPN_INTERVAL_NS: u64 = 2_000_000_000;

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

/// 端口 QoS 统计行：(端口, 广播上限, 限速 kbps, 限速丢弃, 风暴抑制丢弃)。
pub type QosStat = (PortId, Option<u32>, Option<u32>, u64, u64);

/// 远程登录（Telnet/SSH）失败原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginError {
    /// 目标不可达 / 目标地址不属于任何设备
    Unreachable,
    /// VTY 未允许该协议
    TransportDenied,
    /// 认证失败（未开启本地认证或用户名/口令/服务类型不匹配）
    AuthFailed,
}

/// MSTP 实例换入前的 CIST 现场。
struct CistSaved {
    root: BridgeId,
    root_cost: u32,
    root_port: Option<PortId>,
    ports: std::collections::HashMap<PortId, StpPortSt>,
    port_bpdus: std::collections::HashMap<PortId, BpduPacket>,
    last_bpdu_ns: std::collections::HashMap<PortId, u64>,
    epochs: std::collections::HashMap<PortId, u32>,
    priority: u16,
}

#[allow(clippy::large_enum_variant)] // 事件里直接携带帧，避免热路径上额外的分配
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
    /// LACP 周期定时器（发 LACPDU + 老化对端 + 重选成员）
    LacpTimer {
        dev: DevId,
    },
    /// OSPF Hello/邻居老化定时器
    OspfTimer {
        dev: DevId,
    },
    /// VRRP 通告/超时定时器
    VrrpTimer {
        dev: DevId,
    },
    /// IPv6 ping 发送/结束
    Ping6Send {
        dev: DevId,
        sid: u16,
        probe: u32,
    },
    Ping6Done {
        dev: DevId,
        sid: u16,
    },
    /// BGP Keepalive/会话检查定时器
    BgpTimer {
        dev: DevId,
    },
    /// IS-IS Hello/老化定时器
    IsisTimer {
        dev: DevId,
    },
    /// EVPN 路由通告定时器
    EvpnTimer {
        dev: DevId,
    },
    /// OSPFv3 Hello 定时器
    Ospfv3Timer {
        dev: DevId,
    },
}

/// OSPF 邻居（简化：仅记录存活、首次跳地址与接口）。
#[derive(Clone, Copy, Debug)]
struct OspfNeighbor {
    ip: Ipv4Addr,
    port: PortId,
    last_ns: u64,
}

/// OSPF 进程运行时（LSDB + 邻居），不进摘要。
#[derive(Default)]
struct OspfRuntime {
    lsdb: std::collections::HashMap<Ipv4Addr, OspfLsa>,
    neighbors: std::collections::HashMap<Ipv4Addr, OspfNeighbor>,
    timer_pending: bool,
}

/// OSPFv3 邻居（简化：记录接口 IPv6 与本机前缀列表）。
#[derive(Clone, Debug)]
struct Ospfv3Neighbor {
    ip: Ipv6Addr,
    port: PortId,
    last_ns: u64,
    prefixes: Vec<(Ipv6Addr, u8)>,
}

#[derive(Default)]
struct Ospfv3Runtime {
    neighbors: std::collections::HashMap<Ipv4Addr, Ospfv3Neighbor>,
    timer_pending: bool,
}

/// BGP 邻居会话状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BgpState {
    Idle,
    Established,
}

#[derive(Clone, Copy, Debug)]
struct BgpSession {
    remote_as: u32,
    state: BgpState,
    last_rx_ns: u64,
}

#[derive(Clone, Debug)]
struct BgpLearned {
    nexthop: Ipv4Addr,
    path: Vec<u32>,
    from: Ipv4Addr,
    local_pref: u32,
}

#[derive(Default)]
struct BgpRuntime {
    sessions: std::collections::HashMap<Ipv4Addr, BgpSession>,
    learned: std::collections::HashMap<(Ipv4Addr, u8), BgpLearned>,
    timer_pending: bool,
}

#[derive(Clone, Copy, Debug)]
struct IsisNb {
    ip: Ipv4Addr,
    port: PortId,
    last_ns: u64,
}

#[derive(Default)]
struct IsisRuntime {
    lsdb: std::collections::HashMap<[u8; 6], IsisLsa>,
    neighbors: std::collections::HashMap<[u8; 6], IsisNb>,
    timer_pending: bool,
}

/// 端口 QoS 运行时（令牌桶 + 广播计数窗口）。
#[derive(Clone, Copy, Debug)]
struct QosState {
    last_ns: u64,
    tokens: f64,
    window_ns: u64,
    bcast: u32,
    drops: u64,
    bcast_drops: u64,
}

impl Default for QosState {
    fn default() -> Self {
        Self {
            last_ns: 0,
            tokens: f64::INFINITY,
            window_ns: 0,
            bcast: 0,
            drops: 0,
            bcast_drops: 0,
        }
    }
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
    ospf: std::collections::HashMap<DevId, OspfRuntime>,
    vrrp_pending: std::collections::HashSet<DevId>,
    qos: std::collections::HashMap<(DevId, PortId), QosState>,
    bgp: std::collections::HashMap<DevId, BgpRuntime>,
    isis: std::collections::HashMap<DevId, IsisRuntime>,
    ospfv3: std::collections::HashMap<DevId, Ospfv3Runtime>,
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
            ospf: std::collections::HashMap::new(),
            vrrp_pending: Default::default(),
            qos: std::collections::HashMap::new(),
            bgp: std::collections::HashMap::new(),
            isis: std::collections::HashMap::new(),
            ospfv3: std::collections::HashMap::new(),
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
                EKind::LacpTimer { dev } => self.on_lacp_timer(dev),
                EKind::OspfTimer { dev } => self.on_ospf_timer(dev),
                EKind::VrrpTimer { dev } => self.on_vrrp_timer(dev),
                EKind::Ping6Send { dev, sid, probe } => self.on_ping6_send(dev, sid, probe),
                EKind::Ping6Done { dev, sid } => self.on_ping6_done(dev, sid),
                EKind::BgpTimer { dev } => self.on_bgp_timer(dev),
                EKind::IsisTimer { dev } => self.on_isis_timer(dev),
                EKind::EvpnTimer { dev } => self.on_evpn_timer(dev),
                EKind::Ospfv3Timer { dev } => self.on_ospfv3_timer(dev),
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
            DeviceKind::Switch | DeviceKind::AccessPoint => {
                self.switch_ingress(dev_id, port, frame)
            }
            _ => self.l3_ingress(dev_id, port, frame),
        }
    }

    /// 端口能否转发该 VLAN 的数据帧（STP 实例 Forwarding + 聚合成员已选中）。
    /// 堆叠/M-LAG 共享的系统 MAC（用于跨设备 LACP 呈现同一对端）。
    fn fabric_system_mac(&self, dev: DevId) -> Option<Mac> {
        let d = self.net.device(dev)?;
        let DeviceState::Switch(sw) = &d.state else {
            return None;
        };
        if let Some(s) = &sw.stack {
            return Some([0x02, 0, 0, (s.domain >> 8) as u8, s.domain as u8, 0x01]);
        }
        if let Some(m) = &sw.mlag {
            return Some([0x02, 0, 0, (m.domain >> 8) as u8, m.domain as u8, 0x02]);
        }
        None
    }

    /// 堆叠（同 domain 的成员间）或 M-LAG peer-link 视为内部链路，STP 不阻塞。
    fn is_fabric_link(&self, dev: DevId, port: PortId) -> bool {
        let Some(d) = self.net.device(dev) else {
            return false;
        };
        let DeviceState::Switch(sw) = &d.state else {
            return false;
        };
        if sw
            .mlag
            .as_ref()
            .is_some_and(|m| m.peer_links.contains(&port))
        {
            return true;
        }
        let Some(link) = d.port(port).and_then(|p| p.link) else {
            return false;
        };
        let Some(l) = self.net.link(link) else {
            return false;
        };
        let peer = if l.a.0 == dev { l.b.0 } else { l.a.0 };
        let Some(pd) = self.net.device(peer) else {
            return false;
        };
        let DeviceState::Switch(psw) = &pd.state else {
            return false;
        };
        match (&sw.stack, &psw.stack) {
            (Some(a), Some(b)) => a.domain != 0 && a.domain == b.domain,
            _ => false,
        }
    }

    fn can_forward(&self, dev_id: DevId, port: PortId, vlan: VlanId) -> bool {
        if self.is_fabric_link(dev_id, port) {
            return true;
        }
        let Some(d) = self.net.device(dev_id) else {
            return false;
        };
        // 聚合成员：未被 LACP 选中的成员不转发数据
        if let Some(gid) = d.port(port).and_then(|p| p.lag) {
            let DeviceState::Switch(sw) = &d.state else {
                return false;
            };
            let Some(g) = sw.lags.get(&gid) else {
                return false;
            };
            if !g.selected.contains(&port) {
                return false;
            }
        }
        match &d.state {
            DeviceState::Switch(sw) => {
                if sw.stack.is_some() || sw.mlag.is_some() || !sw.stp.enabled {
                    return true;
                }
                let inst = if sw.stp.mode == StpMode::Mstp {
                    sw.stp.instance_for_vlan(vlan)
                } else {
                    0
                };
                sw.stp.state_in(inst, self.logical_port(dev_id, port)) == StpSt::Forwarding
            }
            _ => true,
        }
    }

    fn stp_on(&self, dev_id: DevId) -> bool {
        matches!(
            self.net.device(dev_id).map(|d| &d.state),
            Some(DeviceState::Switch(sw))
                if sw.stp.enabled && sw.stack.is_none() && sw.mlag.is_none()
        )
    }

    /// 三层交换机入方向：LACP/BPDU 截获 → VLAN 归类 → 学习 → SVI 截获 → 转发/洪泛。
    fn switch_ingress(&mut self, dev_id: DevId, in_port: PortId, frame: EthernetFrame) {
        // 0. AP：未关联的无线终端帧直接丢弃
        if !self.ap_client_allowed(dev_id, in_port) {
            self.dropped += 1;
            return;
        }
        // 0b. AP：CAPWAP 配置帧由 AP 本机处理
        if d_kind(&self.net, dev_id) == DeviceKind::AccessPoint {
            if let L3::Capwap(pkt) = frame.payload {
                if self.local_ip_of(dev_id) == Some(pkt.dst) {
                    self.capwap_ingress(dev_id, pkt);
                    return;
                }
            }
        }
        // 0c. VXLAN：VTEP 解封装（仅本机 VTEP 地址）；Underlay 路由器按外层 IPv4 转发
        if let L3::Vxlan(vx) = frame.payload {
            let fwd = VxlanPacket {
                ttl: vx.ttl.saturating_sub(1),
                ..vx
            };
            if !self.encap_fwd_over_underlay(dev_id, in_port, vx.dst, vx.ttl, L3::Vxlan(fwd)) {
                self.vxlan_on_frame(dev_id, in_port, vx);
            }
            return;
        }
        // 0d. EVPN：VTEP 控制面；Underlay 路由器按外层单播转发
        if let L3::Evpn(e) = frame.payload {
            if !self.encap_fwd_over_underlay(dev_id, in_port, e.dst, DEFAULT_TTL, L3::Evpn(e)) {
                self.evpn_on_packet(dev_id, e);
            }
            return;
        }
        // 0a. LACP：先于 VLAN/STP 处理（聚合协商与端口状态无关）
        if frame.ethertype == ETHERTYPE_LACP {
            if let L3::Lacp(pdu) = frame.payload {
                self.lacp_on_pdu(dev_id, in_port, pdu);
            }
            return;
        }
        // 0. BPDU：先于 VLAN/端口状态处理（STP 与 VLAN 无关，阻塞端口照常收）
        if frame.ethertype == ETHERTYPE_BPDU {
            if let L3::Bpdu(bpdu) = frame.payload {
                // 聚合组视为一个逻辑端口：任何成员收到的 BPDU 归到代表端口
                let logical = self.logical_port(dev_id, in_port);
                if self.stp_on(dev_id) {
                    self.stp_on_bpdu(dev_id, logical, bpdu);
                } else if matches!(
                    self.net.device(dev_id).map(|d| &d.state),
                    Some(DeviceState::Switch(sw)) if sw.stack.is_some() || sw.mlag.is_some()
                ) {
                    // 堆叠/M-LAG 成员不参与 STP，BPDU 直接丢弃，避免泛洪成环
                } else {
                    // STP 关闭：BPDU 当作未知组播洪泛（广播风暴演示的一部分）
                    for p in self.logical_ports(dev_id) {
                        if p != logical {
                            self.transmit_frame(dev_id, p, frame);
                        }
                    }
                }
            }
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
        // STP 启用时，非 Forwarding 端口（按该 VLAN 的实例）丢弃数据帧（不学习）
        if self.stp_on(dev_id)
            && !self.is_fabric_link(dev_id, in_port)
            && !self.can_forward(dev_id, in_port, vlan)
        {
            self.dropped += 1;
            return;
        }
        // QoS：入口限速 / 广播风暴抑制
        if self.qos_ingress_drop(dev_id, in_port, &frame) {
            self.dropped += 1;
            return;
        }
        // DHCP Snooping：仅信任端口可下发服务器报文；同时学习绑定表
        if let L3::Dhcp(dhcp) = frame.payload {
            if self.dhcp_snooping_ingress(dev_id, in_port, vlan, &dhcp) {
                self.dropped += 1;
                return;
            }
        }
        // 端口安全：超出最大 MAC 数按违规动作处理
        if !self.port_security_allow(dev_id, in_port, frame.src) {
            self.dropped += 1;
            return;
        }
        // VXLAN：向远端 VTEP 复制封装（本地转发继续）
        self.vxlan_encap(dev_id, in_port, vlan, &frame);
        // 2. 学习 + 查表（学习到聚合组的逻辑端口，避免成员间漂移）
        let logical_in = self.logical_port(dev_id, in_port);
        let (dst_port, svi_ip, _port_count) = {
            let Some(d) = self.net.device_mut(dev_id) else {
                return;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return;
            };
            sw.mac_table.insert((vlan, frame.src), logical_in);
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
        let from_fabric = self.is_fabric_link(dev_id, in_port);
        let out_ports: Vec<PortId> = match dst_port {
            Some(p)
                if p != logical_in
                    && self.can_forward(dev_id, p, vlan)
                    && !self.isolation_blocks(dev_id, in_port, p) =>
            {
                vec![p]
            }
            Some(_) => vec![], // 已知单播但源同端口/不可转发/被隔离：丢弃
            None => self
                .logical_ports(dev_id)
                .into_iter()
                .filter(|&p| {
                    if from_fabric
                        && self
                            .net
                            .device(dev_id)
                            .and_then(|d| d.port(p))
                            .and_then(|p| p.lag)
                            .is_some()
                    {
                        // M-LAG peer-link 水平分割：peer 来的广播不再回送跨设备 LAG
                        return false;
                    }
                    p != logical_in
                        && self.can_forward(dev_id, p, vlan)
                        && !self.isolation_blocks(dev_id, in_port, p)
                })
                .collect(),
        };
        for p in out_ports {
            self.l2_egress(dev_id, p, vlan, frame);
        }
    }

    /// 端口隔离 / MUX VLAN：两个同 VLAN 端口之间是否禁止二层互通。
    fn isolation_blocks(&self, dev: DevId, a: PortId, b: PortId) -> bool {
        let Some(d) = self.net.device(dev) else {
            return false;
        };
        let (Some(pa), Some(pb)) = (d.port(a), d.port(b)) else {
            return false;
        };
        if let (Some(ra), Some(rb)) = (pa.mux_role, pb.mux_role) {
            match (ra, rb) {
                (MuxRole::Principal, _) | (_, MuxRole::Principal) => return false,
                (MuxRole::Separate, MuxRole::Separate) => return true,
                (MuxRole::Separate, MuxRole::Group(_)) | (MuxRole::Group(_), MuxRole::Separate) => {
                    return true;
                }
                (MuxRole::Group(x), MuxRole::Group(y)) => return x != y,
            }
        }
        matches!(
            (pa.isolate_group, pb.isolate_group),
            (Some(x), Some(y)) if x == y
        )
    }

    /// 出方向：按端口口型决定加/剥 tag 后发送；不发则静默跳过。
    fn l2_egress(&mut self, dev_id: DevId, port: PortId, vlan: VlanId, mut frame: EthernetFrame) {
        // 聚合组：按流哈希在选中成员间分担
        let port = self.egress_member(dev_id, port, &frame);
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

    /// 聚合组出方向选成员：同一流（源/目的 MAC）稳定落在同一成员。
    fn egress_member(&self, dev_id: DevId, port: PortId, frame: &EthernetFrame) -> PortId {
        let Some(d) = self.net.device(dev_id) else {
            return port;
        };
        let Some(gid) = d.port(port).and_then(|p| p.lag) else {
            return port;
        };
        let DeviceState::Switch(sw) = &d.state else {
            return port;
        };
        let Some(g) = sw.lags.get(&gid) else {
            return port;
        };
        let live: Vec<PortId> = g
            .selected
            .iter()
            .copied()
            .filter(|m| {
                d.port(*m)
                    .map(|p| p.admin_up && p.link.map(|l| self.net.link_up(l)).unwrap_or(false))
                    .unwrap_or(false)
            })
            .collect();
        if live.is_empty() {
            return port;
        }
        let mut h: u64 = 1469598103934665603;
        for b in frame.dst.iter().chain(frame.src.iter()) {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        live[(h % live.len() as u64) as usize]
    }

    /// 在指定 VLAN 内把帧送达已知 MAC（查表或洪泛）。
    fn l2_send_in_vlan(&mut self, dev_id: DevId, vlan: VlanId, dst: Mac, frame: EthernetFrame) {
        let learned = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            sw.mac_table.get(&(vlan, dst)).copied()
        };
        match learned {
            Some(p) => self.l2_egress(dev_id, p, vlan, frame),
            None => {
                for p in self.logical_ports(dev_id) {
                    if self.can_forward(dev_id, p, vlan) {
                        self.l2_egress(dev_id, p, vlan, frame);
                    }
                }
            }
        }
    }

    fn flood_in_vlan(&mut self, dev_id: DevId, vlan: VlanId, frame: EthernetFrame) {
        for p in self.logical_ports(dev_id) {
            if self.can_forward(dev_id, p, vlan) {
                self.l2_egress(dev_id, p, vlan, frame);
            }
        }
    }

    fn flood_in_vlan_except(
        &mut self,
        dev_id: DevId,
        vlan: VlanId,
        frame: EthernetFrame,
        exclude: PortId,
    ) {
        for p in self.logical_ports(dev_id) {
            if p != exclude && self.can_forward(dev_id, p, vlan) {
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
            L3::Tcp(tcp) => self.tcp_ingress(dev_id, in_port, tcp),
            L3::Ipsec(pkt) => self.ipsec_ingress(dev_id, in_port, pkt),
            L3::Capwap(pkt) => self.capwap_ingress(dev_id, pkt),
            L3::Vxlan(_) => {}
            L3::Evpn(_) => {}
            L3::Ospfv3(_) => {}
            L3::Srv6(_) => {}
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
            L3::Lacp(_) => {} // LACP 在二层路径已处理
            L3::Ospf(_) => {} // SVI 不跑 OSPF（简化）
            L3::Vrrp(_) => {} // 交换机不做 VRRP
            L3::Ipv6(_) => {} // IPv6 仅路由器/主机（简化）
            L3::Bgp(_) => {}  // 交换机不参与 BGP
            L3::Isis(_) => {} // 交换机不参与 IS-IS
            L3::Mpls(_) => {} // 交换机不处理 MPLS
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
        // OSPF/VRRP 使用组播，先于 MAC/单播判断处理
        if frame.ethertype == ETHERTYPE_OSPF {
            if let L3::Ospf(pkt) = frame.payload {
                self.ospf_on_packet(dev_id, port, pkt);
            }
            return;
        }
        if frame.ethertype == ETHERTYPE_VRRP {
            if let L3::Vrrp(pkt) = frame.payload {
                self.vrrp_on_packet(dev_id, pkt);
            }
            return;
        }
        if frame.ethertype == ETHERTYPE_IPV6 {
            match frame.payload {
                L3::Ospfv3(pkt) => self.ospfv3_on_packet(dev_id, port, pkt),
                L3::Srv6(pkt) => self.srv6_ingress(dev_id, port, pkt),
                _ => self.ipv6_on_frame(dev_id, port, frame),
            }
            return;
        }
        if frame.ethertype == ETHERTYPE_BGP {
            if let L3::Bgp(pkt) = frame.payload {
                self.on_bgp_packet(dev_id, port, pkt);
            }
            return;
        }
        if frame.ethertype == ETHERTYPE_ISIS {
            if let L3::Isis(pkt) = frame.payload {
                self.on_isis_packet(dev_id, port, pkt);
            }
            return;
        }
        if frame.ethertype == ETHERTYPE_MPLS {
            if let L3::Mpls(m) = frame.payload {
                self.mpls_on_frame(dev_id, port, m);
            }
            return;
        }
        let (my_mac, my_ip) = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            let Some(p) = d.port(port) else {
                return;
            };
            (p.mac, p.ip)
        };
        let vmac_ok = self.is_vrrp_virtual_mac_on(dev_id, port, &frame.dst);
        if frame.dst != my_mac && frame.dst != BROADCAST_MAC && !vmac_ok {
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
            L3::Tcp(tcp) => self.tcp_ingress(dev_id, port, tcp),
            L3::Ipsec(pkt) => self.ipsec_ingress(dev_id, port, pkt),
            L3::Capwap(pkt) => self.capwap_ingress(dev_id, pkt),
            L3::Vxlan(vx) => {
                // VXLAN 外层在 Underlay 路由器上按外层 IPv4 转发（广播 MAC 逐跳约定）
                let fwd = VxlanPacket {
                    ttl: vx.ttl.saturating_sub(1),
                    ..vx
                };
                self.encap_fwd_over_underlay(dev_id, port, vx.dst, vx.ttl, L3::Vxlan(fwd));
            }
            L3::Evpn(e) => {
                // EVPN 通告跨 Underlay 路由器转发
                self.encap_fwd_over_underlay(dev_id, port, e.dst, DEFAULT_TTL, L3::Evpn(e));
            }
            L3::Bpdu(_) => {}   // 主机/路由器不处理 802.1D BPDU
            L3::Lacp(_) => {}   // 主机/路由器不做链路聚合
            L3::Ospf(_) => {}   // OSPF 已在入口处理
            L3::Vrrp(_) => {}   // VRRP 已在入口处理
            L3::Ipv6(_) => {}   // IPv6 已在入口处理
            L3::Ospfv3(_) => {} // OSPFv3 已在入口处理
            L3::Srv6(_) => {}   // SRv6 已在入口处理
            L3::Bgp(_) => {}    // BGP 已在入口处理
            L3::Isis(_) => {}   // IS-IS 已在入口处理
            L3::Mpls(_) => {}   // MPLS 已在入口处理
            L3::Dhcp(dhcp) => {
                // DHCP 广播：主机客户端 / 路由器服务器各自处理
                match d_kind(&self.net, dev_id) {
                    DeviceKind::Host => self.dhcp_client_ingress(dev_id, port, dhcp),
                    DeviceKind::Router
                    | DeviceKind::Firewall
                    | DeviceKind::WlanController
                    | DeviceKind::LoadBalancer => self.dhcp_server_ingress(dev_id, port, dhcp),
                    DeviceKind::Switch | DeviceKind::AccessPoint => {}
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
        if arp.op == ArpOp::Request {
            // VRRP：Master 用虚拟 MAC 应答虚拟网关的 ARP
            if let Some(vmac) = self.vrrp_vip_owner(dev_id, port, arp.target_ip) {
                let reply = ArpPacket {
                    op: ArpOp::Reply,
                    sender_mac: vmac,
                    sender_ip: arp.target_ip,
                    target_mac: arp.sender_mac,
                    target_ip: arp.sender_ip,
                };
                let frame = EthernetFrame {
                    dst: arp.sender_mac,
                    src: vmac,
                    vlan: None,
                    ethertype: ETHERTYPE_ARP,
                    payload: L3::Arp(reply),
                };
                self.transmit_frame(dev_id, port, frame);
                return;
            }
            // F5 LTM：虚拟服务器 VIP 用本端口 MAC 应答 ARP
            if self.ltm_owns_vip(dev_id, arp.target_ip) {
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
                return;
            }
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
        if matches!(
            d_kind(&self.net, dev_id),
            DeviceKind::Router | DeviceKind::Firewall | DeviceKind::LoadBalancer
        ) {
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
        // 路由器/防火墙：NAT 入向还原优先（outside 口收到目的为出口 IP 的报文 → 还原目的后转发）
        if matches!(
            d_kind(&self.net, dev_id),
            DeviceKind::Router | DeviceKind::Firewall | DeviceKind::LoadBalancer
        ) {
            let vrf = self
                .net
                .device(dev_id)
                .and_then(|d| d.port(port))
                .and_then(|p| p.vrf);
            let mut pkt2 = pkt;
            if vrf.is_none() && self.nat_check_ingress(dev_id, port, &mut pkt2) {
                self.router_forward(dev_id, None, Some(port), pkt2);
                return;
            }
            let vip_mac = if vrf.is_none() {
                self.vrrp_vip_owner(dev_id, port, pkt.dst)
            } else {
                None
            };
            if Some(pkt.dst) == my_ip || vip_mac.is_some() {
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
                        src: vip_mac.unwrap_or(my_mac),
                        vlan: None,
                        ethertype: ETHERTYPE_IPV4,
                        payload: L3::Ipv4(reply),
                    };
                    self.transmit_frame(dev_id, port, out);
                }
                return;
            }
            self.router_forward(dev_id, vrf, Some(port), pkt);
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
        // SSL VPN 客户端：访问非本地网段时封装到网关
        let vpn_send = {
            let Some(d) = self.net.device(dev_id) else {
                return;
            };
            match (&d.state, d.port(port)) {
                (DeviceState::Host(hs), Some(p)) => {
                    let vpn = hs.vpn.clone();
                    let local = p.ip.map(|ip| (ip, p.prefix));
                    match (vpn, local) {
                        (Some(v), Some((ip, prefix))) => {
                            let mask = mask_from_prefix(prefix);
                            if (pkt.dst & mask) != (ip & mask) {
                                Some((v, ip))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        };
        if let Some((v, real_ip)) = vpn_send {
            let inner = Ipv4Packet {
                src: v.virtual_ip,
                dst: pkt.dst,
                ttl: pkt.ttl,
                icmp: pkt.icmp,
            };
            let outer = IpsecPacket {
                src: real_ip,
                dst: v.gateway,
                ttl: DEFAULT_TTL,
                inner,
            };
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: src_mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV4,
                payload: L3::Ipsec(outer),
            };
            self.transmit_frame(dev_id, port, frame);
            return;
        }
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

    fn router_forward(
        &mut self,
        dev_id: DevId,
        vrf: Option<u16>,
        in_port: Option<PortId>,
        mut pkt: Ipv4Packet,
    ) {
        if pkt.ttl <= 1 {
            self.dropped += 1;
            return;
        }
        pkt.ttl -= 1;
        // IPsec 站点隧道：命中保护子网则封装（教学用 IP-in-IP）
        if vrf.is_none() && self.ipsec_encapsulate(dev_id, pkt) {
            return;
        }
        // SSL VPN 回程：目的为虚拟 IP 时封装回客户端
        if vrf.is_none() && self.sslvpn_encapsulate(dev_id, pkt) {
            return;
        }
        // VRF 入口：PE 的 VPN 路由下一跳位于全局 MPLS 域，先尝试压签
        if vrf.is_some() && self.mpls_push(dev_id, pkt, vrf) {
            return;
        }
        let Some((port, nexthop, _vlan)) = self.net.lookup_route_vrf(dev_id, vrf, pkt.dst) else {
            self.dropped += 1;
            return;
        };
        if let Some(ip) = in_port {
            if !self.firewall_allow(dev_id, ip, port, &pkt) {
                self.dropped += 1;
                return;
            }
        }
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
        // NAT 出向改写（EasyIP/静态；VRF 内不做 NAT）
        if vrf.is_none() {
            self.nat_egress(dev_id, port, &mut pkt);
        }
        // MPLS 入 LSP：命中 FEC 则压签发送
        if self.mpls_push(dev_id, pkt, None) {
            return;
        }
        self.device_send_ip(dev_id, port, pkt, nexthop);
    }

    /// 防火墙区域策略：同区域/未划分区域放行；跨区域按策略首条匹配，
    /// 无命中默认拒绝；已放行的 ICMP 会话允许反向回包（30s）。
    fn firewall_allow(
        &mut self,
        dev_id: DevId,
        in_port: PortId,
        out_port: PortId,
        pkt: &Ipv4Packet,
    ) -> bool {
        let (in_zone, out_zone, in_level, out_level) = {
            let Some(d) = self.net.device(dev_id) else {
                return true;
            };
            if d.kind != DeviceKind::Firewall {
                return true;
            }
            let z = |p: PortId| d.port(p).and_then(|p| p.zone.clone());
            let l = |p: PortId| d.port(p).and_then(|p| p.security_level);
            (z(in_port), z(out_port), l(in_port), l(out_port))
        };
        let now = self.now;
        let Some(d) = self.net.device_mut(dev_id) else {
            return true;
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return true;
        };
        rs.security_sessions.retain(|s| {
            now.saturating_sub(s.last_ns) < 30_000_000_000
                && !(s.src == pkt.src && s.dst == pkt.dst && s.icmp_id == pkt.icmp.id)
        });
        let reverse_session = rs
            .security_sessions
            .iter()
            .any(|s| s.src == pkt.dst && s.dst == pkt.src && s.icmp_id == pkt.icmp.id);
        if let (Some(in_zone), Some(out_zone)) = (in_zone, out_zone) {
            if in_zone == out_zone || reverse_session {
                return true;
            }
            let Some(policy) = rs
                .security_policies
                .iter()
                .find(|p| p.src_zone == in_zone && p.dst_zone == out_zone)
            else {
                return false;
            };
            if !policy.permit {
                return false;
            }
        } else if let (Some(il), Some(ol)) = (in_level, out_level) {
            // ASA 默认：高安全级别 → 低安全级别放行，低 → 高默认拒绝（回包走会话）
            if il < ol && !reverse_session {
                return false;
            }
        } else {
            return true;
        }
        rs.security_sessions.push(SecuritySession {
            src: pkt.src,
            dst: pkt.dst,
            icmp_id: pkt.icmp.id,
            last_ns: now,
        });
        true
    }

    fn firewall_tcp_allow(
        &mut self,
        dev_id: DevId,
        in_port: PortId,
        out_port: PortId,
        tcp: &TcpPacket,
    ) -> bool {
        let (in_zone, out_zone, in_level, out_level) = {
            let Some(d) = self.net.device(dev_id) else {
                return true;
            };
            if d.kind != DeviceKind::Firewall {
                return true;
            }
            let z = |p: PortId| d.port(p).and_then(|p| p.zone.clone());
            let l = |p: PortId| d.port(p).and_then(|p| p.security_level);
            (z(in_port), z(out_port), l(in_port), l(out_port))
        };
        let now = self.now;
        let Some(d) = self.net.device_mut(dev_id) else {
            return true;
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return true;
        };
        rs.tcp_flows
            .retain(|f| now.saturating_sub(f.last_ns) < 60_000_000_000);
        let existing = rs.tcp_flows.iter_mut().find(|f| {
            (f.client == tcp.src
                && f.server == tcp.dst
                && f.client_port == tcp.src_port
                && f.server_port == tcp.dst_port)
                || (f.server == tcp.src
                    && f.client == tcp.dst
                    && f.server_port == tcp.src_port
                    && f.client_port == tcp.dst_port)
        });
        if let Some(flow) = existing {
            flow.last_ns = now;
            if tcp.flags & TCP_RST != 0 {
                flow.state = TcpFlowState::Closed;
            } else if tcp.flags & TCP_FIN != 0 {
                flow.state = TcpFlowState::FinWait;
            } else if (tcp.flags & TCP_SYN != 0 && tcp.flags & TCP_ACK != 0)
                || (tcp.flags & TCP_ACK != 0 && flow.state == TcpFlowState::SynSent)
            {
                flow.state = TcpFlowState::Established;
            }
            return true;
        }
        let is_syn = tcp.flags & TCP_SYN != 0 && tcp.flags & TCP_ACK == 0;
        let permitted = if let (Some(in_zone), Some(out_zone)) = (in_zone, out_zone) {
            if in_zone == out_zone {
                true
            } else {
                rs.security_policies
                    .iter()
                    .find(|p| p.src_zone == in_zone && p.dst_zone == out_zone)
                    .map(|p| p.permit)
                    .unwrap_or(false)
            }
        } else if let (Some(il), Some(ol)) = (in_level, out_level) {
            il >= ol
        } else {
            true
        };
        if !permitted || !is_syn {
            return false;
        }
        rs.tcp_flows.push(TcpFlow {
            client: tcp.src,
            server: tcp.dst,
            client_port: tcp.src_port,
            server_port: tcp.dst_port,
            state: TcpFlowState::SynSent,
            last_ns: now,
        });
        true
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
            DeviceKind::Router
                | DeviceKind::Switch
                | DeviceKind::Host
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::AccessPoint
                | DeviceKind::LoadBalancer
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

    // ---------- TCP 握手/挥手（教学简化，无应用数据） ----------

    pub fn start_tcp_probe(&mut self, dev: DevId, target: Ipv4Addr, port: u16) -> Option<u16> {
        if d_kind(&self.net, dev) != DeviceKind::Host {
            return None;
        }
        let (id, src_ip) = {
            let src_ip = self
                .net
                .device(dev)
                .and_then(|d| d.port(0))
                .and_then(|p| p.ip)?;
            let d = self.net.device_mut(dev)?;
            let DeviceState::Host(hs) = &mut d.state else {
                return None;
            };
            let id = hs.next_tcp_id;
            hs.next_tcp_id = hs.next_tcp_id.wrapping_add(1);
            hs.tcp_probes.push(TcpProbe {
                id,
                target,
                target_port: port,
                src_port: 40000 + id,
                state: TcpProbeState::SynSent,
                opened: None,
                reported: false,
            });
            (id, src_ip)
        };
        let tcp = TcpPacket {
            src: src_ip,
            dst: target,
            ttl: DEFAULT_TTL,
            src_port: 40000 + id,
            dst_port: port,
            flags: TCP_SYN,
            seq: 1000,
            ack: 0,
        };
        self.send_tcp_broadcast(dev, 0, tcp);
        Some(id)
    }

    pub fn take_unreported_tcp(&mut self, dev: DevId) -> Vec<TcpProbe> {
        let Some(d) = self.net.device_mut(dev) else {
            return vec![];
        };
        let DeviceState::Host(hs) = &mut d.state else {
            return vec![];
        };
        let mut out = vec![];
        for p in hs.tcp_probes.iter_mut() {
            if !p.reported {
                p.reported = true;
                out.push(p.clone());
            }
        }
        hs.tcp_probes.retain(|p| !p.reported);
        out
    }

    fn send_tcp_broadcast(&mut self, dev: DevId, port: PortId, tcp: TcpPacket) {
        let Some(mac) = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .map(|p| p.mac)
        else {
            return;
        };
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: mac,
            vlan: None,
            ethertype: ETHERTYPE_IPV4,
            payload: L3::Tcp(tcp),
        };
        self.transmit_frame(dev, port, frame);
    }

    fn tcp_ingress(&mut self, dev: DevId, port: PortId, tcp: TcpPacket) {
        let my_ip = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .and_then(|p| p.ip);
        if Some(tcp.dst) == my_ip {
            match d_kind(&self.net, dev) {
                DeviceKind::Host => self.host_tcp_ingress(dev, port, tcp),
                DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::Switch
                | DeviceKind::AccessPoint
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer => {
                    let rst = TcpPacket {
                        src: tcp.dst,
                        dst: tcp.src,
                        ttl: DEFAULT_TTL,
                        src_port: tcp.dst_port,
                        dst_port: tcp.src_port,
                        flags: TCP_RST | TCP_ACK,
                        seq: tcp.ack,
                        ack: tcp.seq.wrapping_add(1),
                    };
                    self.send_tcp_broadcast(dev, port, rst);
                }
            }
            return;
        }
        match d_kind(&self.net, dev) {
            DeviceKind::Router
            | DeviceKind::Firewall
            | DeviceKind::Switch
            | DeviceKind::AccessPoint
            | DeviceKind::WlanController
            | DeviceKind::LoadBalancer => self.tcp_forward(dev, port, tcp),
            DeviceKind::Host => {}
        }
    }

    fn host_tcp_ingress(&mut self, dev: DevId, port: PortId, tcp: TcpPacket) {
        let is_client = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Host(hs) = &d.state else {
                return;
            };
            hs.tcp_probes.iter().any(|p| {
                p.target == tcp.src && p.target_port == tcp.src_port && p.src_port == tcp.dst_port
            })
        };
        if is_client {
            let mut out: Vec<TcpPacket> = vec![];
            if let Some(d) = self.net.device_mut(dev) {
                let DeviceState::Host(hs) = &mut d.state else {
                    return;
                };
                let Some(p) = hs.tcp_probes.iter_mut().find(|p| {
                    p.target == tcp.src
                        && p.target_port == tcp.src_port
                        && p.src_port == tcp.dst_port
                }) else {
                    return;
                };
                if tcp.flags & TCP_RST != 0 {
                    p.opened = Some(false);
                    p.state = TcpProbeState::Closed;
                } else if tcp.flags & TCP_SYN != 0 && tcp.flags & TCP_ACK != 0 {
                    p.opened = Some(true);
                    p.state = TcpProbeState::Established;
                    out.push(TcpPacket {
                        src: tcp.dst,
                        dst: tcp.src,
                        ttl: DEFAULT_TTL,
                        src_port: tcp.dst_port,
                        dst_port: tcp.src_port,
                        flags: TCP_ACK,
                        seq: tcp.ack,
                        ack: tcp.seq.wrapping_add(1),
                    });
                    out.push(TcpPacket {
                        src: tcp.dst,
                        dst: tcp.src,
                        ttl: DEFAULT_TTL,
                        src_port: tcp.dst_port,
                        dst_port: tcp.src_port,
                        flags: TCP_ACK | TCP_FIN,
                        seq: tcp.ack,
                        ack: tcp.seq.wrapping_add(1),
                    });
                    p.state = TcpProbeState::FinWait;
                } else if tcp.flags & TCP_FIN != 0 {
                    p.state = TcpProbeState::Closed;
                    out.push(TcpPacket {
                        src: tcp.dst,
                        dst: tcp.src,
                        ttl: DEFAULT_TTL,
                        src_port: tcp.dst_port,
                        dst_port: tcp.src_port,
                        flags: TCP_ACK,
                        seq: tcp.ack,
                        ack: tcp.seq.wrapping_add(1),
                    });
                }
            }
            for pkt in out {
                self.send_tcp_broadcast(dev, port, pkt);
            }
            return;
        }
        // 服务端：监听端口回 SYN-ACK，未监听端口回 RST；FIN 回 ACK+FIN
        let listening = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Host(hs) = &d.state else {
                return;
            };
            hs.tcp_listeners.contains(&tcp.dst_port)
        };
        let reply = if tcp.flags & TCP_SYN != 0 && tcp.flags & TCP_ACK == 0 && !listening {
            Some(TcpPacket {
                src: tcp.dst,
                dst: tcp.src,
                ttl: DEFAULT_TTL,
                src_port: tcp.dst_port,
                dst_port: tcp.src_port,
                flags: TCP_RST | TCP_ACK,
                seq: 0,
                ack: tcp.seq.wrapping_add(1),
            })
        } else if tcp.flags & TCP_SYN != 0 && tcp.flags & TCP_ACK == 0 {
            Some(TcpPacket {
                src: tcp.dst,
                dst: tcp.src,
                ttl: DEFAULT_TTL,
                src_port: tcp.dst_port,
                dst_port: tcp.src_port,
                flags: TCP_SYN | TCP_ACK,
                seq: 2000,
                ack: tcp.seq.wrapping_add(1),
            })
        } else if tcp.flags & TCP_FIN != 0 {
            Some(TcpPacket {
                src: tcp.dst,
                dst: tcp.src,
                ttl: DEFAULT_TTL,
                src_port: tcp.dst_port,
                dst_port: tcp.src_port,
                flags: TCP_ACK | TCP_FIN,
                seq: tcp.ack,
                ack: tcp.seq.wrapping_add(1),
            })
        } else {
            None
        };
        if let Some(reply) = reply {
            self.send_tcp_broadcast(dev, port, reply);
        }
    }

    fn tcp_forward(&mut self, dev: DevId, in_port: PortId, mut tcp: TcpPacket) {
        if tcp.ttl <= 1 {
            self.dropped += 1;
            return;
        }
        tcp.ttl -= 1;
        if d_kind(&self.net, dev) == DeviceKind::LoadBalancer
            && self.ltm_translate(dev, &mut tcp).is_none()
        {
            self.dropped += 1;
            return;
        }
        let vrf = self
            .net
            .device(dev)
            .and_then(|d| d.port(in_port))
            .and_then(|p| p.vrf);
        let Some((out_port, _nh, _vlan)) = self.net.lookup_route_vrf(dev, vrf, tcp.dst) else {
            self.dropped += 1;
            return;
        };
        if !self.firewall_tcp_allow(dev, in_port, out_port, &tcp) {
            self.dropped += 1;
            return;
        }
        self.send_tcp_broadcast(dev, out_port, tcp);
    }

    // ---------- IPsec 站点隧道（教学 IP-in-IP） ----------

    fn ipsec_encapsulate(&mut self, dev: DevId, pkt: Ipv4Packet) -> bool {
        let tunnel = {
            let Some(d) = self.net.device(dev) else {
                return false;
            };
            let DeviceState::Router(rs) = &d.state else {
                return false;
            };
            rs.ipsec_tunnels.iter().find(|t| {
                (pkt.src & mask_from_prefix(t.local_subnet.1)) == t.local_subnet.0
                    && (pkt.dst & mask_from_prefix(t.remote_subnet.1)) == t.remote_subnet.0
            })
        }
        .cloned();
        let Some(t) = tunnel else { return false };
        let Some((port, _, _)) = self.net.lookup_route(dev, t.remote) else {
            self.dropped += 1;
            return true;
        };
        let tunneled = IpsecPacket {
            src: t.local,
            dst: t.remote,
            ttl: DEFAULT_TTL,
            inner: pkt,
        };
        if let Some(mac) = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .map(|p| p.mac)
        {
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV4,
                payload: L3::Ipsec(tunneled),
            };
            self.transmit_frame(dev, port, frame);
        }
        true
    }

    fn ipsec_ingress(&mut self, dev: DevId, _port: PortId, pkt: IpsecPacket) {
        let local = self
            .net
            .device(dev)
            .map(|d| {
                matches!(d.state, DeviceState::Router(_))
                    && d.ports.iter().any(|p| p.ip == Some(pkt.dst))
            })
            .unwrap_or(false);
        if local {
            self.router_forward(dev, None, None, pkt.inner);
            return;
        }
        // SSL VPN 客户端解封装：目的为本机时直接处理内层 ICMP 回包
        if d_kind(&self.net, dev) == DeviceKind::Host {
            let mine = self
                .net
                .device(dev)
                .and_then(|d| d.port(0))
                .and_then(|p| p.ip);
            if Some(pkt.dst) == mine && pkt.inner.icmp.is_reply {
                self.on_ping_reply(dev, pkt.inner);
            }
            return;
        }
        let Some(d) = self.net.device(dev) else {
            return;
        };
        if !matches!(
            d.kind,
            DeviceKind::Router | DeviceKind::Firewall | DeviceKind::Switch
        ) {
            return;
        }
        if pkt.ttl <= 1 {
            self.dropped += 1;
            return;
        }
        let mut outer = pkt;
        outer.ttl -= 1;
        let Some((out_port, _, _)) = self.net.lookup_route(dev, outer.dst) else {
            self.dropped += 1;
            return;
        };
        let Some(mac) = self
            .net
            .device(dev)
            .and_then(|d| d.port(out_port))
            .map(|p| p.mac)
        else {
            return;
        };
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: mac,
            vlan: None,
            ethertype: ETHERTYPE_IPV4,
            payload: L3::Ipsec(outer),
        };
        self.transmit_frame(dev, out_port, frame);
    }

    // ---------- SSL VPN（远程接入简化：认证 + 虚拟 IP + IP-in-IP 隧道） ----------

    pub fn sslvpn_connect(
        &mut self,
        pc: DevId,
        gateway_ip: Ipv4Addr,
        user: &str,
        pass: &str,
    ) -> Result<Ipv4Addr, String> {
        if d_kind(&self.net, pc) != DeviceKind::Host {
            return Err("SSL VPN client must be a host".into());
        }
        let Some(gw) = self.mgmt_ip_owner(gateway_ip) else {
            return Err("SSL VPN gateway not found".into());
        };
        let (pool, real_ip) = {
            let Some(d) = self.net.device(gw) else {
                return Err("SSL VPN gateway not found".into());
            };
            let DeviceState::Router(rs) = &d.state else {
                return Err("SSL VPN gateway not found".into());
            };
            let Some(pool) = rs.sslvpn_pool else {
                return Err("SSL VPN virtual-ip pool is not configured".into());
            };
            if !d
                .mgmt
                .users
                .iter()
                .any(|u| u.name == user && u.secret == pass)
            {
                return Err("authentication failed".into());
            }
            let real_ip = self
                .net
                .device(pc)
                .and_then(|d| d.port(0))
                .and_then(|p| p.ip)
                .ok_or("client has no IP")?;
            (pool, real_ip)
        };
        let virtual_ip = {
            let Some(d) = self.net.device_mut(gw) else {
                return Err("SSL VPN gateway not found".into());
            };
            let DeviceState::Router(rs) = &mut d.state else {
                return Err("SSL VPN gateway not found".into());
            };
            if let Some((v, _)) = rs.sslvpn_clients.iter().find(|(_, r)| *r == real_ip) {
                *v
            } else {
                let base = pool.0;
                let host = 2 + rs.sslvpn_clients.len() as u32;
                // pool.1 过小（如 /0）会使位移越界，clamp 到合理池范围
                if host >= (1u32 << (32 - pool.1.clamp(1, 24))) - 1 {
                    return Err("SSL VPN pool exhausted".into());
                }
                let v = base + host;
                rs.sslvpn_clients.push((v, real_ip));
                v
            }
        };
        if let Some(d) = self.net.device_mut(pc) {
            if let DeviceState::Host(hs) = &mut d.state {
                hs.vpn = Some(VpnSession {
                    gateway: gateway_ip,
                    virtual_ip,
                });
            }
        }
        Ok(virtual_ip)
    }

    fn sslvpn_encapsulate(&mut self, dev: DevId, pkt: Ipv4Packet) -> bool {
        let client = {
            let Some(d) = self.net.device(dev) else {
                return false;
            };
            let DeviceState::Router(rs) = &d.state else {
                return false;
            };
            let Some((net, len)) = rs.sslvpn_pool else {
                return false;
            };
            if (pkt.dst & mask_from_prefix(len)) != net {
                return false;
            }
            rs.sslvpn_clients
                .iter()
                .find(|(v, _)| *v == pkt.dst)
                .map(|(_, real)| *real)
        };
        let Some(real_ip) = client else {
            return false;
        };
        let Some((port, _, _)) = self.net.lookup_route(dev, real_ip) else {
            self.dropped += 1;
            return true;
        };
        let Some(src_ip) = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .and_then(|p| p.ip)
        else {
            return false;
        };
        let outer = IpsecPacket {
            src: src_ip,
            dst: real_ip,
            ttl: DEFAULT_TTL,
            inner: pkt,
        };
        if let Some(mac) = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .map(|p| p.mac)
        {
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV4,
                payload: L3::Ipsec(outer),
            };
            self.transmit_frame(dev, port, frame);
        }
        true
    }

    pub fn sslvpn_clients_of(&self, dev: DevId) -> Vec<(Ipv4Addr, Ipv4Addr)> {
        match self.net.device(dev).map(|d| &d.state) {
            Some(DeviceState::Router(rs)) => rs.sslvpn_clients.clone(),
            _ => vec![],
        }
    }

    // ---------- WLAN / CAPWAP（AP 注册 + SSID/WPA2 + 二层桥接） ----------

    fn local_ip_of(&self, dev: DevId) -> Option<Ipv4Addr> {
        let d = self.net.device(dev)?;
        match &d.state {
            DeviceState::Router(_) => d.ports.iter().find_map(|p| p.ip),
            DeviceState::Switch(sw) => sw
                .svi
                .values()
                .find(|s| s.admin_up)
                .map(|s| s.ip)
                .or_else(|| sw.svi.values().next().map(|s| s.ip)),
            DeviceState::Host(_) => d.port(0).and_then(|p| p.ip),
        }
    }

    fn ap_uplink(&self, dev: DevId) -> Option<PortId> {
        let d = self.net.device(dev)?;
        d.ports
            .iter()
            .find(|p| p.link.is_some() && p.mode == PortMode::Trunk)
            .or_else(|| d.ports.iter().find(|p| p.link.is_some()))
            .map(|p| p.idx)
    }

    fn capwap_send(&mut self, dev: DevId, dst: Ipv4Addr, pkt: CapwapPacket) {
        let Some(src) = self.local_ip_of(dev) else {
            return;
        };
        let Some(port) = (match d_kind(&self.net, dev) {
            DeviceKind::AccessPoint => self.ap_uplink(dev),
            _ => self.net.lookup_route(dev, dst).map(|(p, _, _)| p),
        }) else {
            return;
        };
        let Some(mac) = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .map(|p| p.mac)
        else {
            return;
        };
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: mac,
            vlan: None,
            ethertype: ETHERTYPE_IPV4,
            payload: L3::Capwap(CapwapPacket { src, ..pkt }),
        };
        self.transmit_frame(dev, port, frame);
    }

    /// AP 发送 CAPWAP Join（由 `capwap ac` 命令触发）。
    pub fn capwap_start(&mut self, ap: DevId) {
        let Some(ac_ip) = self.net.device(ap).and_then(|d| d.wlan.capwap_ac) else {
            return;
        };
        let pkt = CapwapPacket {
            kind: CapwapKind::Join,
            src: 0,
            dst: ac_ip,
            ssid: [0; 32],
            psk: [0; 32],
            vlan: 0,
            security: 0,
        };
        self.capwap_send(ap, ac_ip, pkt);
    }

    /// AC 配置变化：向已加入的 AP 重新下发 WLAN 配置（先 Reset 再逐个 profile）。
    pub fn wlan_config_changed(&mut self, ac: DevId) {
        let Some(ac_ip) = self.local_ip_of(ac) else {
            return;
        };
        let profiles = self
            .net
            .device(ac)
            .map(|d| d.wlan.profiles.clone())
            .unwrap_or_default();
        let aps: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::AccessPoint && d.wlan.ac_ip == Some(ac_ip))
            .map(|d| d.id)
            .collect();
        for ap in aps {
            let Some(ap_ip) = self.local_ip_of(ap) else {
                continue;
            };
            self.capwap_send_config(ac, ap_ip, &profiles);
        }
    }

    fn capwap_send_config(&mut self, ac: DevId, ap_ip: Ipv4Addr, profiles: &[WlanProfile]) {
        let reset = CapwapPacket {
            kind: CapwapKind::ConfigReset,
            src: 0,
            dst: ap_ip,
            ssid: [0; 32],
            psk: [0; 32],
            vlan: 0,
            security: 0,
        };
        self.capwap_send(ac, ap_ip, reset);
        for p in profiles {
            let pkt = CapwapPacket {
                kind: CapwapKind::Config,
                src: 0,
                dst: ap_ip,
                ssid: sim_packets::fixed_str(&p.ssid),
                psk: sim_packets::fixed_str(p.psk.as_deref().unwrap_or("")),
                vlan: p.vlan,
                security: match p.security {
                    WlanSecurity::Open => 0,
                    WlanSecurity::Wpa2Psk => 1,
                    WlanSecurity::Wpa3Sae => 2,
                },
            };
            self.capwap_send(ac, ap_ip, pkt);
        }
    }

    fn capwap_ingress(&mut self, dev: DevId, pkt: CapwapPacket) {
        match d_kind(&self.net, dev) {
            DeviceKind::WlanController if pkt.dst == self.local_ip_of(dev).unwrap_or(0) => {
                let profiles = self
                    .net
                    .device(dev)
                    .map(|d| d.wlan.profiles.clone())
                    .unwrap_or_default();
                self.capwap_send_config(dev, pkt.src, &profiles);
            }
            DeviceKind::AccessPoint if pkt.kind == CapwapKind::ConfigReset => {
                if let Some(d) = self.net.device_mut(dev) {
                    d.wlan.profiles.clear();
                    d.wlan.ac_ip = Some(pkt.src);
                    d.wlan.joined = false;
                }
            }
            DeviceKind::AccessPoint if pkt.kind == CapwapKind::Config => {
                let ssid = sim_packets::fixed_str_value(&pkt.ssid);
                if ssid.is_empty() {
                    return;
                }
                let psk = sim_packets::fixed_str_value(&pkt.psk);
                let security = match pkt.security {
                    1 => WlanSecurity::Wpa2Psk,
                    2 => WlanSecurity::Wpa3Sae,
                    _ => WlanSecurity::Open,
                };
                if let Some(d) = self.net.device_mut(dev) {
                    if let DeviceState::Switch(sw) = &mut d.state {
                        sw.ensure_vlan(pkt.vlan);
                    }
                    let profile = WlanProfile {
                        ssid: ssid.clone(),
                        security,
                        psk: if psk.is_empty() { None } else { Some(psk) },
                        vlan: pkt.vlan,
                    };
                    match d.wlan.profiles.iter_mut().find(|p| p.ssid == ssid) {
                        Some(p) => *p = profile,
                        None => d.wlan.profiles.push(profile),
                    }
                    d.wlan.ac_ip = Some(pkt.src);
                    d.wlan.joined = true;
                }
            }
            _ => {}
        }
    }

    fn ap_client_allowed(&self, dev: DevId, port: PortId) -> bool {
        let Some(d) = self.net.device(dev) else {
            return true;
        };
        if d.kind != DeviceKind::AccessPoint {
            return true;
        }
        let Some(link) = d.port(port).and_then(|p| p.link) else {
            return true;
        };
        let Some(l) = self.net.link(link) else {
            return true;
        };
        let peer = if l.a.0 == dev { l.b.0 } else { l.a.0 };
        match self.net.device(peer).map(|x| &x.state) {
            Some(DeviceState::Host(hs)) => hs.wifi.as_ref().map(|w| w.ap == dev).unwrap_or(false),
            _ => true,
        }
    }

    /// PC 无线关联：直连 AP、SSID 匹配、WPA2 口令正确。
    pub fn wifi_connect(&mut self, pc: DevId, ssid: &str, password: &str) -> Result<DevId, String> {
        let candidates = self.wifi_candidates(ssid);
        let Some((ap, profile)) = candidates.first().cloned() else {
            return Err("SSID not found or AP not joined".into());
        };
        self.check_wlan_password(&profile, password)?;
        self.wifi_associate(pc, ap, ssid)?;
        Ok(ap)
    }

    /// 漫游：切换到另一个同 SSID 的 AP（高优先级优先）。
    pub fn wifi_roam(&mut self, pc: DevId) -> Result<DevId, String> {
        let Some((current, ssid)) = self.net.device(pc).and_then(|d| match &d.state {
            DeviceState::Host(hs) => hs.wifi.as_ref().map(|w| (w.ap, w.ssid.clone())),
            _ => None,
        }) else {
            return Err("not associated".into());
        };
        let next = self
            .wifi_candidates(&ssid)
            .into_iter()
            .find(|(ap, _)| *ap != current);
        let Some((ap, _profile)) = next else {
            return Err("no other AP with the same SSID".into());
        };
        self.wifi_associate(pc, ap, &ssid)?;
        Ok(ap)
    }

    fn wifi_candidates(&self, ssid: &str) -> Vec<(DevId, WlanProfile)> {
        let mut out: Vec<(DevId, u8, WlanProfile)> = self
            .net
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::AccessPoint && d.wlan.joined)
            .filter_map(|d| {
                d.wlan
                    .profile(ssid)
                    .map(|p| (d.id, d.wlan.priority, p.clone()))
            })
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.into_iter().map(|(id, _, p)| (id, p)).collect()
    }

    fn check_wlan_password(&self, profile: &WlanProfile, password: &str) -> Result<(), String> {
        match profile.security {
            WlanSecurity::Open => Ok(()),
            WlanSecurity::Wpa2Psk if profile.psk.as_deref() == Some(password) => Ok(()),
            WlanSecurity::Wpa3Sae if profile.psk.as_deref() == Some(password) => Ok(()),
            WlanSecurity::Wpa2Psk => Err("invalid WPA2 password".into()),
            WlanSecurity::Wpa3Sae => Err("invalid WPA3 password".into()),
        }
    }

    fn wifi_associate(&mut self, pc: DevId, ap: DevId, ssid: &str) -> Result<(), String> {
        let Some(d) = self.net.device(pc) else {
            return Err("device not found".into());
        };
        if d.kind != DeviceKind::Host {
            return Err("wifi client must be a host".into());
        }
        let Some(profile) = self
            .net
            .device(ap)
            .and_then(|d| d.wlan.profile(ssid).cloned())
        else {
            return Err("SSID not found".into());
        };
        if let Some(link) = d.port(0).and_then(|p| p.link) {
            self.net.remove_link(link);
        }
        let link = self.net.add_link(pc, ap).map_err(|_| "no free AP port")?;
        let (_, ap_port) = {
            let Some(l) = self.net.link(link) else {
                return Err("link error".into());
            };
            if l.a.0 == ap {
                (l.a.0, l.a.1)
            } else {
                (l.b.0, l.b.1)
            }
        };
        if let Some(d) = self.net.device_mut(ap) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.ensure_vlan(profile.vlan);
            }
            if let Some(p) = d.port_mut(ap_port) {
                p.mode = PortMode::Access;
                p.pvid = profile.vlan;
                p.admin_up = true;
            }
        }
        if let Some(d) = self.net.device_mut(pc) {
            if let DeviceState::Host(hs) = &mut d.state {
                hs.wifi = Some(WifiSession {
                    ap,
                    ssid: ssid.to_string(),
                });
            }
        }
        Ok(())
    }

    /// 供 show 使用：AC 下挂 AP（名称、管理 IP、是否已加入）。
    pub fn wlan_aps_of(&self, ac: DevId) -> Vec<(String, Option<Ipv4Addr>, bool)> {
        let Some(ac_ip) = self.local_ip_of(ac) else {
            return vec![];
        };
        self.net
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::AccessPoint && d.wlan.ac_ip == Some(ac_ip))
            .map(|d| (d.name.clone(), self.local_ip_of(d.id), d.wlan.joined))
            .collect()
    }

    // ---------- VXLAN（EVPN 数据面简化：静态 VTEP 对端 + VNI） ----------

    fn vxlan_encap(&mut self, dev: DevId, in_port: PortId, vlan: VlanId, frame: &EthernetFrame) {
        let (local, vni, peers, known) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            let Some(local) = sw.vxlan.local else {
                return;
            };
            let Some((vni, _)) = sw.vxlan.mappings.iter().find(|(_, v)| *v == vlan) else {
                return;
            };
            let peers = sw.vxlan.peers.clone();
            let known = sw.mac_table.contains_key(&(vlan, frame.dst));
            (local, *vni, peers, known)
        };
        if peers.is_empty() || known {
            return;
        }
        let payload = match frame.payload {
            L3::Ipv4(p) => VxlanPayload::Ipv4(p),
            L3::Arp(a) => VxlanPayload::Arp(a),
            _ => return,
        };
        for peer in peers {
            let Some((port, nh, out_vlan)) = self.net.lookup_route(dev, peer) else {
                continue;
            };
            // 经静态路由的 peer：路由表项本身无 VLAN，按下一跳所在的直连网段解析出发送 VLAN
            let out_vlan =
                out_vlan.or_else(|| self.net.lookup_route(dev, nh).and_then(|(_, _, v)| v));
            let src_mac = out_vlan.map(svi_mac).unwrap_or_else(|| {
                self.net
                    .device(dev)
                    .and_then(|d| d.port(port))
                    .map(|p| p.mac)
                    .unwrap_or([0; 6])
            });
            let vx = VxlanPacket {
                src: local,
                dst: peer,
                ttl: DEFAULT_TTL,
                vni,
                inner_src: frame.src,
                inner_dst: frame.dst,
                inner_vlan: frame.vlan,
                payload,
            };
            let outer = EthernetFrame {
                dst: BROADCAST_MAC,
                src: src_mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV4,
                payload: L3::Vxlan(vx),
            };
            match out_vlan {
                Some(v) => self.flood_in_vlan_except(dev, v, outer, in_port),
                None if port != in_port => self.transmit_frame(dev, port, outer),
                None => {}
            }
        }
    }

    /// Underlay 路由器转发 VXLAN/EVPN 封装帧（与 IPsec/TCP 相同的广播 MAC 逐跳约定）：
    /// 按外层单播 v4 查表转发，出口≠入口，VXLAN TTL 递减。
    /// 返回 true 表示已处理（转发或丢弃）；false 表示留给本机 VTEP 解封装。
    fn encap_fwd_over_underlay(
        &mut self,
        dev: DevId,
        in_port: PortId,
        dst: Ipv4Addr,
        ttl: u8,
        payload: L3,
    ) -> bool {
        if !matches!(
            d_kind(&self.net, dev),
            DeviceKind::Router | DeviceKind::Firewall
        ) {
            return false;
        }
        if ttl <= 1 {
            self.dropped += 1;
            return true;
        }
        let Some((out_port, _, _)) = self.net.lookup_route(dev, dst) else {
            self.dropped += 1;
            return true;
        };
        if out_port == in_port {
            return true;
        }
        let Some(d) = self.net.device(dev) else {
            return true;
        };
        let Some(p) = d.port(out_port) else {
            return true;
        };
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: p.mac,
            vlan: None,
            ethertype: ETHERTYPE_IPV4,
            payload,
        };
        self.transmit_frame(dev, out_port, frame);
        true
    }

    fn vxlan_on_frame(&mut self, dev: DevId, in_port: PortId, vx: VxlanPacket) {
        let (local, vlan) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            let Some(local) = sw.vxlan.local else {
                return;
            };
            let Some((_, vlan)) = sw.vxlan.mappings.iter().find(|(v, _)| *v == vx.vni) else {
                self.dropped += 1;
                return;
            };
            (local, *vlan)
        };
        if vx.dst != local && vx.dst != 0xffff_ffff {
            return;
        }
        let (ethertype, payload) = match vx.payload {
            VxlanPayload::Ipv4(p) => (ETHERTYPE_IPV4, L3::Ipv4(p)),
            VxlanPayload::Arp(a) => (ETHERTYPE_ARP, L3::Arp(a)),
        };
        let inner = EthernetFrame {
            dst: vx.inner_dst,
            src: vx.inner_src,
            vlan: None,
            ethertype,
            payload,
        };
        let learned = self.net.device(dev).and_then(|d| match &d.state {
            DeviceState::Switch(sw) => sw.mac_table.get(&(vlan, vx.inner_dst)).copied(),
            _ => None,
        });
        if let Some(p) = learned {
            if p != in_port {
                self.l2_egress(dev, p, vlan, inner);
            }
            return;
        }
        for p in self.logical_ports(dev) {
            if p != in_port && self.can_forward(dev, p, vlan) {
                self.l2_egress(dev, p, vlan, inner);
            }
        }
    }

    // ---------- EVPN 控制面（简化 Type-3：VTEP + VNI 路由） ----------

    pub fn evpn_config_changed(&mut self, dev: DevId) {
        let enabled = matches!(
            self.net.device(dev).map(|d| &d.state),
            Some(DeviceState::Switch(sw)) if sw.evpn.enabled
        );
        if !enabled {
            return;
        }
        self.evpn_schedule(dev);
        self.evpn_send(dev);
    }

    fn evpn_schedule(&mut self, dev: DevId) {
        let need = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return;
            };
            if !sw.evpn.enabled || sw.evpn.timer_pending {
                false
            } else {
                sw.evpn.timer_pending = true;
                true
            }
        };
        if need {
            self.push(self.now + EVPN_INTERVAL_NS, EKind::EvpnTimer { dev });
        }
    }

    fn on_evpn_timer(&mut self, dev: DevId) {
        if let Some(d) = self.net.device_mut(dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.evpn.timer_pending = false;
            }
        }
        self.evpn_send(dev);
        self.evpn_schedule(dev);
    }

    fn evpn_send(&mut self, dev: DevId) {
        let (local, peers, mappings) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            let Some(local) = sw.vxlan.local else {
                return;
            };
            (local, sw.evpn.peers.clone(), sw.vxlan.mappings.clone())
        };
        if peers.is_empty() || mappings.is_empty() {
            return;
        }
        for peer in peers {
            let Some((_, nh, out_vlan)) = self.net.lookup_route(dev, peer) else {
                continue;
            };
            // 经静态路由的 peer：按下一跳所在的直连网段解析通告 VLAN
            let out_vlan =
                out_vlan.or_else(|| self.net.lookup_route(dev, nh).and_then(|(_, _, v)| v));
            let src_mac = out_vlan.map(svi_mac).unwrap_or([0; 6]);
            for (vni, vlan) in &mappings {
                let pkt = EvpnPacket {
                    src: local,
                    dst: peer,
                    vtep_ip: local,
                    vni: *vni,
                    vlan: *vlan,
                };
                let frame = EthernetFrame {
                    dst: BROADCAST_MAC,
                    src: src_mac,
                    vlan: None,
                    ethertype: ETHERTYPE_IPV4,
                    payload: L3::Evpn(pkt),
                };
                if let Some(v) = out_vlan {
                    self.flood_in_vlan(dev, v, frame);
                }
            }
        }
    }

    fn evpn_on_packet(&mut self, dev: DevId, p: EvpnPacket) {
        let local = self.net.device(dev).and_then(|d| match &d.state {
            DeviceState::Switch(sw) => sw.vxlan.local,
            _ => None,
        });
        let Some(local) = local else {
            return;
        };
        if p.dst != local && p.dst != 0xffff_ffff {
            return;
        }
        if p.vtep_ip == local {
            return;
        }
        if let Some(d) = self.net.device_mut(dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                let route = EvpnRoute {
                    vtep: p.vtep_ip,
                    vni: p.vni,
                    vlan: p.vlan,
                };
                match sw
                    .evpn
                    .routes
                    .iter_mut()
                    .find(|r| r.vtep == route.vtep && r.vni == route.vni)
                {
                    Some(r) => *r = route,
                    None => sw.evpn.routes.push(route),
                }
                if !sw.vxlan.peers.contains(&p.vtep_ip) {
                    sw.vxlan.peers.push(p.vtep_ip);
                }
            }
        }
    }

    pub fn evpn_routes_of(&self, dev: DevId) -> Vec<EvpnRoute> {
        match self.net.device(dev).map(|d| &d.state) {
            Some(DeviceState::Switch(sw)) => sw.evpn.routes.clone(),
            _ => vec![],
        }
    }

    // ---------- F5 LTM（虚拟服务器 / 池 / TCP 负载均衡） ----------

    fn ltm_translate(&mut self, dev: DevId, tcp: &mut TcpPacket) -> Option<()> {
        let now = self.now;
        let Some(d) = self.net.device_mut(dev) else {
            return Some(());
        };
        if d.kind != DeviceKind::LoadBalancer {
            return Some(());
        }
        let virtual_cfg = d
            .ltm
            .virtuals
            .iter()
            .find(|v| v.vip == tcp.dst && v.port == tcp.dst_port)
            .cloned();
        if let Some(v) = virtual_cfg {
            let members = d
                .ltm
                .pools
                .iter()
                .find(|p| p.name == v.pool)
                .map(|p| p.members.clone())
                .unwrap_or_default();
            if let Some(s) = d.ltm.sessions.iter_mut().find(|s| {
                s.client_ip == tcp.src
                    && s.client_port == tcp.src_port
                    && s.vip == v.vip
                    && s.vip_port == v.port
            }) {
                tcp.dst = s.member_ip;
                tcp.dst_port = s.member_port;
                s.last_ns = now;
                return Some(());
            }
            let is_syn = tcp.flags & TCP_SYN != 0 && tcp.flags & TCP_ACK == 0;
            if !is_syn || members.is_empty() {
                return None;
            }
            let idx = d.ltm.rr % members.len();
            d.ltm.rr = d.ltm.rr.wrapping_add(1);
            let m = members[idx];
            d.ltm.sessions.push(LtmSession {
                client_ip: tcp.src,
                client_port: tcp.src_port,
                vip: v.vip,
                vip_port: v.port,
                member_ip: m.ip,
                member_port: m.port,
                last_ns: now,
            });
            tcp.dst = m.ip;
            tcp.dst_port = m.port;
            return Some(());
        }
        let reverse = d.ltm.sessions.iter().find(|s| {
            s.member_ip == tcp.src
                && s.member_port == tcp.src_port
                && s.client_ip == tcp.dst
                && s.client_port == tcp.dst_port
        });
        if let Some(s) = reverse {
            tcp.src = s.vip;
            tcp.src_port = s.vip_port;
        }
        Some(())
    }

    fn ltm_owns_vip(&self, dev: DevId, ip: Ipv4Addr) -> bool {
        self.net.device(dev).is_some_and(|d| {
            d.kind == DeviceKind::LoadBalancer && d.ltm.virtuals.iter().any(|v| v.vip == ip)
        })
    }

    pub fn ltm_sessions_of(&self, dev: DevId) -> Vec<LtmSession> {
        self.net
            .device(dev)
            .map(|d| d.ltm.sessions.clone())
            .unwrap_or_default()
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
            hs.next_ping_id.wrapping_mul(2654435761) | 1
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

    // ---------- 端口安全 / DHCP Snooping / QoS ----------

    fn dhcp_snooping_ingress(
        &mut self,
        dev: DevId,
        port: PortId,
        vlan: VlanId,
        dhcp: &DhcpPacket,
    ) -> bool {
        let (enabled, trusted) = {
            let Some(d) = self.net.device(dev) else {
                return false;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return false;
            };
            (
                sw.dhcp_snooping,
                d.port(port).map(|p| p.dhcp_trust).unwrap_or(false),
            )
        };
        if !enabled {
            return false;
        }
        if matches!(dhcp.op, DhcpOp::Offer | DhcpOp::Ack) && !trusted {
            return true; // 非信任端口来的服务器报文丢弃
        }
        let ip = match dhcp.op {
            DhcpOp::Request => dhcp.requested_ip,
            DhcpOp::Ack => dhcp.offered_ip,
            _ => 0,
        };
        if ip != 0 {
            let Some(d) = self.net.device_mut(dev) else {
                return false;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return false;
            };
            let b = DhcpSnoopingBinding {
                mac: dhcp.client_mac,
                ip,
                vlan,
                port,
            };
            if let Some(x) = sw.dhcp_bindings.iter_mut().find(|x| x.mac == b.mac) {
                *x = b;
            } else {
                sw.dhcp_bindings.push(b);
            }
        }
        false
    }

    fn port_security_allow(&mut self, dev: DevId, port: PortId, mac: Mac) -> bool {
        let shutdown = {
            let Some(d) = self.net.device_mut(dev) else {
                return true;
            };
            let Some(cfg) = d.port(port).and_then(|p| p.port_security) else {
                return true;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return true;
            };
            let list = sw.secure_macs.entry(port).or_default();
            if list.contains(&mac) {
                return true;
            }
            if (list.len() as u16) < cfg.max {
                list.push(mac);
                return true;
            }
            *sw.port_sec_violations.entry(port).or_insert(0) += 1;
            cfg.violation == PortSecViolation::Shutdown
        };
        if shutdown {
            self.net.set_port_admin(dev, port, false);
            self.stp_topology_changed();
        }
        false
    }

    fn qos_ingress_drop(&mut self, dev: DevId, port: PortId, frame: &EthernetFrame) -> bool {
        let (limit, rate) = {
            let Some(d) = self.net.device(dev) else {
                return false;
            };
            let Some(p) = d.port(port) else {
                return false;
            };
            (p.broadcast_limit_pps, p.rate_limit_kbps)
        };
        if limit.is_none() && rate.is_none() {
            return false;
        }
        let now = self.now;
        let size = frame.wire_size() as f64;
        let st = self.qos.entry((dev, port)).or_insert_with(|| QosState {
            last_ns: now,
            ..Default::default()
        });
        let mut drop = false;
        if let Some(rate) = rate {
            let bps = rate as f64 * 1000.0 / 8.0;
            let elapsed = now.saturating_sub(st.last_ns) as f64 / 1e9;
            st.last_ns = now;
            let burst = bps.max(size);
            if st.tokens.is_infinite() {
                st.tokens = burst;
            }
            st.tokens = (st.tokens + elapsed * bps).min(burst);
            if st.tokens < size {
                st.drops += 1;
                drop = true;
            } else {
                st.tokens -= size;
            }
        }
        if let Some(lim) = limit {
            if frame.dst == BROADCAST_MAC {
                if now.saturating_sub(st.window_ns) >= 1_000_000_000 {
                    st.window_ns = now;
                    st.bcast = 0;
                }
                st.bcast += 1;
                if st.bcast > lim {
                    st.bcast_drops += 1;
                    drop = true;
                }
            }
        }
        drop
    }

    /// 供 show 使用：(端口, 广播上限, 限速 kbps, 限速丢弃, 风暴抑制丢弃)。
    pub fn qos_stats_of(&self, dev: DevId) -> Vec<QosStat> {
        let Some(d) = self.net.device(dev) else {
            return vec![];
        };
        d.ports
            .iter()
            .filter(|p| p.broadcast_limit_pps.is_some() || p.rate_limit_kbps.is_some())
            .map(|p| {
                let st = self.qos.get(&(dev, p.idx)).copied().unwrap_or_default();
                (
                    p.idx,
                    p.broadcast_limit_pps,
                    p.rate_limit_kbps,
                    st.drops,
                    st.bcast_drops,
                )
            })
            .collect()
    }

    pub fn dhcp_bindings_of(&self, dev: DevId) -> Vec<DhcpSnoopingBinding> {
        match self.net.device(dev).map(|d| &d.state) {
            Some(DeviceState::Switch(sw)) if sw.dhcp_snooping => sw.dhcp_bindings.clone(),
            _ => vec![],
        }
    }

    pub fn dhcp_snooping_on(&self, dev: DevId) -> bool {
        matches!(self.net.device(dev).map(|d| &d.state), Some(DeviceState::Switch(sw)) if sw.dhcp_snooping)
    }

    pub fn port_security_of(&self, dev: DevId, port: PortId) -> Option<(PortSecurity, usize, u32)> {
        let d = self.net.device(dev)?;
        let DeviceState::Switch(sw) = &d.state else {
            return None;
        };
        let cfg = d.port(port)?.port_security?;
        let current = sw.secure_macs.get(&port).map(|v| v.len()).unwrap_or(0);
        let viol = sw.port_sec_violations.get(&port).copied().unwrap_or(0);
        Some((cfg, current, viol))
    }

    // ---------- BGP（eBGP 简化：广播报文 + 周期 Keepalive） ----------

    pub fn bgp_config_changed(&mut self, dev: DevId) {
        let Some(cfg) = self.bgp_config(dev) else {
            return;
        };
        {
            let rt = self.bgp.entry(dev).or_default();
            for (ip, ras) in &cfg.neighbors {
                let s = rt.sessions.entry(*ip).or_insert(BgpSession {
                    remote_as: *ras,
                    state: BgpState::Idle,
                    last_rx_ns: 0,
                });
                if s.remote_as != *ras {
                    s.remote_as = *ras;
                    s.state = BgpState::Idle;
                }
            }
            let valid: std::collections::HashSet<Ipv4Addr> =
                cfg.neighbors.iter().map(|(i, _)| *i).collect();
            rt.sessions.retain(|ip, _| valid.contains(ip));
            rt.learned.retain(|_, l| valid.contains(&l.from));
        }
        self.bgp_recompute_routes(dev);
        self.bgp_schedule(dev);
        self.bgp_send_all(dev);
    }

    fn bgp_config(&self, dev: DevId) -> Option<BgpConfig> {
        let d = self.net.device(dev)?;
        let DeviceState::Router(rs) = &d.state else {
            return None;
        };
        rs.bgp.clone()
    }

    /// 路由前缀是否命中基础 ACL（ACL permit = 命中）。
    fn route_acl_permit(&self, dev: DevId, acl_no: u32, net: Ipv4Addr) -> bool {
        let Some(d) = self.net.device(dev) else {
            return false;
        };
        let DeviceState::Router(rs) = &d.state else {
            return false;
        };
        let Some(acl) = rs.acls.iter().find(|a| a.number == acl_no) else {
            return false;
        };
        for r in &acl.rules {
            if r.matches_route(net) {
                return r.permit;
            }
        }
        false
    }

    /// 执行路由策略：按 seq 升序首个命中节点决定动作；无节点命中 = 隐式拒绝。
    fn route_policy_decide(
        &self,
        dev: DevId,
        name: &str,
        net: Ipv4Addr,
    ) -> Option<(bool, Option<u32>)> {
        let d = self.net.device(dev)?;
        let DeviceState::Router(rs) = &d.state else {
            return None;
        };
        let rp = rs.route_policies.iter().find(|p| p.name == name)?;
        let mut nodes: Vec<&RoutePolicyNode> = rp.nodes.iter().collect();
        nodes.sort_by_key(|n| n.seq);
        for node in nodes {
            let matched = node
                .match_acl
                .is_none_or(|acl| self.route_acl_permit(dev, acl, net));
            if matched {
                return Some((node.permit, node.set_local_pref));
            }
        }
        Some((false, None))
    }

    /// 邻居入向策略：未配置策略放行；配置了但策略不存在 = 拒绝。
    fn bgp_import_decision(
        &self,
        dev: DevId,
        neighbor: Ipv4Addr,
        net: Ipv4Addr,
    ) -> (bool, Option<u32>) {
        let Some(cfg) = self.bgp_config(dev) else {
            return (true, None);
        };
        let Some(pp) = cfg
            .peer_policies
            .iter()
            .find(|p| p.import && p.neighbor == neighbor)
        else {
            return (true, None);
        };
        self.route_policy_decide(dev, &pp.name, net)
            .unwrap_or((false, None))
    }

    /// 邻居出向策略：默认放行。
    fn bgp_export_allowed(&self, dev: DevId, neighbor: Ipv4Addr, net: Ipv4Addr) -> bool {
        let Some(cfg) = self.bgp_config(dev) else {
            return true;
        };
        let Some(pp) = cfg
            .peer_policies
            .iter()
            .find(|p| !p.import && p.neighbor == neighbor)
        else {
            return true;
        };
        self.route_policy_decide(dev, &pp.name, net)
            .map(|(permit, _)| permit)
            .unwrap_or(false)
    }

    /// 策略变更后清空已学习路由并重算，邻居下一轮 Update 会按新策略重新学习。
    pub fn bgp_policy_changed(&mut self, dev: DevId) {
        if let Some(rt) = self.bgp.get_mut(&dev) {
            rt.learned.clear();
        }
        self.bgp_recompute_routes(dev);
        self.bgp_send_all(dev);
    }

    fn bgp_schedule(&mut self, dev: DevId) {
        let rt = self.bgp.entry(dev).or_default();
        if !rt.timer_pending {
            rt.timer_pending = true;
            self.push(self.now + BGP_KEEPALIVE_NS, EKind::BgpTimer { dev });
        }
    }

    fn bgp_entries_for(&self, dev: DevId, neighbor: Ipv4Addr) -> ([Option<BgpEntry>; 8], u8) {
        let mut out = [None; 8];
        let mut n = 0usize;
        let Some(cfg) = self.bgp_config(dev) else {
            return (out, 0);
        };
        let mut push = |net: Ipv4Addr, len: u8, path: Vec<u32>| {
            if n >= 8 {
                return;
            }
            let mut p = [0u32; 4];
            let take = path.len().min(4);
            p[..take].copy_from_slice(&path[..take]);
            out[n] = Some(BgpEntry {
                net,
                len,
                path: p,
                path_len: take as u8,
            });
            n += 1;
        };
        for (net, len) in &cfg.networks {
            if !self.bgp_export_allowed(dev, neighbor, *net) {
                continue;
            }
            push(*net, *len, vec![cfg.asn]);
        }
        if let Some(rt) = self.bgp.get(&dev) {
            for ((net, len), l) in &rt.learned {
                if l.from == neighbor {
                    continue;
                }
                if !self.bgp_export_allowed(dev, neighbor, *net) {
                    continue;
                }
                let mut path = vec![cfg.asn];
                path.extend(l.path.iter().copied());
                push(*net, *len, path);
            }
        }
        (out, n as u8)
    }

    fn bgp_send(
        &mut self,
        dev: DevId,
        neighbor: Ipv4Addr,
        kind: BgpKind,
        entries: [Option<BgpEntry>; 8],
        count: u8,
    ) {
        let Some(cfg) = self.bgp_config(dev) else {
            return;
        };
        let Some((port, _nh, _vlan)) = self.net.lookup_route(dev, neighbor) else {
            return;
        };
        let Some(d) = self.net.device(dev) else {
            return;
        };
        let Some(p) = d.port(port) else {
            return;
        };
        let Some(src_ip) = p.ip else {
            return;
        };
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: p.mac,
            vlan: None,
            ethertype: ETHERTYPE_BGP,
            payload: L3::Bgp(BgpPacket {
                kind,
                asn: cfg.asn,
                src_ip,
                entries,
                count,
            }),
        };
        self.transmit_frame(dev, port, frame);
    }

    fn bgp_send_all(&mut self, dev: DevId) {
        let neighbors: Vec<(Ipv4Addr, BgpState)> = self
            .bgp
            .get(&dev)
            .map(|rt| rt.sessions.iter().map(|(ip, s)| (*ip, s.state)).collect())
            .unwrap_or_default();
        for (ip, state) in neighbors {
            let (entries, count) = self.bgp_entries_for(dev, ip);
            self.bgp_send(dev, ip, BgpKind::Open, [None; 8], 0);
            if state == BgpState::Established {
                self.bgp_send(dev, ip, BgpKind::Keepalive, [None; 8], 0);
                self.bgp_send(dev, ip, BgpKind::Update, entries, count);
            }
        }
    }

    fn on_bgp_timer(&mut self, dev: DevId) {
        if let Some(rt) = self.bgp.get_mut(&dev) {
            rt.timer_pending = false;
        }
        let Some(cfg) = self.bgp_config(dev) else {
            return;
        };
        let now = self.now;
        // 会话老化
        let mut lost: Vec<Ipv4Addr> = vec![];
        {
            let rt = self.bgp.entry(dev).or_default();
            for (ip, s) in rt.sessions.iter_mut() {
                if s.state == BgpState::Established
                    && now.saturating_sub(s.last_rx_ns) > BGP_HOLD_NS
                {
                    s.state = BgpState::Idle;
                    lost.push(*ip);
                }
            }
            for ip in &lost {
                rt.learned.retain(|_, l| l.from != *ip);
            }
        }
        if !lost.is_empty() {
            self.bgp_recompute_routes(dev);
        }
        // 周期发送：Open / Keepalive + Update
        let neighbors: Vec<(Ipv4Addr, BgpState)> = self
            .bgp
            .get(&dev)
            .map(|rt| rt.sessions.iter().map(|(ip, s)| (*ip, s.state)).collect())
            .unwrap_or_default();
        for (ip, state) in neighbors {
            if state == BgpState::Idle {
                self.bgp_send(dev, ip, BgpKind::Open, [None; 8], 0);
            } else {
                self.bgp_send(dev, ip, BgpKind::Keepalive, [None; 8], 0);
                let (entries, count) = self.bgp_entries_for(dev, ip);
                self.bgp_send(dev, ip, BgpKind::Update, entries, count);
            }
        }
        let _ = cfg;
        self.bgp_schedule(dev);
    }

    fn on_bgp_packet(&mut self, dev: DevId, _port: PortId, pkt: BgpPacket) {
        let Some(cfg) = self.bgp_config(dev) else {
            return;
        };
        let Some(&(_, remote_as)) = cfg.neighbors.iter().find(|(ip, _)| *ip == pkt.src_ip) else {
            return;
        };
        if pkt.asn != remote_as {
            return;
        }
        {
            let rt = self.bgp.entry(dev).or_default();
            let Some(s) = rt.sessions.get_mut(&pkt.src_ip) else {
                return;
            };
            s.last_rx_ns = self.now;
            if pkt.kind == BgpKind::Open {
                s.state = BgpState::Established;
            }
            if s.state != BgpState::Established && pkt.kind != BgpKind::Open {
                return;
            }
        }
        if pkt.kind == BgpKind::Open {
            let (entries, count) = self.bgp_entries_for(dev, pkt.src_ip);
            self.bgp_send(dev, pkt.src_ip, BgpKind::Keepalive, [None; 8], 0);
            self.bgp_send(dev, pkt.src_ip, BgpKind::Update, entries, count);
            return;
        }
        if pkt.kind != BgpKind::Update {
            return;
        }
        let mut changed = false;
        let mut incoming: Vec<((Ipv4Addr, u8), Vec<u32>, u32)> = vec![];
        for e in pkt.entries.iter().flatten() {
            let path: Vec<u32> = e.path[..e.path_len as usize].to_vec();
            if path.contains(&cfg.asn) {
                continue; // AS_PATH 环路
            }
            let (permit, local_pref) = self.bgp_import_decision(dev, pkt.src_ip, e.net);
            if !permit {
                continue;
            }
            incoming.push(((e.net, e.len), path, local_pref.unwrap_or(100)));
        }
        {
            let Some(rt) = self.bgp.get_mut(&dev) else {
                return;
            };
            for (key, path, local_pref) in incoming {
                let better = match rt.learned.get(&key) {
                    None => true,
                    Some(old) => {
                        local_pref > old.local_pref
                            || (local_pref == old.local_pref && path.len() < old.path.len())
                    }
                };
                if better {
                    rt.learned.insert(
                        key,
                        BgpLearned {
                            nexthop: pkt.src_ip,
                            path,
                            from: pkt.src_ip,
                            local_pref,
                        },
                    );
                    changed = true;
                }
            }
        }
        if changed {
            self.bgp_recompute_routes(dev);
            // 向其它已建立邻居重发更新
            let others: Vec<Ipv4Addr> = self
                .bgp
                .get(&dev)
                .map(|rt| {
                    rt.sessions
                        .iter()
                        .filter(|(ip, s)| **ip != pkt.src_ip && s.state == BgpState::Established)
                        .map(|(ip, _)| *ip)
                        .collect()
                })
                .unwrap_or_default();
            for ip in others {
                let (entries, count) = self.bgp_entries_for(dev, ip);
                self.bgp_send(dev, ip, BgpKind::Update, entries, count);
            }
        }
    }

    fn bgp_recompute_routes(&mut self, dev: DevId) {
        let routes: Vec<Route> = self
            .bgp
            .get(&dev)
            .map(|rt| {
                rt.learned
                    .iter()
                    .map(|((net, len), l)| Route {
                        dst: *net,
                        len: *len,
                        nexthop: l.nexthop,
                        vlan: None,
                        port: 0,
                        src: RouteSrc::Bgp,
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Some(d) = self.net.device_mut(dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.bgp_routes = routes;
            }
        }
    }

    /// 供 show 使用：(邻居, 对端 AS, 是否 Established)。
    pub fn bgp_sessions_of(&self, dev: DevId) -> Vec<(Ipv4Addr, u32, bool)> {
        let mut v: Vec<(Ipv4Addr, u32, bool)> = self
            .bgp
            .get(&dev)
            .map(|rt| {
                rt.sessions
                    .iter()
                    .map(|(ip, s)| (*ip, s.remote_as, s.state == BgpState::Established))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|x| x.0);
        v
    }

    /// 供 show 使用：(网段, 前缀, AS_PATH, 下一跳, Local Preference)。
    pub fn bgp_routes_of(&self, dev: DevId) -> Vec<(Ipv4Addr, u8, Vec<u32>, Ipv4Addr, u32)> {
        let mut v: Vec<(Ipv4Addr, u8, Vec<u32>, Ipv4Addr, u32)> = self
            .bgp
            .get(&dev)
            .map(|rt| {
                rt.learned
                    .iter()
                    .map(|((net, len), l)| (*net, *len, l.path.clone(), l.nexthop, l.local_pref))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|x| (x.0, x.1));
        v
    }

    // ---------- IS-IS（单区域 Level-1 简化） ----------

    pub fn isis_config_changed(&mut self, dev: DevId) {
        if !self.isis_on(dev) {
            return;
        }
        self.isis.entry(dev).or_default();
        let lsa = self.isis_build_lsa(dev);
        self.isis
            .entry(dev)
            .or_default()
            .lsdb
            .insert(lsa.system_id, lsa);
        self.isis_recompute_all();
        self.isis_flood(dev, lsa, None);
        self.isis_send_hello(dev);
        self.isis_schedule(dev);
    }

    fn isis_on(&self, dev: DevId) -> bool {
        self.net
            .device(dev)
            .map(|d| matches!(&d.state, DeviceState::Router(rs) if rs.isis.is_some()))
            .unwrap_or(false)
    }

    fn isis_ports(&self, dev: DevId) -> Vec<(PortId, Ipv4Addr, u8)> {
        let Some(d) = self.net.device(dev) else {
            return vec![];
        };
        d.ports
            .iter()
            .filter(|p| p.isis_enable && p.admin_up && p.link.is_some() && p.vrf.is_none())
            .filter_map(|p| p.ip.map(|ip| (p.idx, ip, p.prefix)))
            .collect()
    }

    fn isis_system_id(&self, dev: DevId) -> Option<[u8; 6]> {
        let d = self.net.device(dev)?;
        let DeviceState::Router(rs) = &d.state else {
            return None;
        };
        rs.isis.as_ref().map(|c| c.system_id)
    }

    fn isis_build_lsa(&self, dev: DevId) -> IsisLsa {
        let mut lsa = IsisLsa {
            system_id: self.isis_system_id(dev).unwrap_or([0; 6]),
            ..Default::default()
        };
        for (_, ip, prefix) in self.isis_ports(dev) {
            if lsa.link_count as usize >= lsa.links.len() {
                break;
            }
            let i = lsa.link_count as usize;
            lsa.links[i] = Some(OspfLink {
                net: ip & mask_from_prefix(prefix),
                len: prefix,
            });
            lsa.link_count += 1;
        }
        if let Some(rt) = self.isis.get(&dev) {
            let now = self.now;
            for (sid, nb) in &rt.neighbors {
                if now.saturating_sub(nb.last_ns) <= ISIS_DEAD_NS
                    && (lsa.neighbor_count as usize) < lsa.neighbors.len()
                {
                    lsa.neighbors[lsa.neighbor_count as usize] = *sid;
                    lsa.neighbor_count += 1;
                }
            }
        }
        lsa
    }

    fn isis_schedule(&mut self, dev: DevId) {
        let rt = self.isis.entry(dev).or_default();
        if !rt.timer_pending {
            rt.timer_pending = true;
            self.push(self.now + ISIS_HELLO_NS, EKind::IsisTimer { dev });
        }
    }

    fn isis_send_hello(&mut self, dev: DevId) {
        let lsa = self.isis_build_lsa(dev);
        for (port, ip, _) in self.isis_ports(dev) {
            let Some(mac) = self
                .net
                .device(dev)
                .and_then(|d| d.port(port))
                .map(|p| p.mac)
            else {
                continue;
            };
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_ISIS,
                payload: L3::Isis(IsisPacket {
                    kind: IsisKind::Hello,
                    system_id: lsa.system_id,
                    src_ip: ip,
                    lsa,
                }),
            };
            self.transmit_frame(dev, port, frame);
        }
    }

    fn isis_flood(&mut self, dev: DevId, lsa: IsisLsa, skip: Option<PortId>) {
        for (port, ip, _) in self.isis_ports(dev) {
            if skip == Some(port) {
                continue;
            }
            let Some(mac) = self
                .net
                .device(dev)
                .and_then(|d| d.port(port))
                .map(|p| p.mac)
            else {
                continue;
            };
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_ISIS,
                payload: L3::Isis(IsisPacket {
                    kind: IsisKind::Lsp,
                    system_id: lsa.system_id,
                    src_ip: ip,
                    lsa,
                }),
            };
            self.transmit_frame(dev, port, frame);
        }
    }

    fn on_isis_timer(&mut self, dev: DevId) {
        if !self.isis_on(dev) {
            return;
        }
        if let Some(rt) = self.isis.get_mut(&dev) {
            rt.timer_pending = false;
        }
        let now = self.now;
        let expired: Vec<[u8; 6]> = self
            .isis
            .get(&dev)
            .map(|rt| {
                rt.neighbors
                    .iter()
                    .filter(|(_, nb)| now.saturating_sub(nb.last_ns) > ISIS_DEAD_NS)
                    .map(|(sid, _)| *sid)
                    .collect()
            })
            .unwrap_or_default();
        if !expired.is_empty() {
            if let Some(rt) = self.isis.get_mut(&dev) {
                for sid in expired {
                    rt.neighbors.remove(&sid);
                }
            }
            let lsa = self.isis_build_lsa(dev);
            if let Some(rt) = self.isis.get_mut(&dev) {
                rt.lsdb.insert(lsa.system_id, lsa);
            }
            self.isis_flood(dev, lsa, None);
            self.isis_recompute_all();
        }
        self.isis_send_hello(dev);
        self.isis_schedule(dev);
    }

    fn on_isis_packet(&mut self, dev: DevId, port: PortId, pkt: IsisPacket) {
        if !self.isis_on(dev) {
            return;
        }
        if !self.isis_ports(dev).iter().any(|(p, _, _)| *p == port) {
            return;
        }
        if pkt.system_id == [0; 6] || Some(pkt.system_id) == self.isis_system_id(dev) {
            return;
        }
        let now = self.now;
        let mut changed = false;
        {
            let rt = self.isis.entry(dev).or_default();
            match rt.neighbors.get(&pkt.system_id) {
                Some(nb) if now.saturating_sub(nb.last_ns) <= ISIS_DEAD_NS => {}
                _ => changed = true,
            }
            rt.neighbors.insert(
                pkt.system_id,
                IsisNb {
                    ip: pkt.src_ip,
                    port,
                    last_ns: now,
                },
            );
            if pkt.lsa.system_id != [0; 6] && rt.lsdb.get(&pkt.lsa.system_id) != Some(&pkt.lsa) {
                rt.lsdb.insert(pkt.lsa.system_id, pkt.lsa);
                changed = true;
            }
        }
        if changed {
            let own = self.isis_build_lsa(dev);
            if let Some(rt) = self.isis.get_mut(&dev) {
                rt.lsdb.insert(own.system_id, own);
            }
            self.isis_flood(dev, pkt.lsa, Some(port));
            self.isis_recompute_all();
        }
    }

    fn isis_recompute_all(&mut self) {
        let devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Router(rs) if rs.isis.is_some()))
            .map(|d| d.id)
            .collect();
        for dev in &devs {
            let lsa = self.isis_build_lsa(*dev);
            self.isis
                .entry(*dev)
                .or_default()
                .lsdb
                .insert(lsa.system_id, lsa);
        }
        for dev in devs {
            let routes = self.isis_spf(dev);
            if let Some(d) = self.net.device_mut(dev) {
                if let DeviceState::Router(rs) = &mut d.state {
                    rs.isis_routes = routes;
                }
            }
        }
    }

    fn isis_spf(&self, dev: DevId) -> Vec<Route> {
        let Some(rt) = self.isis.get(&dev) else {
            return vec![];
        };
        let Some(self_id) = self.isis_system_id(dev) else {
            return vec![];
        };
        let mut dist: std::collections::HashMap<[u8; 6], u32> = Default::default();
        let mut first: std::collections::HashMap<[u8; 6], [u8; 6]> = Default::default();
        let mut done: std::collections::HashSet<[u8; 6]> = Default::default();
        dist.insert(self_id, 0);
        loop {
            let next = dist
                .iter()
                .filter(|(sid, _)| !done.contains(*sid))
                .min_by_key(|(_, d)| **d)
                .map(|(sid, d)| (*sid, *d));
            let Some((u, du)) = next else { break };
            done.insert(u);
            let Some(lsa) = rt.lsdb.get(&u) else { continue };
            for i in 0..lsa.neighbor_count as usize {
                let v = lsa.neighbors[i];
                if v == [0; 6] || !rt.lsdb.contains_key(&v) {
                    continue;
                }
                let nd = du + 1;
                if nd < *dist.get(&v).unwrap_or(&u32::MAX) {
                    dist.insert(v, nd);
                    let hop = if u == self_id {
                        v
                    } else {
                        *first.get(&u).unwrap_or(&v)
                    };
                    first.insert(v, hop);
                }
            }
        }
        let connected: std::collections::HashSet<(Ipv4Addr, u8)> = self
            .net
            .device(dev)
            .map(|d| {
                d.connected_routes()
                    .iter()
                    .map(|r| (r.dst, r.len))
                    .collect()
            })
            .unwrap_or_default();
        let mut routes = vec![];
        let mut seen: std::collections::HashSet<(Ipv4Addr, u8)> = Default::default();
        for sid in dist.keys() {
            if *sid == self_id {
                continue;
            }
            let Some(hop) = first.get(sid) else { continue };
            let Some(nb) = rt.neighbors.get(hop) else {
                continue;
            };
            let Some(lsa) = rt.lsdb.get(sid) else {
                continue;
            };
            for i in 0..lsa.link_count as usize {
                let Some(link) = lsa.links[i] else { continue };
                if connected.contains(&(link.net, link.len)) || !seen.insert((link.net, link.len)) {
                    continue;
                }
                routes.push(Route {
                    dst: link.net,
                    len: link.len,
                    nexthop: nb.ip,
                    vlan: None,
                    port: 0,
                    src: RouteSrc::Isis,
                });
            }
        }
        routes
    }

    pub fn isis_neighbors_of(&self, dev: DevId) -> Vec<([u8; 6], Ipv4Addr, PortId)> {
        let now = self.now;
        let mut v: Vec<([u8; 6], Ipv4Addr, PortId)> = self
            .isis
            .get(&dev)
            .map(|rt| {
                rt.neighbors
                    .iter()
                    .filter(|(_, nb)| now.saturating_sub(nb.last_ns) <= ISIS_DEAD_NS)
                    .map(|(sid, nb)| (*sid, nb.ip, nb.port))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|x| x.0);
        v
    }

    pub fn isis_lsdb_of(&self, dev: DevId) -> Vec<([u8; 6], u8, u8)> {
        let mut v: Vec<([u8; 6], u8, u8)> = self
            .isis
            .get(&dev)
            .map(|rt| {
                rt.lsdb
                    .values()
                    .map(|l| (l.system_id, l.link_count, l.neighbor_count))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|x| x.0);
        v
    }

    pub fn isis_system_of(&self, dev: DevId) -> Option<[u8; 6]> {
        self.isis_system_id(dev)
    }

    // ---------- OSPF（单区域简化：Hello + Router-LSA 泛洪 + SPF） ----------

    /// 配置变化后启动/刷新 OSPF 进程。
    pub fn ospf_config_changed(&mut self, dev: DevId) {
        if !self.ospf_enabled(dev) {
            return;
        }
        self.ospf.entry(dev).or_default();
        let lsa = self.ospf_build_lsa(dev);
        self.ospf
            .entry(dev)
            .or_default()
            .lsdb
            .insert(lsa.router_id, lsa);
        self.ospf_recompute_all();
        self.ospf_flood(dev, lsa, None);
        self.ospf_send_hello(dev);
        self.ospf_schedule(dev);
    }

    /// 供 show 使用：本机 OSPF Router ID。
    pub fn ospf_router_id_of(&self, dev: DevId) -> Ipv4Addr {
        self.ospf_router_id(dev)
    }

    /// 供 show 使用：存活邻居 (router id, 邻居接口 IP, 本地接口)。
    pub fn ospf_neighbors_of(&self, dev: DevId) -> Vec<(Ipv4Addr, Ipv4Addr, PortId)> {
        let now = self.now;
        let mut v: Vec<(Ipv4Addr, Ipv4Addr, PortId)> = self
            .ospf
            .get(&dev)
            .map(|rt| {
                rt.neighbors
                    .iter()
                    .filter(|(_, nb)| now.saturating_sub(nb.last_ns) <= OSPF_DEAD_NS)
                    .map(|(rid, nb)| (*rid, nb.ip, nb.port))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_unstable();
        v
    }

    /// 供 show 使用：LSDB 摘要 (router id, 网段数, 邻居数)。
    pub fn ospf_lsdb_of(&self, dev: DevId) -> Vec<(Ipv4Addr, u8, u8)> {
        let mut v: Vec<(Ipv4Addr, u8, u8)> = self
            .ospf
            .get(&dev)
            .map(|rt| {
                rt.lsdb
                    .values()
                    .map(|l| (l.router_id, l.link_count, l.neighbor_count))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_unstable();
        v
    }

    fn ospf_enabled(&self, dev: DevId) -> bool {
        self.net
            .device(dev)
            .map(|d| matches!(&d.state, DeviceState::Router(rs) if !rs.ospf_networks.is_empty()))
            .unwrap_or(false)
    }

    fn ospf_port_enabled(&self, dev: DevId, port: PortId) -> bool {
        let Some(d) = self.net.device(dev) else {
            return false;
        };
        let Some(ip) = d.port(port).and_then(|p| p.ip) else {
            return false;
        };
        let DeviceState::Router(rs) = &d.state else {
            return false;
        };
        rs.ospf_networks
            .iter()
            .any(|(net, wc)| (ip & !wc) == (*net & !wc))
    }

    fn ospf_enabled_ports(&self, dev: DevId) -> Vec<(PortId, Ipv4Addr, u8)> {
        let Some(d) = self.net.device(dev) else {
            return vec![];
        };
        d.ports
            .iter()
            .filter(|p| p.admin_up && p.link.is_some() && p.ip.is_some() && p.vrf.is_none())
            .filter(|p| self.ospf_port_enabled(dev, p.idx))
            .map(|p| (p.idx, p.ip.unwrap(), p.prefix))
            .collect()
    }

    fn ospf_router_id(&self, dev: DevId) -> Ipv4Addr {
        let Some(d) = self.net.device(dev) else {
            return 1;
        };
        let mut best = 0u32;
        for p in &d.ports {
            if let Some(ip) = p.ip {
                best = best.max(ip);
            }
            if let DeviceState::Switch(sw) = &d.state {
                for s in sw.svi.values() {
                    if s.admin_up && s.ip != 0 {
                        best = best.max(s.ip);
                    }
                }
            }
        }
        if best == 0 {
            1
        } else {
            best
        }
    }

    fn ospf_build_lsa(&self, dev: DevId) -> OspfLsa {
        let mut lsa = OspfLsa {
            router_id: self.ospf_router_id(dev),
            ..Default::default()
        };
        for (_, ip, prefix) in self.ospf_enabled_ports(dev) {
            if lsa.link_count as usize >= lsa.links.len() {
                break;
            }
            let i = lsa.link_count as usize;
            lsa.links[i] = Some(OspfLink {
                net: ip & mask_from_prefix(prefix),
                len: prefix,
            });
            lsa.link_count += 1;
        }
        if let Some(rt) = self.ospf.get(&dev) {
            let now = self.now;
            for (rid, nb) in &rt.neighbors {
                if now.saturating_sub(nb.last_ns) <= OSPF_DEAD_NS
                    && (lsa.neighbor_count as usize) < lsa.neighbors.len()
                {
                    lsa.neighbors[lsa.neighbor_count as usize] = *rid;
                    lsa.neighbor_count += 1;
                }
            }
        }
        lsa
    }

    fn ospf_schedule(&mut self, dev: DevId) {
        let need = {
            let rt = self.ospf.entry(dev).or_default();
            if rt.timer_pending {
                false
            } else {
                rt.timer_pending = true;
                true
            }
        };
        if need {
            self.push(self.now + OSPF_HELLO_NS, EKind::OspfTimer { dev });
        }
    }

    fn ospf_send_hello(&mut self, dev: DevId) {
        let lsa = self.ospf_build_lsa(dev);
        for (port, ip, _) in self.ospf_enabled_ports(dev) {
            let Some(mac) = self
                .net
                .device(dev)
                .and_then(|d| d.port(port))
                .map(|p| p.mac)
            else {
                continue;
            };
            let frame = EthernetFrame {
                dst: OSPF_MULTICAST_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_OSPF,
                payload: L3::Ospf(OspfPacket {
                    kind: OspfKind::Hello,
                    area: 0,
                    router_id: lsa.router_id,
                    src_ip: ip,
                    lsa,
                }),
            };
            self.transmit_frame(dev, port, frame);
        }
    }

    fn ospf_flood(&mut self, dev: DevId, lsa: OspfLsa, skip: Option<PortId>) {
        for (port, ip, _) in self.ospf_enabled_ports(dev) {
            if skip == Some(port) {
                continue;
            }
            let Some(mac) = self
                .net
                .device(dev)
                .and_then(|d| d.port(port))
                .map(|p| p.mac)
            else {
                continue;
            };
            let frame = EthernetFrame {
                dst: OSPF_MULTICAST_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_OSPF,
                payload: L3::Ospf(OspfPacket {
                    kind: OspfKind::Lsu,
                    area: 0,
                    router_id: lsa.router_id,
                    src_ip: ip,
                    lsa,
                }),
            };
            self.transmit_frame(dev, port, frame);
        }
    }

    fn ospf_on_packet(&mut self, dev: DevId, port: PortId, pkt: OspfPacket) {
        if !self.ospf_enabled(dev) || !self.ospf_port_enabled(dev, port) || pkt.area != 0 {
            return;
        }
        let now = self.now;
        let mut changed = false;
        {
            let rt = self.ospf.entry(dev).or_default();
            match rt.neighbors.get(&pkt.router_id) {
                Some(nb) if now.saturating_sub(nb.last_ns) <= OSPF_DEAD_NS => {}
                _ => changed = true,
            }
            rt.neighbors.insert(
                pkt.router_id,
                OspfNeighbor {
                    ip: pkt.src_ip,
                    port,
                    last_ns: now,
                },
            );
            if pkt.lsa.router_id != 0 && rt.lsdb.get(&pkt.lsa.router_id) != Some(&pkt.lsa) {
                rt.lsdb.insert(pkt.lsa.router_id, pkt.lsa);
                changed = true;
            }
        }
        if changed {
            let own = self.ospf_build_lsa(dev);
            if let Some(rt) = self.ospf.get_mut(&dev) {
                rt.lsdb.insert(own.router_id, own);
            }
            self.ospf_flood(dev, pkt.lsa, Some(port));
            self.ospf_recompute_all();
        }
    }

    fn on_ospf_timer(&mut self, dev: DevId) {
        if !self.ospf_enabled(dev) {
            return;
        }
        if let Some(rt) = self.ospf.get_mut(&dev) {
            rt.timer_pending = false;
        }
        let now = self.now;
        let expired: Vec<Ipv4Addr> = self
            .ospf
            .get(&dev)
            .map(|rt| {
                rt.neighbors
                    .iter()
                    .filter(|(_, nb)| now.saturating_sub(nb.last_ns) > OSPF_DEAD_NS)
                    .map(|(rid, _)| *rid)
                    .collect()
            })
            .unwrap_or_default();
        if !expired.is_empty() {
            if let Some(rt) = self.ospf.get_mut(&dev) {
                for rid in expired {
                    rt.neighbors.remove(&rid);
                }
            }
            let lsa = self.ospf_build_lsa(dev);
            if let Some(rt) = self.ospf.get_mut(&dev) {
                rt.lsdb.insert(lsa.router_id, lsa);
            }
            self.ospf_flood(dev, lsa, None);
            self.ospf_recompute_all();
        }
        self.ospf_send_hello(dev);
        self.ospf_schedule(dev);
    }

    /// 对每个 OSPF 路由器跑一次 SPF 并写回动态路由。
    fn ospf_recompute_all(&mut self) {
        let devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Router(rs) if !rs.ospf_networks.is_empty()))
            .map(|d| d.id)
            .collect();
        for dev in &devs {
            let lsa = self.ospf_build_lsa(*dev);
            self.ospf
                .entry(*dev)
                .or_default()
                .lsdb
                .insert(lsa.router_id, lsa);
        }
        for dev in devs {
            let routes = self.ospf_spf(dev);
            if let Some(d) = self.net.device_mut(dev) {
                if let DeviceState::Router(rs) = &mut d.state {
                    rs.ospf_routes = routes;
                }
            }
        }
    }

    fn ospf_spf(&self, dev: DevId) -> Vec<Route> {
        let Some(rt) = self.ospf.get(&dev) else {
            return vec![];
        };
        let self_id = self.ospf_router_id(dev);
        let mut dist: std::collections::HashMap<Ipv4Addr, u32> = std::collections::HashMap::new();
        let mut first: std::collections::HashMap<Ipv4Addr, Ipv4Addr> =
            std::collections::HashMap::new();
        let mut done: std::collections::HashSet<Ipv4Addr> = Default::default();
        dist.insert(self_id, 0);
        loop {
            let next = dist
                .iter()
                .filter(|(rid, _)| !done.contains(*rid))
                .min_by_key(|(_, d)| **d)
                .map(|(rid, d)| (*rid, *d));
            let Some((u, du)) = next else { break };
            done.insert(u);
            let Some(lsa) = rt.lsdb.get(&u) else { continue };
            for i in 0..lsa.neighbor_count as usize {
                let v = lsa.neighbors[i];
                if v == 0 || !rt.lsdb.contains_key(&v) {
                    continue;
                }
                let nd = du + 1;
                if nd < *dist.get(&v).unwrap_or(&u32::MAX) {
                    dist.insert(v, nd);
                    let hop = if u == self_id {
                        v
                    } else {
                        *first.get(&u).unwrap_or(&v)
                    };
                    first.insert(v, hop);
                }
            }
        }
        let mut routes = vec![];
        let mut seen: std::collections::HashSet<(Ipv4Addr, u8)> = Default::default();
        // 本机直连网段不重复装 OSPF 路由（连接路由优先）
        let connected: std::collections::HashSet<(Ipv4Addr, u8)> = self
            .net
            .device(dev)
            .map(|d| {
                d.connected_routes()
                    .iter()
                    .map(|r| (r.dst, r.len))
                    .collect()
            })
            .unwrap_or_default();
        for rid in dist.keys() {
            if *rid == self_id {
                continue;
            }
            let Some(hop) = first.get(rid) else { continue };
            let Some(nb) = rt.neighbors.get(hop) else {
                continue;
            };
            let Some(lsa) = rt.lsdb.get(rid) else {
                continue;
            };
            for i in 0..lsa.link_count as usize {
                let Some(link) = lsa.links[i] else { continue };
                if connected.contains(&(link.net, link.len)) || !seen.insert((link.net, link.len)) {
                    continue;
                }
                routes.push(Route {
                    dst: link.net,
                    len: link.len,
                    nexthop: nb.ip,
                    vlan: None,
                    port: 0,
                    src: RouteSrc::Ospf,
                });
            }
        }
        routes
    }

    // ---------- OSPFv3（简化：邻居前缀学习 + IPv6 路由） ----------

    pub fn ospfv3_config_changed(&mut self, dev: DevId) {
        if self.ospfv3_enabled_ports(dev).is_empty() {
            return;
        }
        self.ospfv3.entry(dev).or_default();
        self.ospfv3_send_update(dev);
        self.ospfv3_schedule(dev);
    }

    pub fn ospfv3_router_id_of(&self, dev: DevId) -> Ipv4Addr {
        self.ospfv3_router_id(dev)
    }

    /// 存活邻居：(router id, 邻居接口 IPv6, 本地接口)。
    pub fn ospfv3_neighbors_of(&self, dev: DevId) -> Vec<(Ipv4Addr, Ipv6Addr, PortId)> {
        let now = self.now;
        let mut v: Vec<_> = self
            .ospfv3
            .get(&dev)
            .map(|rt| {
                rt.neighbors
                    .iter()
                    .filter(|(_, nb)| now.saturating_sub(nb.last_ns) <= OSPFV3_DEAD_NS)
                    .map(|(rid, nb)| (*rid, nb.ip, nb.port))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|x| x.0);
        v
    }

    /// 本机 OSPFv3 前缀（show 用）。
    pub fn ospfv3_prefixes_of(&self, dev: DevId) -> Vec<(Ipv6Addr, u8)> {
        let mut v: Vec<_> = self
            .ospfv3_enabled_ports(dev)
            .into_iter()
            .map(|(_, ip, len)| (ip, len))
            .collect();
        v.sort_by_key(|a| a.0);
        v
    }

    fn ospfv3_enabled_ports(&self, dev: DevId) -> Vec<(PortId, Ipv6Addr, u8)> {
        let Some(d) = self.net.device(dev) else {
            return vec![];
        };
        if !matches!(&d.state, DeviceState::Router(rs) if rs.ospfv3_enabled) {
            return vec![];
        }
        d.ports
            .iter()
            .filter(|p| {
                p.admin_up
                    && p.link.is_some()
                    && p.ospfv3_area.is_some()
                    && p.ipv6.is_some()
                    && p.vrf.is_none()
            })
            .map(|p| (p.idx, p.ipv6.unwrap(), p.prefix6))
            .collect()
    }

    fn ospfv3_router_id(&self, dev: DevId) -> Ipv4Addr {
        let Some(d) = self.net.device(dev) else {
            return 0x0101_0101;
        };
        if let DeviceState::Router(rs) = &d.state {
            if let Some(id) = rs.ospfv3_router_id {
                return id;
            }
        }
        let mut best = 0u32;
        for p in &d.ports {
            if let Some(ip) = p.ip {
                best = best.max(ip);
            }
        }
        if best == 0 {
            0x0101_0101
        } else {
            best
        }
    }

    /// 构建 OSPFv3 Update：本机直连前缀 + 学到的路由（水平分割——不向"学到该路由的口"发回）。
    /// 这让 LSA 风格的前缀信息能穿越中间路由器（三台链式时 R3 也能学到 R1 的网段）。
    fn ospfv3_build_packet(&self, dev: DevId, exclude_port: Option<PortId>) -> Ospfv3Packet {
        let mut pkt = Ospfv3Packet {
            router_id: self.ospfv3_router_id(dev),
            area: 0,
            src: [0u8; 16],
            prefixes: [None; 8],
            count: 0,
        };
        let push = |pkt: &mut Ospfv3Packet, ip: Ipv6Addr, len: u8| {
            if pkt.count < 8 {
                pkt.prefixes[pkt.count as usize] = Some((ip, len));
                pkt.count += 1;
            }
        };
        let ports = self.ospfv3_enabled_ports(dev);
        for (_, ip, len) in &ports {
            push(&mut pkt, *ip, *len);
        }
        // learned 路由随 Update 泛洪；水平分割：来源口与发送口相同则跳过
        let source_port_of = |nexthop: Ipv6Addr| -> Option<PortId> {
            self.ospfv3
                .get(&dev)
                .and_then(|rt| rt.neighbors.values().find(|nb| nb.ip == nexthop))
                .map(|nb| nb.port)
        };
        if let Some(d) = self.net.device(dev) {
            if let DeviceState::Router(rs) = &d.state {
                for r in &rs.ospfv3_routes6 {
                    if Some(r.src) != Some(RouteSrc::Ospf) {
                        continue;
                    }
                    if exclude_port == source_port_of(r.nexthop) {
                        continue;
                    }
                    push(&mut pkt, r.dst, r.len);
                }
            }
        }
        pkt
    }

    fn ospfv3_send_update(&mut self, dev: DevId) {
        let ports = self.ospfv3_enabled_ports(dev);
        for (port, ip, _) in ports {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let Some(p) = d.port(port) else { continue };
            let mut pkt = self.ospfv3_build_packet(dev, Some(port));
            pkt.src = ip;
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: p.mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV6,
                payload: L3::Ospfv3(pkt),
            };
            self.transmit_frame(dev, port, frame);
        }
    }

    fn ospfv3_schedule(&mut self, dev: DevId) {
        let need = {
            let rt = self.ospfv3.entry(dev).or_default();
            if rt.timer_pending {
                false
            } else {
                rt.timer_pending = true;
                true
            }
        };
        if need {
            self.push(self.now + OSPFV3_HELLO_NS, EKind::Ospfv3Timer { dev });
        }
    }

    fn ospfv3_on_packet(&mut self, dev: DevId, port: PortId, pkt: Ospfv3Packet) {
        let enabled = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            matches!(&d.state, DeviceState::Router(rs) if rs.ospfv3_enabled)
                && d.port(port).and_then(|p| p.ospfv3_area).is_some()
        };
        if !enabled || pkt.router_id == self.ospfv3_router_id(dev) {
            return;
        }
        let mut prefixes = Vec::new();
        for i in 0..pkt.count.min(8) as usize {
            if let Some((ip, len)) = pkt.prefixes[i] {
                prefixes.push((ip, len));
            }
        }
        let rt = self.ospfv3.entry(dev).or_default();
        rt.neighbors.insert(
            pkt.router_id,
            Ospfv3Neighbor {
                ip: pkt.src,
                port,
                last_ns: self.now,
                prefixes,
            },
        );
        self.ospfv3_recompute(dev);
    }

    fn on_ospfv3_timer(&mut self, dev: DevId) {
        let ports_empty = self.ospfv3_enabled_ports(dev).is_empty();
        if ports_empty {
            if let Some(rt) = self.ospfv3.get_mut(&dev) {
                rt.timer_pending = false;
                rt.neighbors.clear();
            }
            return;
        }
        let now = self.now;
        if let Some(rt) = self.ospfv3.get_mut(&dev) {
            rt.neighbors
                .retain(|_, nb| now.saturating_sub(nb.last_ns) <= OSPFV3_DEAD_NS);
            rt.timer_pending = false;
        }
        self.ospfv3_recompute(dev);
        self.ospfv3_send_update(dev);
        self.ospfv3_schedule(dev);
    }

    fn ospfv3_recompute(&mut self, dev: DevId) {
        let connected: std::collections::HashSet<(Ipv6Addr, u8)> = self
            .net
            .device(dev)
            .map(|d| {
                d.ports
                    .iter()
                    .filter_map(|p| p.ipv6.map(|ip| (ip, p.prefix6)))
                    .collect()
            })
            .unwrap_or_default();
        let mut routes: Vec<Route6> = Vec::new();
        let mut seen: std::collections::HashSet<(Ipv6Addr, u8)> = Default::default();
        if let Some(rt) = self.ospfv3.get(&dev) {
            for nb in rt.neighbors.values() {
                for (net, len) in &nb.prefixes {
                    let masked = *net;
                    if connected.contains(&(masked, *len)) || !seen.insert((masked, *len)) {
                        continue;
                    }
                    routes.push(Route6 {
                        dst: masked,
                        len: *len,
                        nexthop: nb.ip,
                        src: RouteSrc::Ospf,
                    });
                }
            }
        }
        if let Some(d) = self.net.device_mut(dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.ospfv3_routes6 = routes;
            }
        }
    }

    // ---------- IPv6（简化：NDP + ICMPv6 ping + 静态路由） ----------

    pub fn start_ping6(&mut self, dev: DevId, target: Ipv6Addr) -> Option<u16> {
        let kind = d_kind(&self.net, dev);
        if !matches!(
            kind,
            DeviceKind::Router
                | DeviceKind::Host
                | DeviceKind::Firewall
                | DeviceKind::WlanController
        ) {
            return None;
        }
        let sid = {
            let d = self.net.device_mut(dev)?;
            let (pings, next) = match &mut d.state {
                DeviceState::Router(rs) => (&mut rs.pings6, &mut rs.next_ping6_id),
                DeviceState::Host(hs) => (&mut hs.pings6, &mut hs.next_ping6_id),
                _ => return None,
            };
            let id = (*next & 0xffff) as u16;
            *next += 1;
            pings.push(Ping6Session {
                id,
                target,
                sent: 0,
                received: 0,
                done: false,
                reported: false,
            });
            id
        };
        self.push(self.now, EKind::Ping6Send { dev, sid, probe: 0 });
        Some(sid)
    }

    fn ping6_session_mut(&mut self, dev: DevId, sid: u16) -> Option<&mut Ping6Session> {
        let d = self.net.device_mut(dev)?;
        let pings = match &mut d.state {
            DeviceState::Router(rs) => &mut rs.pings6,
            DeviceState::Host(hs) => &mut hs.pings6,
            _ => return None,
        };
        pings.iter_mut().find(|s| s.id == sid)
    }

    /// 返回 (出端口, 下一跳地址, 源地址)。
    fn route_for_ping6(
        &self,
        dev: DevId,
        target: Ipv6Addr,
    ) -> Option<(PortId, Ipv6Addr, Ipv6Addr)> {
        let d = self.net.device(dev)?;
        match &d.state {
            DeviceState::Host(hs) => {
                let p = &d.ports[0];
                let ip = p.ipv6?;
                let arp_for = if ipv6_in_prefix(&target, &ip, p.prefix6) {
                    target
                } else {
                    hs.gw6?
                };
                Some((0, arp_for, ip))
            }
            DeviceState::Router(_) => {
                let (port, nh) = self.net.lookup_route6(dev, target)?;
                let src = d.port(port).and_then(|p| p.ipv6)?;
                Some((port, nh, src))
            }
            _ => None,
        }
    }

    fn on_ping6_send(&mut self, dev: DevId, sid: u16, probe: u32) {
        let now = self.now;
        let (target, sent) = {
            let Some(s) = self.ping6_session_mut(dev, sid) else {
                return;
            };
            if s.done || probe != s.sent || probe >= Ping6Session::TOTAL {
                return;
            }
            s.sent += 1;
            (s.target, s.sent)
        };
        if let Some((port, arp_for, src)) = self.route_for_ping6(dev, target) {
            let pkt = Ipv6Packet {
                src,
                dst: target,
                hop_limit: 64,
                icmp: Ipv6Icmp::EchoRequest {
                    id: sid,
                    seq: probe as u16,
                },
            };
            self.send_ipv6(dev, port, pkt, arp_for);
        }
        if sent < Ping6Session::TOTAL {
            self.push(
                now + PingSession::INTERVAL_NS,
                EKind::Ping6Send {
                    dev,
                    sid,
                    probe: probe + 1,
                },
            );
        } else {
            self.push(now + PingSession::TIMEOUT_NS, EKind::Ping6Done { dev, sid });
        }
    }

    fn on_ping6_done(&mut self, dev: DevId, sid: u16) {
        if let Some(s) = self.ping6_session_mut(dev, sid) {
            s.done = true;
        }
    }

    /// 取已结束未输出的 ping6 会话。
    pub fn take_unreported_ping6(&mut self, dev: DevId) -> Vec<Ping6Session> {
        let Some(d) = self.net.device_mut(dev) else {
            return vec![];
        };
        let pings = match &mut d.state {
            DeviceState::Router(rs) => &mut rs.pings6,
            DeviceState::Host(hs) => &mut hs.pings6,
            _ => return vec![],
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

    /// 本机在该端口上的 IPv6 地址。
    /// 地址是否为本机任一接口（端口 v6 地址，路由器含本地 SRv6 SID/Locator 内地址）。
    /// 真实路由器上目的为任一本机地址的报文都应上送，而非按转发表从其他口发回给自己。
    fn my_ipv6_any(&self, dev: DevId, addr: &Ipv6Addr) -> bool {
        let Some(d) = self.net.device(dev) else {
            return false;
        };
        if d.ports.iter().any(|p| p.ipv6 == Some(*addr)) {
            return true;
        }
        match &d.state {
            DeviceState::Router(rs) => {
                rs.srv6.sids.contains(addr)
                    || rs
                        .srv6
                        .locator
                        .map(|(net, len)| ipv6_in_prefix(addr, &net, len))
                        .unwrap_or(false)
            }
            _ => false,
        }
    }

    fn ndp_lookup(&self, dev: DevId, port: PortId, ip: Ipv6Addr) -> Option<NdpEntry> {
        let d = self.net.device(dev)?;
        match &d.state {
            DeviceState::Router(rs) => rs.ndp.get(&(port, ip)).copied(),
            DeviceState::Host(hs) => hs.ndp.get(&ip).copied(),
            _ => None,
        }
    }

    fn ndp_learn(&mut self, dev: DevId, port: PortId, ip: Ipv6Addr, mac: Mac) {
        {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            match &mut d.state {
                DeviceState::Router(rs) => {
                    rs.ndp.insert((port, ip), NdpEntry { mac, port });
                    rs.ndping.remove(&(port, ip));
                }
                DeviceState::Host(hs) => {
                    hs.ndp.insert(ip, NdpEntry { mac, port });
                    hs.ndping.remove(&ip);
                }
                _ => return,
            }
        }
        self.flush_pending6(dev, ip, mac);
    }

    fn flush_pending6(&mut self, dev: DevId, ip: Ipv6Addr, mac: Mac) {
        let ready: Vec<Pending6> = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            match &mut d.state {
                DeviceState::Router(rs) => {
                    let (ok, rest): (Vec<_>, Vec<_>) =
                        rs.pending6.drain(..).partition(|p| p.dst_ip == ip);
                    rs.pending6 = rest;
                    ok
                }
                DeviceState::Host(hs) => {
                    let (ok, rest): (Vec<_>, Vec<_>) =
                        hs.pending6.drain(..).partition(|p| p.dst_ip == ip);
                    hs.pending6 = rest;
                    ok
                }
                _ => return,
            }
        };
        for p in ready {
            let frame = EthernetFrame {
                dst: mac,
                src: p.src_mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV6,
                payload: L3::Ipv6(p.pkt),
            };
            self.transmit_frame(dev, p.port, frame);
        }
    }

    /// 发送 IPv6（必要时先发 NS 解析邻居）。
    fn send_ipv6(&mut self, dev: DevId, port: PortId, pkt: Ipv6Packet, arp_for: Ipv6Addr) {
        let (mac, src_ip) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let Some(p) = d.port(port) else {
                return;
            };
            (p.mac, p.ipv6.unwrap_or([0u8; 16]))
        };
        if let Some(e) = self.ndp_lookup(dev, port, arp_for) {
            let frame = EthernetFrame {
                dst: e.mac,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV6,
                payload: L3::Ipv6(pkt),
            };
            self.transmit_frame(dev, port, frame);
            return;
        }
        let need_ns = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            match &mut d.state {
                DeviceState::Router(rs) => {
                    rs.pending6.push(Pending6 {
                        port,
                        dst_ip: arp_for,
                        src_ip: pkt.src,
                        src_mac: mac,
                        pkt,
                    });
                    rs.ndping.insert((port, arp_for))
                }
                DeviceState::Host(hs) => {
                    hs.pending6.push(Pending6 {
                        port,
                        dst_ip: arp_for,
                        src_ip: pkt.src,
                        src_mac: mac,
                        pkt,
                    });
                    hs.ndping.insert(arp_for)
                }
                _ => return,
            }
        };
        if need_ns {
            let ns = Ipv6Packet {
                src: src_ip,
                dst: ipv6_solicited_addr(&arp_for),
                hop_limit: 255,
                icmp: Ipv6Icmp::NeighborSolicitation { target: arp_for },
            };
            let frame = EthernetFrame {
                dst: ipv6_solicited_mac(&arp_for),
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_IPV6,
                payload: L3::Ipv6(ns),
            };
            self.transmit_frame(dev, port, frame);
        }
    }

    fn ipv6_on_frame(&mut self, dev: DevId, port: PortId, frame: EthernetFrame) {
        let L3::Ipv6(pkt) = frame.payload else {
            return;
        };
        let my_mac = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .map(|p| p.mac)
            .unwrap_or([0; 6]);
        match pkt.icmp {
            Ipv6Icmp::NeighborSolicitation { target } => {
                if pkt.src != [0u8; 16] && pkt.src != target {
                    self.ndp_learn(dev, port, pkt.src, frame.src);
                }
                if self.my_ipv6_any(dev, &target) {
                    let na = Ipv6Packet {
                        src: target,
                        dst: pkt.src,
                        hop_limit: 255,
                        icmp: Ipv6Icmp::NeighborAdvertisement { target },
                    };
                    let out = EthernetFrame {
                        dst: frame.src,
                        src: my_mac,
                        vlan: None,
                        ethertype: ETHERTYPE_IPV6,
                        payload: L3::Ipv6(na),
                    };
                    self.transmit_frame(dev, port, out);
                }
            }
            Ipv6Icmp::NeighborAdvertisement { target } => {
                self.ndp_learn(dev, port, target, frame.src);
            }
            Ipv6Icmp::EchoRequest { id, seq } => {
                if self.my_ipv6_any(dev, &pkt.dst) {
                    let reply = Ipv6Packet {
                        src: pkt.dst,
                        dst: pkt.src,
                        hop_limit: 64,
                        icmp: Ipv6Icmp::EchoReply { id, seq },
                    };
                    let out = EthernetFrame {
                        dst: frame.src,
                        src: my_mac,
                        vlan: None,
                        ethertype: ETHERTYPE_IPV6,
                        payload: L3::Ipv6(reply),
                    };
                    self.transmit_frame(dev, port, out);
                }
            }
            Ipv6Icmp::EchoReply { id, .. } => {
                if self.my_ipv6_any(dev, &pkt.dst) {
                    let matched = {
                        let Some(d) = self.net.device_mut(dev) else {
                            return;
                        };
                        let pings = match &mut d.state {
                            DeviceState::Router(rs) => &mut rs.pings6,
                            DeviceState::Host(hs) => &mut hs.pings6,
                            _ => return,
                        };
                        pings
                            .iter_mut()
                            .find(|s| s.id == id && s.target == pkt.src && !s.done)
                            .map(|s| {
                                s.received += 1;
                            })
                            .is_some()
                    };
                    let _ = matched;
                }
            }
        };
        // 数据转发：仅路由器
        if matches!(
            pkt.icmp,
            Ipv6Icmp::EchoRequest { .. } | Ipv6Icmp::EchoReply { .. }
        ) && !self.my_ipv6_any(dev, &pkt.dst)
        {
            let is_router = matches!(
                d_kind(&self.net, dev),
                DeviceKind::Router | DeviceKind::Firewall
            );
            if !is_router || pkt.hop_limit <= 1 {
                return;
            }
            if self.srv6_encapsulate(dev, pkt) {
                return;
            }
            if let Some((out_port, nh)) = self.net.lookup_route6(dev, pkt.dst) {
                let mut fwd = pkt;
                fwd.hop_limit -= 1;
                self.send_ipv6(dev, out_port, fwd, nh);
            } else {
                self.dropped += 1;
            }
        }
    }

    // ---------- SRv6 BE（教学用 IPv6-in-IPv6） ----------

    fn srv6_route_for(&self, dev: DevId, dst: Ipv6Addr) -> Option<Ipv6Addr> {
        let d = self.net.device(dev)?;
        let DeviceState::Router(rs) = &d.state else {
            return None;
        };
        rs.srv6
            .routes
            .iter()
            .filter(|r| ipv6_in_prefix(&dst, &r.dst, r.len))
            .max_by_key(|r| r.len)
            .map(|r| r.sid)
    }

    fn srv6_source(&self, dev: DevId) -> Option<Ipv6Addr> {
        let d = self.net.device(dev)?;
        let DeviceState::Router(rs) = &d.state else {
            return None;
        };
        if let Some((net, _)) = rs.srv6.locator {
            return Some(net);
        }
        d.ports.iter().find_map(|p| p.ipv6)
    }

    fn srv6_encapsulate(&mut self, dev: DevId, pkt: Ipv6Packet) -> bool {
        let Some(sid) = self.srv6_route_for(dev, pkt.dst) else {
            return false;
        };
        let Some(src) = self.srv6_source(dev) else {
            return false;
        };
        let outer = Srv6Packet {
            src,
            dst: sid,
            hop_limit: 64,
            inner: pkt,
        };
        self.send_srv6_outer(dev, outer);
        true
    }

    fn send_srv6_outer(&mut self, dev: DevId, mut pkt: Srv6Packet) {
        let Some((out_port, _)) = self.net.lookup_route6(dev, pkt.dst) else {
            self.dropped += 1;
            return;
        };
        let Some(mac) = self
            .net
            .device(dev)
            .and_then(|d| d.port(out_port))
            .map(|p| p.mac)
        else {
            return;
        };
        if pkt.hop_limit > 0 {
            pkt.hop_limit -= 1;
        }
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: mac,
            vlan: None,
            ethertype: ETHERTYPE_IPV6,
            payload: L3::Srv6(pkt),
        };
        self.transmit_frame(dev, out_port, frame);
    }

    fn srv6_ingress(&mut self, dev: DevId, port: PortId, pkt: Srv6Packet) {
        let local = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            match &d.state {
                DeviceState::Router(rs) => {
                    rs.srv6.sids.contains(&pkt.dst)
                        || rs
                            .srv6
                            .locator
                            .map(|(net, len)| ipv6_in_prefix(&pkt.dst, &net, len))
                            .unwrap_or(false)
                }
                _ => false,
            }
        };
        if local {
            let frame = EthernetFrame {
                dst: BROADCAST_MAC,
                src: BROADCAST_MAC,
                vlan: None,
                ethertype: ETHERTYPE_IPV6,
                payload: L3::Ipv6(pkt.inner),
            };
            self.ipv6_on_frame(dev, port, frame);
            return;
        }
        if pkt.hop_limit <= 1 {
            self.dropped += 1;
            return;
        }
        self.send_srv6_outer(dev, pkt);
    }

    // ---------- VRRP（主备虚拟网关，简化） ----------

    pub fn vrrp_config_changed(&mut self, dev: DevId) {
        let has = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            let DeviceState::Router(rs) = &mut d.state else {
                return;
            };
            for g in &mut rs.vrrp {
                if g.state == VrrpState::Init {
                    g.state = VrrpState::Master; // 通过通告收敛
                }
            }
            !rs.vrrp.is_empty()
        };
        if has {
            self.vrrp_schedule(dev);
        }
    }

    fn vrrp_schedule(&mut self, dev: DevId) {
        if self.vrrp_pending.insert(dev) {
            self.push(self.now + VRRP_ADVERT_NS, EKind::VrrpTimer { dev });
        }
    }

    fn on_vrrp_timer(&mut self, dev: DevId) {
        self.vrrp_pending.remove(&dev);
        let now = self.now;
        let mut sends: Vec<(PortId, Mac, VrrpPacket)> = vec![];
        {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            let ports = d.ports.clone();
            let DeviceState::Router(rs) = &mut d.state else {
                return;
            };
            for g in &mut rs.vrrp {
                if g.state == VrrpState::Backup
                    && now.saturating_sub(g.last_advert_ns) > VRRP_MASTER_DOWN_NS
                {
                    g.state = VrrpState::Master;
                }
                if g.state != VrrpState::Master {
                    continue;
                }
                let Some(p) = ports.get(g.port) else { continue };
                let Some(ip) = p.ip else { continue };
                sends.push((
                    g.port,
                    p.mac,
                    VrrpPacket {
                        vrid: g.vrid,
                        priority: g.priority,
                        vip: g.vip,
                        master_ip: ip,
                    },
                ));
            }
        }
        for (port, mac, pkt) in sends {
            let frame = EthernetFrame {
                dst: VRRP_MULTICAST_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_VRRP,
                payload: L3::Vrrp(pkt),
            };
            self.transmit_frame(dev, port, frame);
        }
        let has = self
            .net
            .device(dev)
            .map(|d| matches!(&d.state, DeviceState::Router(rs) if !rs.vrrp.is_empty()))
            .unwrap_or(false);
        if has {
            self.vrrp_schedule(dev);
        }
    }

    fn vrrp_on_packet(&mut self, dev: DevId, pkt: VrrpPacket) {
        let now = self.now;
        let Some(d) = self.net.device_mut(dev) else {
            return;
        };
        let ports = d.ports.clone();
        let DeviceState::Router(rs) = &mut d.state else {
            return;
        };
        let Some(g) = rs
            .vrrp
            .iter_mut()
            .find(|g| g.vrid == pkt.vrid && g.vip == pkt.vip)
        else {
            return;
        };
        g.last_advert_ns = now;
        let my_ip = ports.get(g.port).and_then(|p| p.ip).unwrap_or(0);
        let remote_higher =
            pkt.priority > g.priority || (pkt.priority == g.priority && pkt.master_ip > my_ip);
        if remote_higher {
            g.state = VrrpState::Backup;
        } else if g.preempt && g.state == VrrpState::Backup {
            g.state = VrrpState::Master;
        }
    }

    /// 该接口上是否由本机充当该 VIP 的 Master，返回虚拟 MAC。
    fn vrrp_vip_owner(&self, dev: DevId, port: PortId, vip: Ipv4Addr) -> Option<Mac> {
        let d = self.net.device(dev)?;
        let DeviceState::Router(rs) = &d.state else {
            return None;
        };
        rs.vrrp
            .iter()
            .find(|g| g.state == VrrpState::Master && g.port == port && g.vip == vip)
            .map(|g| vrrp_mac(g.vrid))
    }

    /// 该端口上本机是否拥有这个虚拟 MAC（Master 才应答）。
    fn is_vrrp_virtual_mac_on(&self, dev: DevId, port: PortId, mac: &Mac) -> bool {
        let Some(d) = self.net.device(dev) else {
            return false;
        };
        let DeviceState::Router(rs) = &d.state else {
            return false;
        };
        rs.vrrp
            .iter()
            .any(|g| g.state == VrrpState::Master && g.port == port && vrrp_mac(g.vrid) == *mac)
    }

    // ---------- MPLS（静态 LSP 简化） ----------

    fn mpls_push(&mut self, dev: DevId, pkt: Ipv4Packet, vrf: Option<u16>) -> bool {
        let lsp = {
            let Some(d) = self.net.device(dev) else {
                return false;
            };
            let DeviceState::Router(rs) = &d.state else {
                return false;
            };
            let want_vrf = vrf
                .and_then(|id| rs.vrfs.iter().find(|v| v.id == id))
                .map(|v| v.name.clone());
            rs.mpls_lsps
                .iter()
                .find(|l| {
                    l.out_label.is_some()
                        && l.in_label.is_none()
                        && l.vrf == want_vrf
                        && l.net
                            .map(|(net, len)| (pkt.dst & mask_from_prefix(len)) == net)
                            .unwrap_or(false)
                })
                .cloned()
        };
        let Some(lsp) = lsp else { return false };
        let (Some(label), Some(nh)) = (lsp.out_label, lsp.nexthop) else {
            return false;
        };
        let Some((port, _, _)) = self.net.lookup_route(dev, nh) else {
            return false;
        };
        let Some(mac) = self
            .net
            .device(dev)
            .and_then(|d| d.port(port))
            .map(|p| p.mac)
        else {
            return false;
        };
        let vpn = lsp.vpn_label.filter(|l| *l != 0);
        let frame = EthernetFrame {
            dst: BROADCAST_MAC,
            src: mac,
            vlan: None,
            ethertype: ETHERTYPE_MPLS,
            payload: L3::Mpls(MplsPacket {
                labels: [label, vpn.unwrap_or(0)],
                depth: if vpn.is_some() { 2 } else { 1 },
                ip: pkt,
            }),
        };
        self.transmit_frame(dev, port, frame);
        true
    }

    fn mpls_on_frame(&mut self, dev: DevId, port: PortId, m: MplsPacket) {
        if m.depth == 0 {
            self.mpls_deliver(dev, port, m.ip, None);
            return;
        }
        let lsp = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Router(rs) = &d.state else {
                return;
            };
            rs.mpls_lsps
                .iter()
                .find(|l| l.in_label == Some(m.labels[0]))
                .cloned()
        };
        let Some(lsp) = lsp else {
            self.dropped += 1;
            return;
        };
        match (lsp.out_label, lsp.nexthop) {
            (Some(out), Some(nh)) => {
                let Some((out_port, _, _)) = self.net.lookup_route(dev, nh) else {
                    self.dropped += 1;
                    return;
                };
                let Some(mac) = self
                    .net
                    .device(dev)
                    .and_then(|d| d.port(out_port))
                    .map(|p| p.mac)
                else {
                    return;
                };
                let mut m2 = m;
                m2.labels[0] = out;
                let frame = EthernetFrame {
                    dst: BROADCAST_MAC,
                    src: mac,
                    vlan: None,
                    ethertype: ETHERTYPE_MPLS,
                    payload: L3::Mpls(m2),
                };
                self.transmit_frame(dev, out_port, frame);
            }
            _ => {
                // 弹签：若本标签是 VPN 标签则投递到对应 VRF；
                // 否则在双标签场景下继续处理内层标签（外层为传输标签）。
                let target_vrf = lsp
                    .vrf
                    .as_ref()
                    .and_then(|name| self.net.device(dev).and_then(|d| d.vrf_id(name)));
                if target_vrf.is_some() {
                    self.mpls_deliver(dev, port, m.ip, target_vrf);
                } else if m.depth > 1 {
                    let mut inner = m;
                    inner.labels[0] = inner.labels[1];
                    inner.labels[1] = 0;
                    inner.depth -= 1;
                    self.mpls_on_frame(dev, port, inner);
                } else {
                    self.mpls_deliver(dev, port, m.ip, None);
                }
            }
        }
    }

    fn mpls_deliver(&mut self, dev: DevId, port: PortId, pkt: Ipv4Packet, vrf: Option<u16>) {
        let (my_ip, reply_port) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            match vrf {
                Some(vid) => d
                    .ports
                    .iter()
                    .find(|p| p.vrf == Some(vid) && p.ip == Some(pkt.dst))
                    .map(|p| (Some(pkt.dst), p.idx))
                    .unwrap_or((None, port)),
                None => (d.port(port).and_then(|p| p.ip), port),
            }
        };
        if Some(pkt.dst) == my_ip {
            if pkt.icmp.is_reply {
                self.on_ping_reply(dev, pkt);
            } else {
                let my_mac = self
                    .net
                    .device(dev)
                    .and_then(|d| d.port(reply_port))
                    .map(|p| p.mac)
                    .unwrap_or([0; 6]);
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
                    dst: BROADCAST_MAC,
                    src: my_mac,
                    vlan: None,
                    ethertype: ETHERTYPE_IPV4,
                    payload: L3::Ipv4(reply),
                };
                self.transmit_frame(dev, reply_port, out);
            }
        } else {
            self.router_forward(dev, vrf, Some(port), pkt);
        }
    }

    pub fn mpls_lsps_of(&self, dev: DevId) -> Vec<StaticLsp> {
        match self.net.device(dev).map(|d| &d.state) {
            Some(DeviceState::Router(rs)) => rs.mpls_lsps.clone(),
            _ => vec![],
        }
    }

    // ---------- 远程管理（Telnet / SSH） ----------

    /// 找到拥有该管理 IP 的设备（物理口或启用的 SVI）。
    pub fn mgmt_ip_owner(&self, ip: Ipv4Addr) -> Option<DevId> {
        self.net.devices.iter().find_map(|d| {
            if d.ports.iter().any(|p| p.ip == Some(ip)) {
                return Some(d.id);
            }
            if let DeviceState::Switch(sw) = &d.state {
                if sw.svi.values().any(|s| s.admin_up && s.ip == ip) {
                    return Some(d.id);
                }
            }
            None
        })
    }

    /// 源设备到目标管理地址是否可达（路由存在；主机按同网段/有网关判断）。
    fn mgmt_reachable(&self, from: DevId, ip: Ipv4Addr) -> bool {
        let Some(d) = self.net.device(from) else {
            return false;
        };
        match &d.state {
            DeviceState::Host(h) => {
                let p = &d.ports[0];
                match p.ip {
                    Some(my) => {
                        let mask = mask_from_prefix(p.prefix);
                        (ip & mask) == (my & mask) || h.gw.is_some()
                    }
                    None => false,
                }
            }
            _ => self.net.lookup_route(from, ip).is_some(),
        }
    }

    /// 登录前置检查：找到目标、可达、VTY 允许该协议。成功返回目标设备 id。
    pub fn login_target(
        &self,
        from: DevId,
        target_ip: Ipv4Addr,
        proto: MgmtProto,
    ) -> Result<DevId, LoginError> {
        let target = self
            .mgmt_ip_owner(target_ip)
            .ok_or(LoginError::Unreachable)?;
        if target == from || !self.mgmt_reachable(from, target_ip) {
            return Err(LoginError::Unreachable);
        }
        let Some(d) = self.net.device(target) else {
            return Err(LoginError::Unreachable);
        };
        if !d.mgmt.transport.allows(proto) {
            return Err(LoginError::TransportDenied);
        }
        Ok(target)
    }

    /// 尝试一次远程登录并认证。成功返回 (目标设备 id, 用户权限级别)。
    pub fn try_login(
        &self,
        from: DevId,
        target_ip: Ipv4Addr,
        proto: MgmtProto,
        user: &str,
        pass: &str,
    ) -> Result<(DevId, u8), LoginError> {
        let target = self.login_target(from, target_ip, proto)?;
        let Some(d) = self.net.device(target) else {
            return Err(LoginError::Unreachable);
        };
        if !d.mgmt.auth_local {
            return Err(LoginError::AuthFailed);
        }
        let Some(u) = d.mgmt.find_user(user) else {
            return Err(LoginError::AuthFailed);
        };
        if u.secret != pass {
            return Err(LoginError::AuthFailed);
        }
        let svc_ok = match proto {
            MgmtProto::Telnet => u.telnet,
            MgmtProto::Ssh => u.ssh,
        };
        if !svc_ok {
            return Err(LoginError::AuthFailed);
        }
        Ok((target, u.privilege))
    }

    // ---------- 链路聚合（LACP） ----------

    /// 聚合组的逻辑端口（STP 视角）：优先最小选中成员，否则最小成员。
    pub fn logical_port(&self, dev: DevId, port: PortId) -> PortId {
        let Some(d) = self.net.device(dev) else {
            return port;
        };
        let Some(gid) = d.port_lag(port) else {
            return port;
        };
        let DeviceState::Switch(sw) = &d.state else {
            return port;
        };
        let Some(g) = sw.lags.get(&gid) else {
            return port;
        };
        if g.selected.is_empty() {
            g.members.iter().copied().min().unwrap_or(port)
        } else {
            g.selected.iter().copied().min().unwrap_or(port)
        }
    }

    /// STP/转发视角的逻辑端口列表：普通端口 + 每个聚合组的代表端口。
    fn logical_ports(&self, dev: DevId) -> Vec<PortId> {
        let Some(d) = self.net.device(dev) else {
            return vec![];
        };
        if !matches!(d.state, DeviceState::Switch(_)) {
            return (0..d.ports.len()).collect();
        }
        let mut out: Vec<PortId> = d
            .ports
            .iter()
            .filter(|p| p.lag.is_none() || self.logical_port(dev, p.idx) == p.idx)
            .map(|p| p.idx)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// 逻辑端口是否有可用链路（聚合组任一成员 up 即视为 up）。
    fn logical_link_up(&self, dev: DevId, port: PortId) -> bool {
        let Some(d) = self.net.device(dev) else {
            return false;
        };
        let up = |m: PortId| {
            d.port(m)
                .map(|p| p.admin_up && p.link.map(|l| self.net.link_up(l)).unwrap_or(false))
                .unwrap_or(false)
        };
        match d.port_lag(port) {
            Some(gid) => match &d.state {
                DeviceState::Switch(sw) => sw
                    .lags
                    .get(&gid)
                    .map(|g| g.members.iter().copied().any(up))
                    .unwrap_or(false),
                _ => up(port),
            },
            None => up(port),
        }
    }

    /// 配置变化后：重算成员、启动 LACP 定时器、触发 STP 重算。
    pub fn lacp_config_changed(&mut self, dev: DevId) {
        self.lacp_recompute(dev);
        self.lacp_schedule(dev);
        self.stp_topology_changed();
    }

    /// 摘要加载完成后初始化（LACP + STP 全量重算 + OSPF）。
    pub fn after_load(&mut self) {
        self.lacp_start_all();
        self.stp_topology_changed();
        let routers: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Router(rs) if !rs.ospf_networks.is_empty()))
            .map(|d| d.id)
            .collect();
        for dev in routers {
            self.ospf_config_changed(dev);
        }
        let vrrp_devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Router(rs) if !rs.vrrp.is_empty()))
            .map(|d| d.id)
            .collect();
        for dev in vrrp_devs {
            self.vrrp_config_changed(dev);
        }
        let bgp_devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Router(rs) if rs.bgp.is_some()))
            .map(|d| d.id)
            .collect();
        for dev in bgp_devs {
            self.bgp_config_changed(dev);
        }
        let isis_devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Router(rs) if rs.isis.is_some()))
            .map(|d| d.id)
            .collect();
        for dev in isis_devs {
            self.isis_config_changed(dev);
        }
        let evpn_devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Switch(sw) if sw.evpn.enabled))
            .map(|d| d.id)
            .collect();
        for dev in evpn_devs {
            self.evpn_config_changed(dev);
        }
        let ospfv3_devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| matches!(&d.state, DeviceState::Router(rs) if rs.ospfv3_enabled))
            .map(|d| d.id)
            .collect();
        for dev in ospfv3_devs {
            self.ospfv3_config_changed(dev);
        }
    }

    /// 加载拓扑后初始化聚合状态。
    pub fn lacp_start_all(&mut self) {
        let devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::Switch)
            .map(|d| d.id)
            .collect();
        for dev in devs {
            self.lacp_recompute(dev);
            self.lacp_schedule(dev);
        }
    }

    fn lacp_schedule(&mut self, dev: DevId) {
        let need = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return;
            };
            if sw.lags.is_empty() {
                false
            } else {
                let need = sw.lags.values().any(|g| !g.timer_pending);
                if need {
                    for g in sw.lags.values_mut() {
                        g.timer_pending = true;
                    }
                }
                need
            }
        };
        if need {
            self.push(self.now + LACP_INTERVAL_NS, EKind::LacpTimer { dev });
        }
    }

    /// 重算各聚合组选中成员；返回是否有变化。
    fn lacp_recompute(&mut self, dev: DevId) -> bool {
        let now = self.now;
        let my_mac = port_mac(dev, 0);
        let link_up: Vec<(PortId, bool)> = {
            let Some(d) = self.net.device(dev) else {
                return false;
            };
            if !matches!(d.state, DeviceState::Switch(_)) {
                return false;
            }
            d.ports
                .iter()
                .map(|p| {
                    (
                        p.idx,
                        p.admin_up && p.link.map(|l| self.net.link_up(l)).unwrap_or(false),
                    )
                })
                .collect()
        };
        let mut changed_members: Vec<PortId> = vec![];
        let changed = {
            let Some(d) = self.net.device_mut(dev) else {
                return false;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return false;
            };
            let mut changed = false;
            for g in sw.lags.values_mut() {
                // 老化：超时未收到 LACPDU 的对端信息失效
                let before = g.partners.len();
                g.partners
                    .retain(|_, pr| now.saturating_sub(pr.last_ns) <= LACP_TIMEOUT_NS);
                if g.partners.len() != before {
                    changed = true;
                }
                let mut selected: Vec<PortId> = vec![];
                for &m in &g.members {
                    let up = link_up
                        .iter()
                        .find(|(p, _)| *p == m)
                        .map(|(_, u)| *u)
                        .unwrap_or(false);
                    if !up {
                        continue;
                    }
                    let ok = match g.mode {
                        LagMode::Static => true,
                        _ => g
                            .partners
                            .get(&m)
                            .map(|pr| pr.key == g.id && pr.system != my_mac)
                            .unwrap_or(false),
                    };
                    if ok {
                        selected.push(m);
                    }
                }
                if selected != g.selected {
                    changed_members.extend(g.members.iter().copied());
                    g.selected = selected;
                    changed = true;
                }
            }
            changed
        };
        if changed {
            if let Some(d) = self.net.device_mut(dev) {
                if let DeviceState::Switch(sw) = &mut d.state {
                    sw.mac_table.retain(|_, p| !changed_members.contains(p));
                }
            }
            self.stp_topology_changed();
        }
        changed
    }

    fn lacp_send_group(&mut self, dev: DevId, gid: u16) {
        let my_mac = self
            .fabric_system_mac(dev)
            .unwrap_or_else(|| port_mac(dev, 0));
        let members: Vec<(PortId, Mac, LacpPartner, LagMode)> = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            let Some(g) = sw.lags.get(&gid) else {
                return;
            };
            g.members
                .iter()
                .filter_map(|m| {
                    let p = d.port(*m)?;
                    if !p.admin_up || p.link.is_none() {
                        return None;
                    }
                    Some((
                        *m,
                        p.mac,
                        g.partners.get(m).copied().unwrap_or(LacpPartner {
                            system: ZERO_MAC,
                            key: 0,
                            port: 0,
                            last_ns: 0,
                        }),
                        g.mode,
                    ))
                })
                .collect()
        };
        for (port, mac, partner, mode) in members {
            let mut state = LACP_STATE_AGGREGATION
                | LACP_STATE_SYNCHRONIZATION
                | LACP_STATE_COLLECTING
                | LACP_STATE_DISTRIBUTING;
            if mode == LagMode::LacpActive {
                state |= LACP_STATE_ACTIVITY;
            }
            let pdu = LacpPdu {
                actor_system: my_mac,
                actor_key: gid,
                actor_port: (port as u16) + 1,
                actor_state: state,
                partner_system: partner.system,
                partner_key: partner.key,
                partner_port: partner.port,
                partner_state: if partner.key != 0 {
                    LACP_STATE_SYNCHRONIZATION
                } else {
                    0
                },
            };
            let frame = EthernetFrame {
                dst: SLOW_PROTOCOLS_MAC,
                src: mac,
                vlan: None,
                ethertype: ETHERTYPE_LACP,
                payload: L3::Lacp(pdu),
            };
            self.transmit_frame(dev, port, frame);
        }
    }

    /// 收到 LACPDU：记录对端信息，被动模式立即回应，然后重选成员。
    fn lacp_on_pdu(&mut self, dev: DevId, port: PortId, pdu: LacpPdu) {
        let now = self.now;
        let gid = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            let Some(gid) = d.port_lag(port) else {
                return; // 未加入聚合组的端口不处理 LACPDU
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return;
            };
            let Some(g) = sw.lags.get_mut(&gid) else {
                return;
            };
            g.partners.insert(
                port,
                LacpPartner {
                    system: pdu.actor_system,
                    key: pdu.actor_key,
                    port: pdu.actor_port,
                    last_ns: now,
                },
            );
            gid
        };
        let passive = self
            .net
            .device(dev)
            .and_then(|d| match &d.state {
                DeviceState::Switch(sw) => {
                    sw.lags.get(&gid).map(|g| g.mode == LagMode::LacpPassive)
                }
                _ => None,
            })
            .unwrap_or(false);
        if passive {
            self.lacp_send_group(dev, gid);
        }
        self.lacp_recompute(dev);
    }

    fn on_lacp_timer(&mut self, dev: DevId) {
        let (has_groups, active_groups) = {
            let Some(d) = self.net.device_mut(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &mut d.state else {
                return;
            };
            for g in sw.lags.values_mut() {
                g.timer_pending = false;
            }
            let mut ids: Vec<u16> = sw
                .lags
                .values()
                .filter(|g| g.mode == LagMode::LacpActive)
                .map(|g| g.id)
                .collect();
            ids.sort_unstable();
            (!sw.lags.is_empty(), ids)
        };
        if !has_groups {
            return;
        }
        self.lacp_recompute(dev);
        for gid in active_groups {
            self.lacp_send_group(dev, gid);
        }
        self.lacp_schedule(dev);
    }

    /// 端口路径成本：覆盖值优先，否则按速率（1Gbps → 4）。聚合口取成员最小值。
    fn port_cost(&self, dev: DevId, port: PortId) -> u32 {
        let Some(d) = self.net.device(dev) else {
            return STP_COST_1G;
        };
        let cost_of = |m: PortId| {
            d.port(m)
                .map(|p| p.stp_cost.unwrap_or(STP_COST_1G))
                .unwrap_or(STP_COST_1G)
        };
        match d.port_lag(port) {
            Some(gid) => match &d.state {
                DeviceState::Switch(sw) => sw
                    .lags
                    .get(&gid)
                    .and_then(|g| g.members.iter().copied().map(cost_of).min())
                    .unwrap_or_else(|| cost_of(port)),
                _ => cost_of(port),
            },
            None => cost_of(port),
        }
    }

    /// 拓扑变化后由桥接层调用：清理死端口 BPDU、全量重算、调度 hello。
    pub fn stp_topology_changed(&mut self) {
        let devs: Vec<DevId> = self
            .net
            .devices
            .iter()
            .filter(|d| d.kind == DeviceKind::Switch)
            .map(|d| d.id)
            .collect();
        for dev in devs {
            // 逻辑端口失联（聚合组全部成员 down）→ BPDU 缓存立即失效
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
                    .filter(|p| !self.logical_link_up(dev, *p))
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
                sw.stp.last_bpdu_ns.remove(&p);
            }
        }
        self.stp_recompute_all();
    }

    /// 拓扑变化（前端增删设备/链路）后重算 STP 并启动定时器。
    pub fn topology_changed(&mut self) {
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

    /// 收到 BPDU：按实例号记录并触发对应实例重算。
    fn stp_on_bpdu(&mut self, dev: DevId, port: PortId, bpdu: BpduPacket) {
        let inst = bpdu.instance;
        if inst == 0 {
            if let Some(d) = self.net.device_mut(dev) {
                if let DeviceState::Switch(sw) = &mut d.state {
                    sw.stp.port_bpdus.insert(port, bpdu);
                    sw.stp.last_bpdu_ns.insert(port, self.now);
                }
            }
            self.stp_recompute(dev);
            return;
        }
        // MSTI：把实例现场换入 CIST 字段后复用同一套算法
        let mstp = self
            .net
            .device(dev)
            .map(|d| matches!(&d.state, DeviceState::Switch(sw) if sw.stp.mode == StpMode::Mstp))
            .unwrap_or(false);
        if !mstp {
            return;
        }
        if let Some(saved) = self.mst_swap_in(dev, inst) {
            if let Some(d) = self.net.device_mut(dev) {
                if let DeviceState::Switch(sw) = &mut d.state {
                    sw.stp.port_bpdus.insert(port, bpdu);
                    sw.stp.last_bpdu_ns.insert(port, self.now);
                }
            }
            self.stp_recompute(dev);
            self.mst_swap_out(dev, inst, saved);
        }
    }

    // ---------- MSTP 实例数据换入/换出 ----------

    fn switch_stp_mut(&mut self, dev: DevId) -> Option<&mut StpInstance> {
        match self.net.device_mut(dev).map(|d| &mut d.state) {
            Some(DeviceState::Switch(sw)) => Some(&mut sw.stp),
            _ => None,
        }
    }

    fn mst_swap_in(&mut self, dev: DevId, inst: u16) -> Option<CistSaved> {
        let stp = self.switch_stp_mut(dev)?;
        let m = stp.mst.get_mut(&inst)?;
        let saved = CistSaved {
            root: stp.root,
            root_cost: stp.root_cost,
            root_port: stp.root_port,
            ports: std::mem::take(&mut stp.ports),
            port_bpdus: std::mem::take(&mut stp.port_bpdus),
            last_bpdu_ns: std::mem::take(&mut stp.last_bpdu_ns),
            epochs: std::mem::take(&mut stp.epochs),
            priority: stp.priority,
        };
        stp.root = m.data.root;
        stp.root_cost = m.data.root_cost;
        stp.root_port = m.data.root_port;
        stp.ports = std::mem::take(&mut m.data.ports);
        stp.port_bpdus = std::mem::take(&mut m.data.port_bpdus);
        stp.last_bpdu_ns = std::mem::take(&mut m.data.last_bpdu_ns);
        stp.epochs = std::mem::take(&mut m.data.epochs);
        stp.priority = m.priority;
        Some(saved)
    }

    fn mst_swap_out(&mut self, dev: DevId, inst: u16, saved: CistSaved) {
        let Some(stp) = self.switch_stp_mut(dev) else {
            return;
        };
        if let Some(m) = stp.mst.get_mut(&inst) {
            m.priority = stp.priority;
            m.data.root = stp.root;
            m.data.root_cost = stp.root_cost;
            m.data.root_port = stp.root_port;
            m.data.ports = std::mem::take(&mut stp.ports);
            m.data.port_bpdus = std::mem::take(&mut stp.port_bpdus);
            m.data.last_bpdu_ns = std::mem::take(&mut stp.last_bpdu_ns);
            m.data.epochs = std::mem::take(&mut stp.epochs);
        }
        stp.root = saved.root;
        stp.root_cost = saved.root_cost;
        stp.root_port = saved.root_port;
        stp.ports = saved.ports;
        stp.port_bpdus = saved.port_bpdus;
        stp.last_bpdu_ns = saved.last_bpdu_ns;
        stp.epochs = saved.epochs;
        stp.priority = saved.priority;
    }

    /// 核心算法：根据各端口最优 BPDU 重选根桥、根端口与端口角色，并驱动状态机。
    fn stp_recompute(&mut self, dev: DevId) {
        // 阶段 1：读取现场
        let (my_bridge, enabled, n_ports, bpdus, rstp) = {
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
                matches!(sw.stp.mode, StpMode::Rstp | StpMode::Mstp),
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

        // 阶段 4：逐端口角色（聚合组只在代表端口上参与 STP）
        let stp_ports = self.logical_ports(dev);
        let mut roles: Vec<(PortId, StpRole)> = Vec::with_capacity(n_ports);
        for port in 0..n_ports {
            if !stp_ports.contains(&port) {
                roles.push((port, StpRole::Disabled));
                continue;
            }
            let has_link = self.logical_link_up(dev, port);
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
                        if rstp {
                            // RSTP：角色确定即转发（省略提案/同意握手）
                            StpSt::Forwarding
                        } else if old_state == StpSt::Blocking || epoch_bump {
                            // 新进入转发路径的端口从 Listening 开始
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
            if !rstp
                && cur_state == StpSt::Listening
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
        if sw.stp.mode == StpMode::Rstp {
            return; // RSTP 端口已直接进入转发，旧的推进事件无效
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
        self.stp_age_and_recompute(dev);
        self.stp_send_bpdus(dev, 0);
        // MSTP：每个实例独立老化、重算并发 BPDU（实例数据临时换入 CIST 字段）
        let mst_ids: Vec<u16> = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            if sw.stp.mode != StpMode::Mstp {
                vec![]
            } else {
                let mut v: Vec<u16> = sw.stp.mst.keys().copied().collect();
                v.sort_unstable();
                v
            }
        };
        for inst in mst_ids {
            if let Some(saved) = self.mst_swap_in(dev, inst) {
                self.stp_age_and_recompute(dev);
                self.stp_send_bpdus(dev, inst);
                self.mst_swap_out(dev, inst, saved);
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

    /// Max-Age 老化 + 重算（作用于当前装在 CIST 字段里的实例）。
    fn stp_age_and_recompute(&mut self, dev: DevId) {
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
        }
        // 每个 hello 周期都重算一次：新建的 MSTI 实例没有历史 BPDU，也需要完成首次选举
        self.stp_recompute(dev);
    }

    /// 从当前实例的指定端口发送配置 BPDU（instance=0 为 CIST）。
    fn stp_send_bpdus(&mut self, dev: DevId, instance: u16) {
        let (my_bridge, root, root_cost) = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            (sw.stp.bridge_id(dev), sw.stp.root, sw.stp.root_cost)
        };
        let designated: Vec<PortId> = {
            let Some(d) = self.net.device(dev) else {
                return;
            };
            let DeviceState::Switch(sw) = &d.state else {
                return;
            };
            self.logical_ports(dev)
                .into_iter()
                .filter(|&p| sw.stp.role_of(p) == StpRole::Designated)
                .collect()
        };
        for port in designated {
            let ready = self
                .net
                .device(dev)
                .map(|d| {
                    let link_ok = d
                        .port(port)
                        .map(|p| p.link.is_some() && p.admin_up)
                        .unwrap_or(false);
                    // 聚合口在成员未选中前不发 BPDU（等同逻辑链路未 up）
                    let lag_ok = match d.port_lag(port) {
                        Some(gid) => match &d.state {
                            DeviceState::Switch(sw) => sw
                                .lags
                                .get(&gid)
                                .map(|g| !g.selected.is_empty())
                                .unwrap_or(false),
                            _ => true,
                        },
                        None => true,
                    };
                    link_ok && lag_ok
                })
                .unwrap_or(false);
            if ready {
                let bpdu = BpduPacket {
                    root,
                    cost: root_cost,
                    sender: my_bridge,
                    port: port as u32,
                    instance,
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
    }
}

fn d_kind(net: &Net, dev: DevId) -> DeviceKind {
    net.device(dev)
        .map(|d| d.kind)
        .unwrap_or(DeviceKind::Router)
}
