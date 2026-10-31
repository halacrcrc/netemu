//! 设备、接口与各类型设备的状态模型（厂商中立）。
//!
//! 本模拟器中 `Switch` 即三层交换机：具备 VLAN/SVI/静态路由能力，
//! 纯二层行为（MAC 学习、洪泛）与三层转发共存于同一设备。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sim_packets::*;

pub type DevId = u32;
pub type PortId = usize;
pub type LinkId = u32;
pub type VlanId = u16;

pub const ROUTER_PORTS: usize = 4;
pub const SWITCH_PORTS: usize = 8;
pub const HOST_PORTS: usize = 1;
pub const PORT_SPEED_BPS: u64 = 1_000_000_000;
/// 默认 VLAN（Trunk 原生 VLAN / Access 缺省归属）。
pub const DEFAULT_VLAN: VlanId = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceKind {
    Router,
    Switch,
    Host,
}

impl DeviceKind {
    pub fn port_count(self) -> usize {
        match self {
            DeviceKind::Router => ROUTER_PORTS,
            DeviceKind::Switch => SWITCH_PORTS,
            DeviceKind::Host => HOST_PORTS,
        }
    }
}

/// CLI 方言。设备状态本身不依赖方言，方言只决定命令语法与输出格式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dialect {
    /// 思科 IOS 风格
    #[default]
    Ios,
    /// 华为 VRP 风格
    Vrp,
    /// 新华三 Comware 风格
    Comware,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteSrc {
    Connected,
    Static,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub dst: Ipv4Addr,
    pub len: u8,
    pub nexthop: Ipv4Addr,
    /// 出接口关联的 VLAN：SVI/（未来子接口）路由携带；物理口路由为 None。
    pub vlan: Option<VlanId>,
    pub port: PortId,
    pub src: RouteSrc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArpEntry {
    pub mac: Mac,
    pub port: PortId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PingProbeState {
    Pending,
    Replied { rtt_ns: u64, ttl: u8 },
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingProbe {
    pub state: PingProbeState,
    pub sent_ns: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PingSession {
    pub id: u16,
    pub target: Ipv4Addr,
    pub total: u32,
    pub probes: Vec<PingProbe>,
    /// 下一个待发送的序号
    pub next_probe: u32,
    pub done: bool,
    /// CLI/检查是否已输出过结果
    pub reported: bool,
}

impl PingSession {
    pub const TOTAL: u32 = 5;
    pub const INTERVAL_NS: u64 = 1_000_000_000;
    pub const TIMEOUT_NS: u64 = 2_000_000_000;

    pub fn new(id: u16, target: Ipv4Addr) -> Self {
        Self {
            id,
            target,
            total: Self::TOTAL,
            probes: vec![
                PingProbe {
                    state: PingProbeState::Pending,
                    sent_ns: 0
                };
                Self::TOTAL as usize
            ],
            next_probe: 0,
            done: false,
            reported: false,
        }
    }

    pub fn received(&self) -> u32 {
        self.probes
            .iter()
            .filter(|p| matches!(p.state, PingProbeState::Replied { .. }))
            .count() as u32
    }

    pub fn all_resolved(&self) -> bool {
        self.probes
            .iter()
            .all(|p| !matches!(p.state, PingProbeState::Pending))
    }

    pub fn rtt_ms_triplet(&self) -> Option<(f64, f64, f64)> {
        let rtts: Vec<f64> = self
            .probes
            .iter()
            .filter_map(|p| match p.state {
                PingProbeState::Replied { rtt_ns, .. } => Some(rtt_ns as f64 / 1e6),
                _ => None,
            })
            .collect();
        if rtts.is_empty() {
            return None;
        }
        let min = rtts.iter().cloned().fold(f64::MAX, f64::min);
        let max = rtts.iter().cloned().fold(f64::MIN, f64::max);
        let avg = rtts.iter().sum::<f64>() / rtts.len() as f64;
        Some((min, avg, max))
    }
}

/// 等 ARP 解析后再封装发出的报文。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingFrame {
    pub port: PortId,
    /// 出方向 VLAN（SVI 源发的报文携带）。
    pub vlan: Option<VlanId>,
    pub dst_ip: Ipv4Addr,
    pub src_mac: Mac,
    pub pkt: Ipv4Packet,
}

/// DHCP 地址池（按接收接口网段匹配）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhcpPool {
    pub name: String,
    /// 池网段
    pub network: Ipv4Addr,
    pub prefix: u8,
    /// 下发的网关（通常是路由器接口地址）
    pub gateway: Ipv4Addr,
    /// 下发的 DNS
    pub dns: Ipv4Addr,
}

/// DHCP 服务器已分配的租约。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhcpLease {
    pub mac: Mac,
    pub ip: Ipv4Addr,
}

/// ACL 规则（通配符掩码，0=精确匹配 1=忽略）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AclProto {
    Any,
    Icmp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclRule {
    pub permit: bool,
    pub proto: AclProto,
    pub src: Ipv4Addr,
    pub src_wildcard: u32,
    pub dst: Ipv4Addr,
    pub dst_wildcard: u32,
    /// 命中计数（教学展示用）
    pub hits: u64,
}

impl AclRule {
    /// 数据报文是否命中本条规则（M2b 仅 ICMP/任意协议区分）。
    pub fn matches(&self, src: Ipv4Addr, dst: Ipv4Addr, is_icmp: bool) -> bool {
        let proto_ok = match self.proto {
            AclProto::Any => true,
            AclProto::Icmp => is_icmp,
        };
        proto_ok
            && (src & self.src_wildcard) == (self.src & self.src_wildcard)
            && (dst & self.dst_wildcard) == (self.dst & self.dst_wildcard)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acl {
    pub number: u32,
    pub rules: Vec<AclRule>,
}

impl Acl {
    /// 首条命中规则决定结果；无命中 = 隐式拒绝（真实语义）。
    pub fn decide(&mut self, src: Ipv4Addr, dst: Ipv4Addr, is_icmp: bool) -> Option<bool> {
        for r in &mut self.rules {
            if r.matches(src, dst, is_icmp) {
                r.hits += 1;
                return Some(r.permit);
            }
        }
        None
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RouterState {
    /// 仅静态路由；连接路由由接口配置实时派生。
    pub static_routes: Vec<Route>,
    /// DHCP 服务器：地址池与租约
    pub dhcp_pools: Vec<DhcpPool>,
    pub dhcp_leases: Vec<DhcpLease>,
    pub next_dhcp_txid: u32,
    /// ACL 列表（编号 1-99 基础 / 100-199 高级）
    pub acls: Vec<Acl>,
    /// NAT：内外接口与转换表
    pub nat_inside: Vec<PortId>,
    pub nat_outside: Vec<PortId>,
    /// EasyIP（PAT）：内部 (源IP, ICMP id) → 出口 IP
    pub nat_entries: Vec<NatEntry>,
    /// 静态 NAT：inside_ip → outside_ip
    pub nat_static: Vec<(Ipv4Addr, Ipv4Addr)>,
    /// ARP 表按 (接口, VLAN) 隔离。
    pub arp: HashMap<(PortId, Option<VlanId>, Ipv4Addr), ArpEntry>,
    /// 已发出 ARP 请求、等待解析的 (端口, VLAN, 目标IP)，防止重复请求。
    pub arping: std::collections::HashSet<(PortId, Option<VlanId>, Ipv4Addr)>,
    pub pending: Vec<PendingFrame>,
    pub pings: Vec<PingSession>,
    pub next_ping_id: u32,
}

/// STP 端口角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StpRole {
    /// 指定端口
    Designated,
    /// 根端口
    Root,
    /// 替换端口（阻塞）
    Alternate,
    /// STP 关闭或端口无链路
    Disabled,
}

/// STP 端口状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StpSt {
    Blocking,
    Listening,
    Learning,
    Forwarding,
}

/// 单台交换机的 STP 实例（M2a：全网单实例，类 CST；MSTP 后续里程碑）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StpInstance {
    pub enabled: bool,
    pub priority: u16,
    /// 当前认知的根桥
    pub root: sim_packets::BridgeId,
    /// 本机到根的路径成本（自身为根时 0）
    pub root_cost: u32,
    pub root_port: Option<PortId>,
    /// 每端口角色/状态
    pub ports: HashMap<PortId, StpPortSt>,
    /// 每端口收到的最优 BPDU（用于角色计算）
    pub port_bpdus: HashMap<PortId, sim_packets::BpduPacket>,
    /// 每端口最近一次收到 BPDU 的虚拟时间（max-age 老化用）
    pub last_bpdu_ns: HashMap<PortId, u64>,
    /// 状态推进事件的纪元（角色变化后旧事件作废）
    pub epochs: HashMap<PortId, u32>,
    /// hello 定时器是否已在队列中
    pub hello_pending: bool,
}

impl Default for StpInstance {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: 32768,
            root: sim_packets::BridgeId {
                priority: 32768,
                mac: [0xff; 6],
            },
            root_cost: 0,
            root_port: None,
            ports: HashMap::new(),
            port_bpdus: HashMap::new(),
            last_bpdu_ns: HashMap::new(),
            epochs: HashMap::new(),
            hello_pending: false,
        }
    }
}

impl StpInstance {
    pub fn bridge_id(&self, dev: DevId) -> sim_packets::BridgeId {
        sim_packets::BridgeId::new(self.priority, port_mac(dev, 0))
    }

    pub fn role_of(&self, port: PortId) -> StpRole {
        self.ports
            .get(&port)
            .map(|p| p.role)
            .unwrap_or(StpRole::Disabled)
    }

    pub fn state_of(&self, port: PortId) -> StpSt {
        self.ports
            .get(&port)
            .map(|p| p.state)
            .unwrap_or(StpSt::Blocking)
    }

    pub fn epoch_of(&self, port: PortId) -> u32 {
        self.epochs.get(&port).copied().unwrap_or(0)
    }

    pub fn set_port(&mut self, port: PortId, role: StpRole, state: StpSt) {
        self.ports.insert(port, StpPortSt { role, state });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StpPortSt {
    pub role: StpRole,
    pub state: StpSt,
}

/// 三层交换机：VLAN 表 + 按 VLAN 的 MAC 表 + SVI + 路由。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SviState {
    pub ip: Ipv4Addr,
    pub prefix: u8,
    pub admin_up: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SwitchState {
    /// STP 实例（默认启用）
    pub stp: StpInstance,
    /// 已创建的 VLAN（1 恒存在）。
    pub vlans: Vec<VlanId>,
    /// (VLAN, MAC) → 学习端口。
    pub mac_table: HashMap<(VlanId, Mac), PortId>,
    pub svi: HashMap<VlanId, SviState>,
    /// SVI ARP 表按 VLAN 隔离。
    pub arp: HashMap<(VlanId, Ipv4Addr), ArpEntry>,
    pub arping: std::collections::HashSet<(VlanId, Ipv4Addr)>,
    pub pending: Vec<PendingFrame>,
    pub static_routes: Vec<Route>,
    pub pings: Vec<PingSession>,
    pub next_ping_id: u32,
}

impl SwitchState {
    pub fn ensure_vlan(&mut self, vlan: VlanId) {
        if !self.vlans.contains(&vlan) {
            self.vlans.push(vlan);
            self.vlans.sort_unstable();
        }
    }
}

/// 主机 DHCP 客户端状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDhcp {
    /// 已启用手动 renew 的标记：Discover 已发出
    pub discovering: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostState {
    pub gw: Option<Ipv4Addr>,
    /// DHCP 自动获取：启用后忽略静态配置
    pub dhcp_enable: bool,
    pub dhcp: Option<HostDhcp>,
    pub arp: HashMap<Ipv4Addr, ArpEntry>,
    pub arping: std::collections::HashSet<(PortId, Ipv4Addr)>,
    pub pending: Vec<PendingFrame>,
    pub pings: Vec<PingSession>,
    pub next_ping_id: u32,
}

/// EasyIP 转换表项（ICMP：以 echo id 区分会话）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatEntry {
    pub inside_ip: Ipv4Addr,
    pub inside_id: u16,
    pub outside_ip: Ipv4Addr,
    pub outside_id: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeviceState {
    Router(RouterState),
    Switch(SwitchState),
    Host(HostState),
}

/// 交换机/路由器端口的二层口型。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortMode {
    /// 接入口：只收发 pvid 的 untagged 流量。
    #[default]
    Access,
    /// 干道：允许多个 VLAN，pvid 为原生 VLAN（untagged）。
    Trunk,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Port {
    pub idx: PortId,
    pub admin_up: bool,
    pub mac: Mac,
    pub speed_bps: u64,
    pub ip: Option<Ipv4Addr>,
    pub prefix: u8,
    pub link: Option<LinkId>,
    pub mode: PortMode,
    /// Access = 接入 VLAN；Trunk = 原生 VLAN（untagged）。
    pub pvid: VlanId,
    /// Trunk 放行列表；None = 全部放行（思科默认语义）。
    pub trunk_allowed: Option<Vec<VlanId>>,
    /// STP 端口成本覆盖；None = 按速率推导（1Gbps → 4）。
    pub stp_cost: Option<u32>,
    /// 入方向 ACL（路由器）
    pub acl_in: Option<u32>,
    /// 出方向 ACL（路由器）
    pub acl_out: Option<u32>,
}

impl Port {
    pub fn oper_up(&self, link_up: bool) -> bool {
        self.admin_up && self.link.is_some() && link_up
    }

    /// 入方向 VLAN 归类：None = 丢弃（VLAN 不匹配/未放行）。
    pub fn classify_ingress(&self, tag: Option<VlanId>) -> Option<VlanId> {
        match self.mode {
            PortMode::Access => match tag {
                None => Some(self.pvid),
                Some(t) if t == self.pvid => Some(t),
                _ => None,
            },
            PortMode::Trunk => match tag {
                None => Some(self.pvid),
                Some(t) => {
                    let allowed = self.trunk_allowed.as_ref().is_none_or(|v| v.contains(&t));
                    if allowed {
                        Some(t)
                    } else {
                        None
                    }
                }
            },
        }
    }

    /// 出方向 tag 决策：None = 不从该口发出；Some(None) = 剥 tag 发出；Some(Some(t)) = 带 tag 发出。
    pub fn egress_tag(&self, vlan: VlanId) -> Option<Option<VlanId>> {
        match self.mode {
            PortMode::Access => {
                if vlan == self.pvid {
                    Some(None)
                } else {
                    None
                }
            }
            PortMode::Trunk => {
                let in_allowed = self
                    .trunk_allowed
                    .as_ref()
                    .is_none_or(|v| v.contains(&vlan));
                if !(in_allowed || vlan == self.pvid) {
                    return None;
                }
                Some(if vlan == self.pvid { None } else { Some(vlan) })
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: DevId,
    pub name: String,
    pub kind: DeviceKind,
    pub dialect: Dialect,
    pub ports: Vec<Port>,
    pub state: DeviceState,
}

impl Device {
    pub fn new(id: DevId, name: String, kind: DeviceKind, dialect: Dialect) -> Self {
        let ports = (0..kind.port_count())
            .map(|idx| Port {
                idx,
                // 与真实设备一致：交换机端口与主机网卡默认开启，路由器接口需手工 no shutdown
                admin_up: !matches!(kind, DeviceKind::Router),
                mac: port_mac(id, idx),
                speed_bps: PORT_SPEED_BPS,
                ip: None,
                prefix: 24,
                link: None,
                mode: PortMode::Access,
                pvid: DEFAULT_VLAN,
                trunk_allowed: None,
                stp_cost: None,
                acl_in: None,
                acl_out: None,
            })
            .collect();
        let state = match kind {
            DeviceKind::Router => DeviceState::Router(RouterState::default()),
            DeviceKind::Switch => DeviceState::Switch(SwitchState {
                vlans: vec![DEFAULT_VLAN],
                ..Default::default()
            }),
            DeviceKind::Host => DeviceState::Host(HostState::default()),
        };
        Self {
            id,
            name,
            kind,
            dialect,
            ports,
            state,
        }
    }

    pub fn port(&self, idx: PortId) -> Option<&Port> {
        self.ports.get(idx)
    }

    pub fn port_mut(&mut self, idx: PortId) -> Option<&mut Port> {
        self.ports.get_mut(idx)
    }

    /// 是否具备三层能力（本模拟器中交换机即三层交换机）。
    pub fn is_l3(&self) -> bool {
        matches!(self.kind, DeviceKind::Router | DeviceKind::Switch)
    }

    /// 派生连接路由：路由器取每个配置了 IP 的物理口；交换机取每个启用且配置了 IP 的 SVI。
    pub fn connected_routes(&self) -> Vec<Route> {
        match &self.state {
            DeviceState::Router(_) => self
                .ports
                .iter()
                .filter_map(|p| {
                    let ip = p.ip?;
                    let mask = mask_from_prefix(p.prefix);
                    Some(Route {
                        dst: ip & mask,
                        len: p.prefix,
                        nexthop: ip,
                        vlan: None,
                        port: p.idx,
                        src: RouteSrc::Connected,
                    })
                })
                .collect(),
            DeviceState::Switch(sw) => sw
                .svi
                .iter()
                .filter(|(_, s)| s.admin_up)
                .map(|(vlan, s)| {
                    let mask = mask_from_prefix(s.prefix);
                    Route {
                        dst: s.ip & mask,
                        len: s.prefix,
                        nexthop: s.ip,
                        vlan: Some(*vlan),
                        port: 0,
                        src: RouteSrc::Connected,
                    }
                })
                .collect(),
            _ => vec![],
        }
    }
}

/// 由设备号与端口号确定性地生成 MAC（02:00:00:00:dev:port）。
pub fn port_mac(dev: DevId, port: PortId) -> Mac {
    [0x02, 0x00, 0x00, 0x00, dev as u8, port as u8]
}

/// STP 路径成本（802.1t 简表，本模拟器链路均按 1Gbps → 4）。
pub const STP_COST_1G: u32 = 4;

/// SVI 的确定性 MAC（02:00:00:01:00:xx，xx = VLAN 低 8 位）。
pub fn svi_mac(vlan: VlanId) -> Mac {
    [0x02, 0x00, 0x00, 0x01, (vlan >> 8) as u8, vlan as u8]
}
