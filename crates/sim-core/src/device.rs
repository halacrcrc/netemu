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
    /// OSPF 动态学习
    Ospf,
    /// BGP 动态学习
    Bgp,
    /// IS-IS 动态学习
    Isis,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortSecViolation {
    #[default]
    Restrict,
    Shutdown,
}

/// 端口安全配置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSecurity {
    pub max: u16,
    pub violation: PortSecViolation,
    pub sticky: bool,
}

/// DHCP Snooping 绑定表项。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhcpSnoopingBinding {
    pub mac: Mac,
    pub ip: Ipv4Addr,
    pub vlan: VlanId,
    pub port: PortId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArpEntry {
    pub mac: Mac,
    pub port: PortId,
}

/// IPv6 邻居缓存（NDP）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NdpEntry {
    pub mac: Mac,
    pub port: PortId,
}

/// IPv6 静态路由。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route6 {
    pub dst: Ipv6Addr,
    pub len: u8,
    pub nexthop: Ipv6Addr,
    pub src: RouteSrc,
}

/// 等 NDP 解析后发送的 IPv6 报文。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending6 {
    pub port: PortId,
    pub dst_ip: Ipv6Addr,
    pub src_ip: Ipv6Addr,
    pub src_mac: Mac,
    pub pkt: Ipv6Packet,
}

/// IPv6 ping 会话（简化：5 个探测）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ping6Session {
    pub id: u16,
    pub target: Ipv6Addr,
    pub sent: u32,
    pub received: u32,
    pub done: bool,
    pub reported: bool,
}

impl Ping6Session {
    pub const TOTAL: u32 = 5;
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
    /// 通配符位为 1 表示忽略；源/目的都为 0 且通配符为 0 视为未限制该字段。
    pub fn matches(&self, src: Ipv4Addr, dst: Ipv4Addr, is_icmp: bool) -> bool {
        let proto_ok = match self.proto {
            AclProto::Any => true,
            AclProto::Icmp => is_icmp,
        };
        let src_ok = (src & !self.src_wildcard) == (self.src & !self.src_wildcard);
        let dst_ok = if self.dst == 0 && self.dst_wildcard == 0 {
            true
        } else {
            (dst & !self.dst_wildcard) == (self.dst & !self.dst_wildcard)
        };
        proto_ok && src_ok && dst_ok
    }

    /// 路由前缀匹配：只看源地址段（协议/目的不参与路由过滤）。
    pub fn matches_route(&self, net: Ipv4Addr) -> bool {
        (net & !self.src_wildcard) == (self.src & !self.src_wildcard)
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
    /// IPv6 静态路由
    #[serde(default)]
    pub static_routes6: Vec<Route6>,
    /// IPv6 邻居缓存 / 解析中 / 挂起报文 / ping6
    #[serde(default)]
    pub ndp: HashMap<(PortId, Ipv6Addr), NdpEntry>,
    #[serde(default)]
    pub ndping: std::collections::HashSet<(PortId, Ipv6Addr)>,
    #[serde(default)]
    pub pending6: Vec<Pending6>,
    #[serde(default)]
    pub pings6: Vec<Ping6Session>,
    #[serde(default)]
    pub next_ping6_id: u32,
    /// ARP 表按 (接口, VLAN) 隔离。
    pub arp: HashMap<(PortId, Option<VlanId>, Ipv4Addr), ArpEntry>,
    /// 已发出 ARP 请求、等待解析的 (端口, VLAN, 目标IP)，防止重复请求。
    pub arping: std::collections::HashSet<(PortId, Option<VlanId>, Ipv4Addr)>,
    pub pending: Vec<PendingFrame>,
    pub pings: Vec<PingSession>,
    pub next_ping_id: u32,
    /// OSPF network 语句（网段 + 通配符），区域固定为 0（简化）
    #[serde(default)]
    pub ospf_networks: Vec<(Ipv4Addr, Ipv4Addr)>,
    /// OSPF 计算出的动态路由（瞬态）
    #[serde(default)]
    pub ospf_routes: Vec<Route>,
    /// BGP 配置与动态路由
    #[serde(default)]
    pub bgp: Option<BgpConfig>,
    #[serde(default)]
    pub bgp_routes: Vec<Route>,
    /// IS-IS 配置与动态路由
    #[serde(default)]
    pub isis: Option<IsisConfig>,
    #[serde(default)]
    pub isis_routes: Vec<Route>,
    /// VRF 表
    #[serde(default)]
    pub vrfs: Vec<Vrf>,
    /// MPLS 静态 LSP
    #[serde(default)]
    pub mpls_lsps: Vec<StaticLsp>,
    /// VRRP 组（主备网关）
    #[serde(default)]
    pub vrrp: Vec<VrrpGroup>,
    /// 路由策略（BGP 邻居 import/export 使用）
    #[serde(default)]
    pub route_policies: Vec<RoutePolicy>,
}

/// BGP 配置（AS、邻居、宣告网段、邻居策略）。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BgpConfig {
    pub asn: u32,
    /// (邻居 IP, 对端 AS)
    pub neighbors: Vec<(Ipv4Addr, u32)>,
    /// 本地宣告的网段 (网络, 前缀长度)
    pub networks: Vec<(Ipv4Addr, u8)>,
    /// 邻居路由策略绑定
    #[serde(default)]
    pub peer_policies: Vec<BgpPeerPolicy>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BgpPeerPolicy {
    pub neighbor: Ipv4Addr,
    /// true = import（入方向），false = export（出方向）
    pub import: bool,
    pub name: String,
}

/// 路由策略：按 seq 升序匹配，首个命中节点决定 permit/deny 与属性。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePolicy {
    pub name: String,
    pub nodes: Vec<RoutePolicyNode>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePolicyNode {
    pub seq: u16,
    pub permit: bool,
    /// 匹配基础 ACL（ACL 判定 permit 视为命中）
    pub match_acl: Option<u32>,
    /// 命中后设置 BGP Local Preference
    pub set_local_pref: Option<u32>,
}

/// VRF（虚拟路由转发表，简化：只有连接路由与静态路由）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vrf {
    pub id: u16,
    pub name: String,
    pub static_routes: Vec<Route>,
}

/// 静态 LSP 表项：ingress（net 匹配 + out_label）/ transit（in→out）/ egress（in→pop）。
/// `vrf` 非空时表示 VPN 路由：ingress 按该 VRF 匹配 FEC 并压入 `vpn_label`（双标签），
/// egress 把该标签投递到对应 VRF。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticLsp {
    pub name: String,
    pub net: Option<(Ipv4Addr, u8)>,
    pub in_label: Option<u32>,
    pub out_label: Option<u32>,
    pub nexthop: Option<Ipv4Addr>,
    #[serde(default)]
    pub vrf: Option<String>,
    #[serde(default)]
    pub vpn_label: Option<u32>,
}

/// IS-IS 配置（简化：单区域 Level-1，NET 仅用于取 System ID）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IsisConfig {
    pub net: String,
    pub system_id: [u8; 6],
}

/// VRRP 组成员状态。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VrrpState {
    #[default]
    Init,
    Master,
    Backup,
}

/// VRRP 组：虚拟网关（简化，单 VRRP 实例）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VrrpGroup {
    pub vrid: u8,
    pub vip: Ipv4Addr,
    pub port: PortId,
    pub priority: u8,
    pub preempt: bool,
    pub state: VrrpState,
    pub last_advert_ns: u64,
}

/// VRRP 虚拟 MAC：00:00:5e:00:01:{vrid}
pub fn vrrp_mac(vrid: u8) -> Mac {
    [0x00, 0x00, 0x5e, 0x00, 0x01, vrid]
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

/// STP 工作模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StpMode {
    /// 802.1D 经典 STP（30s 收敛）
    #[default]
    Dot1d,
    /// RSTP 快速生成树（本模拟器简化：端口直达转发，故障即时切换）
    Rstp,
    /// MSTP 多实例（简化：无域协商，按 instance 独立计算）
    Mstp,
}

/// 单个 STP 实例的计算现场（CIST 与 MSTI 共用同一套字段）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StpSnapshot {
    pub root: sim_packets::BridgeId,
    pub root_cost: u32,
    pub root_port: Option<PortId>,
    pub ports: HashMap<PortId, StpPortSt>,
    pub port_bpdus: HashMap<PortId, sim_packets::BpduPacket>,
    pub last_bpdu_ns: HashMap<PortId, u64>,
    pub epochs: HashMap<PortId, u32>,
}

impl Default for StpSnapshot {
    fn default() -> Self {
        Self {
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
        }
    }
}

/// MST 实例（MSTI）：优先级 + 映射的 VLAN + 计算现场。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MstInstance {
    pub priority: u16,
    pub vlans: Vec<VlanId>,
    pub data: StpSnapshot,
}

/// 单台交换机的 STP 实例（CIST + MSTI；无 MSTP 域协商）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StpInstance {
    pub enabled: bool,
    #[serde(default)]
    pub mode: StpMode,
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
    /// MSTI 实例（id → 实例），CIST 为 id 0（上面的字段）
    #[serde(default)]
    pub mst: HashMap<u16, MstInstance>,
    /// VLAN → MSTI 实例映射（缺省为 CIST）
    #[serde(default)]
    pub vlan_instance: HashMap<VlanId, u16>,
    /// hello 定时器是否已在队列中
    pub hello_pending: bool,
}

impl Default for StpInstance {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: StpMode::Dot1d,
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
            mst: HashMap::new(),
            vlan_instance: HashMap::new(),
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

    /// 某 VLAN 使用的 STP 实例（缺省 CIST=0）。
    pub fn instance_for_vlan(&self, vlan: VlanId) -> u16 {
        self.vlan_instance.get(&vlan).copied().unwrap_or(0)
    }

    /// 实例配置（不存在则创建）。
    pub fn ensure_mst(&mut self, id: u16) -> &mut MstInstance {
        self.mst.entry(id).or_insert_with(|| MstInstance {
            priority: 32768,
            vlans: vec![],
            data: StpSnapshot::default(),
        })
    }

    /// 实例当前端口状态（0 = CIST）。
    pub fn state_in(&self, instance: u16, port: PortId) -> StpSt {
        if instance == 0 {
            self.state_of(port)
        } else {
            self.mst
                .get(&instance)
                .and_then(|m| m.data.ports.get(&port))
                .map(|p| p.state)
                .unwrap_or(StpSt::Blocking)
        }
    }

    pub fn role_in(&self, instance: u16, port: PortId) -> StpRole {
        if instance == 0 {
            self.role_of(port)
        } else {
            self.mst
                .get(&instance)
                .and_then(|m| m.data.ports.get(&port))
                .map(|p| p.role)
                .unwrap_or(StpRole::Disabled)
        }
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
    /// DHCP Snooping 全局开关
    #[serde(default)]
    pub dhcp_snooping: bool,
    /// DHCP Snooping 绑定表（瞬态）
    #[serde(default)]
    pub dhcp_bindings: Vec<DhcpSnoopingBinding>,
    /// 端口安全已允许的 MAC
    #[serde(default)]
    pub secure_macs: HashMap<PortId, Vec<Mac>>,
    /// 端口安全违规次数
    #[serde(default)]
    pub port_sec_violations: HashMap<PortId, u32>,
    /// 链路聚合组（id → 组）
    #[serde(default)]
    pub lags: HashMap<u16, LagGroup>,
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

    pub fn ensure_lag(&mut self, id: u16) -> &mut LagGroup {
        self.lags.entry(id).or_insert_with(|| LagGroup::new(id))
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
    /// IPv6 默认网关
    #[serde(default)]
    pub gw6: Option<Ipv6Addr>,
    /// DHCP 自动获取：启用后忽略静态配置
    pub dhcp_enable: bool,
    pub dhcp: Option<HostDhcp>,
    pub arp: HashMap<Ipv4Addr, ArpEntry>,
    pub arping: std::collections::HashSet<(PortId, Ipv4Addr)>,
    pub pending: Vec<PendingFrame>,
    pub pings: Vec<PingSession>,
    pub next_ping_id: u32,
    /// IPv6 邻居/解析/挂起/ping6
    #[serde(default)]
    pub ndp: HashMap<Ipv6Addr, NdpEntry>,
    #[serde(default)]
    pub ndping: std::collections::HashSet<Ipv6Addr>,
    #[serde(default)]
    pub pending6: Vec<Pending6>,
    #[serde(default)]
    pub pings6: Vec<Ping6Session>,
    #[serde(default)]
    pub next_ping6_id: u32,
}

/// EasyIP 转换表项（ICMP：以 echo id 区分会话）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NatEntry {
    pub inside_ip: Ipv4Addr,
    pub inside_id: u16,
    pub outside_ip: Ipv4Addr,
    pub outside_id: u16,
}

#[allow(clippy::large_enum_variant)] // 设备状态按值存放，便于到处直接模式匹配
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
    /// 所属链路聚合组 id；None = 未加入聚合组。
    #[serde(default)]
    pub lag: Option<u16>,
    /// IPv6 地址与前缀
    #[serde(default)]
    pub ipv6: Option<Ipv6Addr>,
    #[serde(default)]
    pub prefix6: u8,
    /// DHCP Snooping 信任端口
    #[serde(default)]
    pub dhcp_trust: bool,
    /// 端口安全
    #[serde(default)]
    pub port_security: Option<PortSecurity>,
    /// 广播风暴抑制（每秒广播帧上限）
    #[serde(default)]
    pub broadcast_limit_pps: Option<u32>,
    /// 接口入口限速（kbps）
    #[serde(default)]
    pub rate_limit_kbps: Option<u32>,
    /// 是否参与 IS-IS
    #[serde(default)]
    pub isis_enable: bool,
    /// 所属 VRF id（None = 全局表）
    #[serde(default)]
    pub vrf: Option<u16>,
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

/// 链路聚合组工作模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LagMode {
    /// LACP 主动（周期发 LACPDU）
    #[default]
    LacpActive,
    /// LACP 被动（收到 LACPDU 才回应）
    LacpPassive,
    /// 静态聚合（不跑 LACP，端口 up 即转发）
    Static,
}

/// 成员端口看到的一侧对端 LACP 信息。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LacpPartner {
    pub system: Mac,
    pub key: u16,
    pub port: u16,
    pub last_ns: u64,
}

/// 链路聚合组（Eth-Trunk / Port-channel / Bridge-Aggregation）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LagGroup {
    pub id: u16,
    pub mode: LagMode,
    /// 成员端口（升序，最多 8 个）
    pub members: Vec<PortId>,
    /// 当前参与转发的成员（LACP 协商选中 / 静态聚合端口 up）
    pub selected: Vec<PortId>,
    /// 每成员看到的对端 LACP 信息
    pub partners: HashMap<PortId, LacpPartner>,
    /// LACP 周期定时器是否已排队
    pub timer_pending: bool,
}

impl Default for LagGroup {
    fn default() -> Self {
        Self {
            id: 0,
            mode: LagMode::LacpActive,
            members: vec![],
            selected: vec![],
            partners: HashMap::new(),
            timer_pending: false,
        }
    }
}

impl LagGroup {
    pub fn new(id: u16) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }
}

/// 远程管理协议。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MgmtProto {
    Telnet,
    Ssh,
}

/// VTY 允许的接入协议。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VtyTransport {
    #[default]
    All,
    Ssh,
    Telnet,
    None,
}

impl VtyTransport {
    pub fn allows(self, p: MgmtProto) -> bool {
        matches!(
            (self, p),
            (VtyTransport::All, _)
                | (VtyTransport::Ssh, MgmtProto::Ssh)
                | (VtyTransport::Telnet, MgmtProto::Telnet)
        )
    }
}

/// 本地管理用户（口令明文存储——仅本地教学数据，不上云）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MgmtUser {
    pub name: String,
    pub secret: String,
    pub privilege: u8,
    pub telnet: bool,
    pub ssh: bool,
}

/// 设备管理面（厂商中立）：本地用户、VTY 认证与接入协议。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MgmtState {
    pub users: Vec<MgmtUser>,
    pub enable_secret: Option<String>,
    /// 本地认证：IOS `login local` / VRP `authentication-mode aaa` / Comware `scheme`
    pub auth_local: bool,
    pub transport: VtyTransport,
}

impl Default for MgmtState {
    fn default() -> Self {
        Self {
            users: vec![],
            enable_secret: None,
            auth_local: false,
            transport: VtyTransport::All,
        }
    }
}

impl MgmtState {
    pub fn ensure_user(&mut self, name: &str) -> &mut MgmtUser {
        if let Some(i) = self.users.iter().position(|u| u.name == name) {
            return &mut self.users[i];
        }
        self.users.push(MgmtUser {
            name: name.to_string(),
            secret: String::new(),
            privilege: 1,
            telnet: true,
            ssh: true,
        });
        self.users.last_mut().unwrap()
    }

    pub fn find_user(&self, name: &str) -> Option<&MgmtUser> {
        self.users.iter().find(|u| u.name == name)
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
    /// 管理面（远程登录认证）
    #[serde(default)]
    pub mgmt: MgmtState,
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
                lag: None,
                ipv6: None,
                prefix6: 64,
                dhcp_trust: false,
                port_security: None,
                broadcast_limit_pps: None,
                rate_limit_kbps: None,
                isis_enable: false,
                vrf: None,
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
            mgmt: MgmtState::default(),
        }
    }

    pub fn port(&self, idx: PortId) -> Option<&Port> {
        self.ports.get(idx)
    }

    pub fn port_mut(&mut self, idx: PortId) -> Option<&mut Port> {
        self.ports.get_mut(idx)
    }

    /// 端口所属聚合组 id。
    pub fn port_lag(&self, port: PortId) -> Option<u16> {
        self.ports.get(port).and_then(|p| p.lag)
    }

    /// 是否具备三层能力（本模拟器中交换机即三层交换机）。
    pub fn is_l3(&self) -> bool {
        matches!(self.kind, DeviceKind::Router | DeviceKind::Switch)
    }

    /// 派生连接路由：路由器取每个配置了 IP 的物理口；交换机取每个启用且配置了 IP 的 SVI。
    pub fn connected_routes(&self) -> Vec<Route> {
        self.connected_routes_vrf(None)
    }

    /// 某 VRF 的连接路由（None = 全局表；VRF 口不计入全局）。
    pub fn connected_routes_vrf(&self, vrf: Option<u16>) -> Vec<Route> {
        match &self.state {
            DeviceState::Router(_) => self
                .ports
                .iter()
                .filter(|p| p.vrf == vrf)
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
            DeviceState::Switch(sw) => {
                if vrf.is_some() {
                    return vec![];
                }
                sw.svi
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
                    .collect()
            }
            _ => vec![],
        }
    }

    /// 按名称查找 VRF id。
    pub fn vrf_id(&self, name: &str) -> Option<u16> {
        match &self.state {
            DeviceState::Router(rs) => rs.vrfs.iter().find(|v| v.name == name).map(|v| v.id),
            _ => None,
        }
    }

    /// 端口所属 VRF 名称。
    pub fn vrf_name_of_port(&self, port: PortId) -> Option<String> {
        let id = self.port(port)?.vrf?;
        match &self.state {
            DeviceState::Router(rs) => rs.vrfs.iter().find(|v| v.id == id).map(|v| v.name.clone()),
            _ => None,
        }
    }
}

/// 由设备号与端口号确定性地生成 MAC（02:00:00:00:dev:port）。
pub fn port_mac(dev: DevId, port: PortId) -> Mac {
    [0x02, 0x00, 0x00, 0x00, dev as u8, port as u8]
}

/// STP 路径成本（802.1t 简表，本模拟器链路均按 1Gbps → 4）。
pub const STP_COST_1G: u32 = 4;

/// 解析 IS-IS NET（如 49.0001.0000.0000.0001.00），返回 System ID（中间 6 字节）。
pub fn parse_isis_net(s: &str) -> Option<[u8; 6]> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() < 16 {
        return None;
    }
    // NET = 区域(可变) + System ID(12 hex) + NSEL(2 hex)
    let sys = &hex[hex.len() - 14..hex.len() - 2];
    let mut out = [0u8; 6];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&sys[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// SVI 的确定性 MAC（02:00:00:01:00:xx，xx = VLAN 低 8 位）。
pub fn svi_mac(vlan: VlanId) -> Mac {
    [0x02, 0x00, 0x00, 0x01, (vlan >> 8) as u8, vlan as u8]
}
