//! 分层报文模型。
//!
//! 内核中以 typed 结构体流转报文（而非字节流），抓包视图渲染时才做"解码"展示。
//! 这样报文体积小、构造/匹配高效，同时保留了逐包逐跳仿真的语义。

use serde::{Deserialize, Serialize};

pub const MAC_LEN: usize = 6;
pub type Mac = [u8; MAC_LEN];
pub type Ipv4Addr = u32;

pub const BROADCAST_MAC: Mac = [0xff; MAC_LEN];
pub const ZERO_MAC: Mac = [0x00; MAC_LEN];
/// 802.1D 网桥组组播地址（STP BPDU 目的 MAC）。
pub const STP_MULTICAST_MAC: Mac = [0x01, 0x80, 0xc2, 0x00, 0x00, 0x00];

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
/// BPDU 在本模拟器中的标记值（真实 BPDU 走 802.3+LLC DSAP 0x42，
/// 我们的帧模型用 ethertype 承载类型标记，抓包视图据此解码）。
pub const ETHERTYPE_BPDU: u16 = 0x0042;

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

/// 配置 BPDU（802.1D 简化：不含 TCN/拓扑变更标志）。
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum L3 {
    Arp(ArpPacket),
    Ipv4(Ipv4Packet),
    Bpdu(BpduPacket),
    /// DHCP（UDP 67/68 的简化承载）
    Dhcp(DhcpPacket),
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
}
