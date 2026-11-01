//! 分层报文模型。
//!
//! 内核中以 typed 结构体流转报文（而非字节流），抓包视图渲染时才做"解码"展示。
//! 这样报文体积小、构造/匹配高效，同时保留了逐包逐跳仿真的语义。

use serde::{Deserialize, Serialize};

pub const MAC_LEN: usize = 6;
pub type Mac = [u8; MAC_LEN];
pub type Ipv4Addr = u32;
pub type Ipv6Addr = [u8; 16];

pub const BROADCAST_MAC: Mac = [0xff; MAC_LEN];
pub const ZERO_MAC: Mac = [0x00; MAC_LEN];
/// 802.1D 网桥组组播地址（STP BPDU 目的 MAC）。
pub const STP_MULTICAST_MAC: Mac = [0x01, 0x80, 0xc2, 0x00, 0x00, 0x00];
/// 慢协议组播地址（LACPDU 目的 MAC）。
pub const SLOW_PROTOCOLS_MAC: Mac = [0x01, 0x80, 0xc2, 0x00, 0x00, 0x02];

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
/// BPDU 在本模拟器中的标记值（真实 BPDU 走 802.3+LLC DSAP 0x42，
/// 我们的帧模型用 ethertype 承载类型标记，抓包视图据此解码）。
pub const ETHERTYPE_BPDU: u16 = 0x0042;
/// LACP（慢协议）ethertype。
pub const ETHERTYPE_LACP: u16 = 0x8809;
/// OSPF 在本模拟器中的类型标记（真实 OSPF 为 IP proto 89）。
pub const ETHERTYPE_OSPF: u16 = 0x0089;
/// OSPF 组播目的 MAC（224.0.0.5 → 01:00:5e:00:00:05）。
pub const OSPF_MULTICAST_MAC: Mac = [0x01, 0x00, 0x5e, 0x00, 0x00, 0x05];
/// VRRP 在本模拟器中的类型标记（真实为 IP proto 112）。
pub const ETHERTYPE_VRRP: u16 = 0x0070;
/// VRRP 组播目的 MAC（224.0.0.18 → 01:00:5e:00:00:12）。
pub const VRRP_MULTICAST_MAC: Mac = [0x01, 0x00, 0x5e, 0x00, 0x00, 0x12];
/// IPv6 ethertype。
pub const ETHERTYPE_IPV6: u16 = 0x86DD;
/// BGP 在本模拟器中的类型标记（真实为 TCP 179）。
pub const ETHERTYPE_BGP: u16 = 0x00B4;
/// IS-IS 在本模拟器中的类型标记。
pub const ETHERTYPE_ISIS: u16 = 0x00FE;
/// MPLS 单播 ethertype。
pub const ETHERTYPE_MPLS: u16 = 0x8847;

/// MPLS 标签帧（简化：最多 2 层标签 + 内层 IPv4）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MplsPacket {
    pub labels: [u32; 2],
    pub depth: u8,
    pub ip: Ipv4Packet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsisKind {
    Hello,
    Lsp,
}

/// IS-IS 链路（复用 OSPF 的网段描述）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsisLsa {
    pub system_id: [u8; 6],
    pub links: [Option<OspfLink>; 8],
    pub link_count: u8,
    pub neighbors: [[u8; 6]; 8],
    pub neighbor_count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsisPacket {
    pub kind: IsisKind,
    pub system_id: [u8; 6],
    pub src_ip: Ipv4Addr,
    pub lsa: IsisLsa,
}
/// BGP 报文（简化：Open / Keepalive / Update；广播发送免 ARP）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BgpKind {
    Open,
    Keepalive,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BgpEntry {
    pub net: Ipv4Addr,
    pub len: u8,
    pub path: [u32; 4],
    pub path_len: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BgpPacket {
    pub kind: BgpKind,
    /// 发送方 AS
    pub asn: u32,
    /// 发送接口 IP
    pub src_ip: Ipv4Addr,
    pub entries: [Option<BgpEntry>; 8],
    pub count: u8,
}

pub const DEFAULT_TTL: u8 = 64;

/// ICMP echo 数据部分大小（字节），近似常见 ping 的 64 字节报文。
pub const ICMP_DATA_SIZE: u32 = 64;

pub fn mac_str(mac: &Mac) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// 思科风格：aabb.cc00.0100
pub fn mac_str_cisco(mac: &Mac) -> String {
    format!(
        "{:02x}{:02x}.{:02x}{:02x}.{:02x}{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// 华为/华三风格：02aa-bbcc-0010（三段四位）
pub fn mac_str_huawei(mac: &Mac) -> String {
    format!(
        "{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// 宽容解析 MAC：支持 `:`、`-` 分隔或无分隔。
pub fn mac_from_str(s: &str) -> Option<Mac> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_lowercase();
    if cleaned.len() != 12 {
        return None;
    }
    let mut mac = [0u8; MAC_LEN];
    for (i, b) in mac.iter_mut().enumerate() {
        *b = u8::from_str_radix(&cleaned[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(mac)
}

pub fn ipv4_str(ip: Ipv4Addr) -> String {
    format!(
        "{}.{}.{}.{}",
        (ip >> 24) & 0xff,
        (ip >> 16) & 0xff,
        (ip >> 8) & 0xff,
        ip & 0xff
    )
}

pub fn ipv4_from_str(s: &str) -> Option<Ipv4Addr> {
    let parts: Vec<&str> = s.trim().split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut ip = 0u32;
    for p in parts {
        let oct: u16 = p.parse().ok()?;
        if oct > 255 {
            return None;
        }
        ip = (ip << 8) | oct as u32;
    }
    Some(ip)
}

pub fn mask_from_prefix(len: u8) -> Ipv4Addr {
    if len == 0 {
        0
    } else {
        u32::MAX << (32 - len as u32)
    }
}

/// 仅接受连续掩码。
pub fn prefix_from_mask(m: Ipv4Addr) -> Option<u8> {
    let ones = m.leading_ones() as u8;
    if mask_from_prefix(ones) == m {
        Some(ones)
    } else {
        None
    }
}

pub fn mask_str(m: Ipv4Addr) -> String {
    ipv4_str(m)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArpOp {
    Request,
    Reply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArpPacket {
    pub op: ArpOp,
    pub sender_mac: Mac,
    pub sender_ip: Ipv4Addr,
    pub target_mac: Mac,
    pub target_ip: Ipv4Addr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IcmpPacket {
    pub id: u16,
    pub seq: u16,
    pub is_reply: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ipv4Packet {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub ttl: u8,
    pub icmp: IcmpPacket,
}

pub const TCP_FIN: u8 = 0x01;
pub const TCP_SYN: u8 = 0x02;
pub const TCP_RST: u8 = 0x04;
pub const TCP_ACK: u8 = 0x10;

/// 简化 TCP 段（教学用：握手/挥手/复位，不携带应用数据）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpPacket {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub ttl: u8,
    pub src_port: u16,
    pub dst_port: u16,
    pub flags: u8,
    pub seq: u32,
    pub ack: u32,
}

/// 网桥 ID：优先级 + MAC，越小越优（先比优先级再比 MAC）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeId {
    pub priority: u16,
    pub mac: Mac,
}

impl BridgeId {
    pub fn new(priority: u16, mac: Mac) -> Self {
        Self { priority, mac }
    }
}

impl PartialOrd for BridgeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BridgeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.priority, &self.mac).cmp(&(other.priority, &other.mac))
    }
}

/// DHCP 报文（UDP 67/68 的简化承载：直接作为 L3 载荷，字段取常用选项）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DhcpOp {
    Discover,
    Offer,
    Request,
    Ack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhcpPacket {
    pub op: DhcpOp,
    pub transaction_id: u32,
    /// 客户端 MAC（chaddr）
    pub client_mac: Mac,
    /// 服务器提供的地址（yiaddr）
    pub offered_ip: Ipv4Addr,
    /// 服务器地址（siaddr）
    pub server_ip: Ipv4Addr,
    /// 网关选项（option 3）
    pub gateway: Ipv4Addr,
    /// 掩码长度（option 1，前缀形式）
    pub prefix: u8,
    /// Request 阶段：客户端想续的地址
    pub requested_ip: Ipv4Addr,
}

/// 配置 BPDU（802.1D 简化：不含 TCN/拓扑变更标志；MSTP 实例号 0 = CIST）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BpduPacket {
    /// 公告的根桥
    pub root: BridgeId,
    /// 公告者到根的路径成本
    pub cost: u32,
    /// 发送网桥 ID
    pub sender: BridgeId,
    /// 发送端口（内部端口号）
    pub port: u32,
    /// MSTP 实例号（0 = CIST）
    #[serde(default)]
    pub instance: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum L3 {
    Arp(ArpPacket),
    Ipv4(Ipv4Packet),
    Bpdu(BpduPacket),
    /// DHCP（UDP 67/68 的简化承载）
    Dhcp(DhcpPacket),
    /// LACP 链路聚合控制协议
    Lacp(LacpPdu),
    /// OSPF（简化：Hello + 单条 Router-LSA 的 LSU）
    Ospf(OspfPacket),
    /// VRRP 通告
    Vrrp(VrrpPacket),
    /// IPv6（含 ICMPv6/NDP）
    Ipv6(Ipv6Packet),
    /// BGP（简化）
    Bgp(BgpPacket),
    /// IS-IS（简化）
    Isis(IsisPacket),
    /// MPLS 标签帧
    Mpls(MplsPacket),
    /// 简化 TCP（IPv4 承载）
    Tcp(TcpPacket),
    /// IPsec 站点隧道（教学用 IP-in-IP，不做 ESP 加密）
    Ipsec(IpsecPacket),
    /// CAPWAP（简化：AP 发现/加入、AC 下发 WLAN 配置）
    Capwap(CapwapPacket),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapwapKind {
    Discovery,
    Join,
    ConfigReset,
    Config,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapwapPacket {
    pub kind: CapwapKind,
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub ssid: [u8; 32],
    pub psk: [u8; 32],
    pub vlan: u16,
    pub security: u8,
}

pub fn fixed_str(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = s.as_bytes();
    let n = bytes.len().min(32);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

pub fn fixed_str_value(bytes: &[u8; 32]) -> String {
    let n = bytes.iter().position(|b| *b == 0).unwrap_or(32);
    String::from_utf8_lossy(&bytes[..n]).to_string()
}

/// 简化 IPsec 隧道报文：外层路由到对端，内层是受保护流量。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpsecPacket {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub ttl: u8,
    pub inner: Ipv4Packet,
}

/// VRRP 通告（简化字段）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VrrpPacket {
    pub vrid: u8,
    pub priority: u8,
    pub vip: Ipv4Addr,
    /// 发送接口 IP（决定竞选平局）
    pub master_ip: Ipv4Addr,
}

/// ICMPv6 / NDP 报文（简化）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ipv6Icmp {
    EchoRequest { id: u16, seq: u16 },
    EchoReply { id: u16, seq: u16 },
    NeighborSolicitation { target: Ipv6Addr },
    NeighborAdvertisement { target: Ipv6Addr },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ipv6Packet {
    pub src: Ipv6Addr,
    pub dst: Ipv6Addr,
    pub hop_limit: u8,
    pub icmp: Ipv6Icmp,
}

/// IPv6 文本表示（最长零压缩）。
pub fn ipv6_str(a: &Ipv6Addr) -> String {
    let groups: Vec<u16> = (0..8)
        .map(|i| ((a[i * 2] as u16) << 8) | a[i * 2 + 1] as u16)
        .collect();
    let mut best = (0usize, 0usize);
    let mut cur: Option<usize> = None;
    for (i, g) in groups.iter().enumerate() {
        if *g == 0 {
            cur.get_or_insert(i);
        } else if let Some(s) = cur.take() {
            if i - s > best.1 - best.0 {
                best = (s, i);
            }
        }
    }
    if let Some(s) = cur {
        if 8 - s > best.1 - best.0 {
            best = (s, 8);
        }
    }
    let hex = |gs: &[u16]| {
        gs.iter()
            .map(|g| format!("{:x}", g))
            .collect::<Vec<_>>()
            .join(":")
    };
    if best.1 - best.0 >= 2 {
        format!("{}::{}", hex(&groups[..best.0]), hex(&groups[best.1..]))
    } else {
        hex(&groups)
    }
}

/// 解析 IPv6（支持 `::` 压缩）。
pub fn ipv6_from_str(s: &str) -> Option<Ipv6Addr> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parse_side = |x: &str| -> Option<Vec<u16>> {
        if x.is_empty() {
            return Some(vec![]);
        }
        x.split(':')
            .map(|p| {
                if p.is_empty() || p.len() > 4 {
                    None
                } else {
                    u16::from_str_radix(p, 16).ok()
                }
            })
            .collect()
    };
    let (head, tail, compressed) = match s.split_once("::") {
        Some((h, t)) => (h, t, true),
        None => (s, "", false),
    };
    let h = parse_side(head)?;
    let t = parse_side(tail)?;
    let total = h.len() + t.len();
    if total > 8 {
        return None;
    }
    if !compressed && total != 8 {
        return None;
    }
    if compressed && total == 8 {
        return None;
    }
    let mut groups: Vec<u16> = h;
    groups.extend(std::iter::repeat_n(0u16, 8 - total));
    groups.extend(t);
    let mut out = [0u8; 16];
    for (i, g) in groups.iter().enumerate() {
        out[i * 2] = (g >> 8) as u8;
        out[i * 2 + 1] = *g as u8;
    }
    Some(out)
}

/// 取地址在指定前缀下的网络地址。
pub fn ipv6_net_addr(addr: &Ipv6Addr, len: u8) -> Ipv6Addr {
    let mut out = *addr;
    let full = (len as usize) / 8;
    let rem = (len as usize) % 8;
    if rem != 0 && full < 16 {
        out[full] &= 0xffu8 << (8 - rem);
    }
    let start = full + usize::from(rem != 0);
    for b in out.iter_mut().skip(start) {
        *b = 0;
    }
    out
}

/// IPv6 前缀比较（按位）。
pub fn ipv6_in_prefix(addr: &Ipv6Addr, net: &Ipv6Addr, len: u8) -> bool {
    let full = (len as usize) / 8;
    let rem = (len as usize) % 8;
    if addr[..full] != net[..full] {
        return false;
    }
    if rem == 0 {
        return true;
    }
    let mask = 0xffu8 << (8 - rem);
    (addr[full] & mask) == (net[full] & mask)
}

/// IPv6 地址对应的 solicited-node 组播 MAC（33:33:ff:xx:xx:xx）。
pub fn ipv6_solicited_mac(addr: &Ipv6Addr) -> Mac {
    [0x33, 0x33, 0xff, addr[13], addr[14], addr[15]]
}

/// IPv6 地址对应的 solicited-node 组播地址（ff02::1:ffxx:xxxx）。
pub fn ipv6_solicited_addr(addr: &Ipv6Addr) -> Ipv6Addr {
    let mut m = [0u8; 16];
    m[0] = 0xff;
    m[1] = 0x02;
    m[11] = 0x01;
    m[12] = 0xff;
    m[13] = addr[13];
    m[14] = addr[14];
    m[15] = addr[15];
    m
}

/// OSPF 链路（Router-LSA 中的网段）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OspfLink {
    pub net: Ipv4Addr,
    pub len: u8,
}

/// Router-LSA（固定容量，保持 Copy 语义）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OspfLsa {
    pub router_id: Ipv4Addr,
    pub links: [Option<OspfLink>; 8],
    pub link_count: u8,
    pub neighbors: [Ipv4Addr; 8],
    pub neighbor_count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OspfKind {
    Hello,
    /// 泛洪一条 Router-LSA
    Lsu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OspfPacket {
    pub kind: OspfKind,
    pub area: u32,
    pub router_id: Ipv4Addr,
    /// 发送接口 IP（用于计算下一跳）
    pub src_ip: Ipv4Addr,
    pub lsa: OspfLsa,
}

/// LACPDU 状态位。
pub const LACP_STATE_ACTIVITY: u8 = 0x01;
pub const LACP_STATE_TIMEOUT: u8 = 0x02;
pub const LACP_STATE_AGGREGATION: u8 = 0x04;
pub const LACP_STATE_SYNCHRONIZATION: u8 = 0x08;
pub const LACP_STATE_COLLECTING: u8 = 0x10;
pub const LACP_STATE_DISTRIBUTING: u8 = 0x20;

/// LACPDU（IEEE 802.3ad 的简化字段：只保留聚合协商所需信息）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LacpPdu {
    pub actor_system: Mac,
    pub actor_key: u16,
    pub actor_port: u16,
    pub actor_state: u8,
    pub partner_system: Mac,
    pub partner_key: u16,
    pub partner_port: u16,
    pub partner_state: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthernetFrame {
    pub dst: Mac,
    pub src: Mac,
    /// 802.1Q VLAN tag；None = 无 tag（Untagged）。
    pub vlan: Option<u16>,
    pub ethertype: u16,
    pub payload: L3,
}

impl EthernetFrame {
    /// 线上字节数近似：帧头 14 + 802.1Q(4) + 载荷 + FCS 4。
    pub fn wire_size(&self) -> u32 {
        let payload = match &self.payload {
            L3::Arp(_) => 28,
            L3::Ipv4(_) => 20 + ICMP_DATA_SIZE,
            L3::Bpdu(_) => 35,           // 802.3+LLC 头 + 配置 BPDU 载荷近似
            L3::Dhcp(_) => 20 + 8 + 272, // IP + UDP + DHCP 载荷近似
            L3::Lacp(_) => 110,          // LACPDU 定长载荷近似
            L3::Ospf(_) => 120,          // OSPF 字段近似
            L3::Vrrp(_) => 40,           // VRRP 通告近似
            L3::Ipv6(_) => 40 + 8 + 16,  // IPv6 头 + ICMPv6 近似
            L3::Bgp(_) => 19 + 32,       // BGP 头 + 条目近似
            L3::Isis(_) => 27 + 32,      // IS-IS 头 + 条目近似
            L3::Mpls(m) => 14 + 4 * m.depth as u32 + 20 + ICMP_DATA_SIZE + 4,
            L3::Tcp(_) => 20 + 20 + 8,
            L3::Ipsec(_) => 20 + 20 + ICMP_DATA_SIZE + 20,
            L3::Capwap(_) => 20 + 8 + 64 + 2,
        };
        14 + payload + 4 + if self.vlan.is_some() { 4 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_roundtrip() {
        let ip = ipv4_from_str("192.168.1.10").unwrap();
        assert_eq!(ipv4_str(ip), "192.168.1.10");
        assert!(ipv4_from_str("256.1.1.1").is_none());
        assert!(ipv4_from_str("1.2.3").is_none());
    }

    #[test]
    fn mask_helpers() {
        assert_eq!(mask_from_prefix(24), 0xffffff00);
        assert_eq!(prefix_from_mask(0xffffff00), Some(24));
        assert_eq!(prefix_from_mask(0xffffff80 | 1), None);
        assert_eq!(mask_str(mask_from_prefix(24)), "255.255.255.0");
    }

    #[test]
    fn mac_roundtrip() {
        let mac: Mac = [0x02, 0x00, 0x00, 0x00, 0x01, 0x02];
        assert_eq!(mac_from_str(&mac_str(&mac)), Some(mac));
        assert_eq!(mac_from_str("02-00-00-00-01-02"), Some(mac));
        assert!(mac_from_str("02:00:00").is_none());
    }

    #[test]
    fn ipv6_roundtrip() {
        for s in [
            "2001:db8::1",
            "::1",
            "::",
            "fe80::1",
            "2001:db8:1:2:3:4:5:6",
        ] {
            let a = ipv6_from_str(s).unwrap_or_else(|| panic!("parse {}", s));
            let t = ipv6_str(&a);
            assert_eq!(ipv6_from_str(&t), Some(a), "{} -> {}", s, t);
        }
        assert!(ipv6_from_str("2001:db8::1::2").is_none());
        assert!(ipv6_from_str("2001:db8:1:2:3:4:5").is_none());
        assert_eq!(
            ipv6_str(&ipv6_from_str("2001:db8::").unwrap()),
            "2001:db8::"
        );
        assert!(ipv6_in_prefix(
            &ipv6_from_str("2001:db8:1::5").unwrap(),
            &ipv6_from_str("2001:db8:1::").unwrap(),
            64
        ));
        assert!(!ipv6_in_prefix(
            &ipv6_from_str("2001:db8:2::5").unwrap(),
            &ipv6_from_str("2001:db8:1::").unwrap(),
            64
        ));
        assert_eq!(
            ipv6_str(&ipv6_net_addr(
                &ipv6_from_str("2001:db8:1:2::9").unwrap(),
                64
            )),
            "2001:db8:1:2::"
        );
    }
}
