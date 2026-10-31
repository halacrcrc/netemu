//! 拓扑：设备与链路容器。

use serde::{Deserialize, Serialize};

use crate::device::*;
use sim_packets::{EthernetFrame, Ipv4Addr, Ipv6Addr};

/// 抓包记录（开启抓包的链路上逐帧记录，环形缓冲）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapturedFrame {
    pub seq: u32,
    pub ns: u64,
    /// 方向：0 = a→b，1 = b→a
    pub dir: u8,
    pub frame: EthernetFrame,
}

pub const CAPTURE_BUF_CAP: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub id: LinkId,
    pub up: bool,
    pub a: (DevId, PortId),
    pub b: (DevId, PortId),
    #[serde(default)]
    pub capture: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capture_buf: Vec<CapturedFrame>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub capture_next: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddLinkError {
    DeviceNotFound,
    NoFreePort,
    AlreadyLinked,
    SelfLink,
}

impl core::fmt::Display for AddLinkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            AddLinkError::DeviceNotFound => "设备不存在",
            AddLinkError::NoFreePort => "设备没有空闲端口",
            AddLinkError::AlreadyLinked => "端口已被占用",
            AddLinkError::SelfLink => "不能连接设备自身",
        };
        f.write_str(s)
    }
}

#[derive(Clone, Default)]
pub struct Net {
    pub devices: Vec<Device>,
    pub links: Vec<Link>,
    pub(crate) next_dev: DevId,
    pub(crate) next_link: LinkId,
}

impl Net {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_dev_id(&self) -> DevId {
        self.next_dev
    }

    pub fn device(&self, id: DevId) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    pub fn device_mut(&mut self, id: DevId) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }

    pub fn link(&self, id: LinkId) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id)
    }

    pub fn link_mut(&mut self, id: LinkId) -> Option<&mut Link> {
        self.links.iter_mut().find(|l| l.id == id)
    }

    pub fn link_up(&self, id: LinkId) -> bool {
        self.link(id).map(|l| l.up).unwrap_or(false)
    }

    pub fn set_capture(&mut self, id: LinkId, on: bool) -> bool {
        let Some(l) = self.link_mut(id) else {
            return false;
        };
        l.capture = on;
        if !on {
            l.capture_buf.clear();
        }
        true
    }

    /// 追加抓包记录（环形缓冲，满则丢弃最旧）。
    pub fn record_capture(&mut self, id: LinkId, dir: u8, frame: &EthernetFrame, ns: u64) {
        let Some(l) = self.link_mut(id) else {
            return;
        };
        if !l.capture {
            return;
        }
        let seq = l.capture_next;
        l.capture_next += 1;
        l.capture_buf.push(CapturedFrame {
            seq,
            ns,
            dir,
            frame: *frame,
        });
        if l.capture_buf.len() > CAPTURE_BUF_CAP {
            l.capture_buf.remove(0);
        }
    }

    pub fn add_device(&mut self, name: String, kind: DeviceKind, dialect: Dialect) -> DevId {
        let id = self.next_dev;
        self.next_dev += 1;
        self.devices.push(Device::new(id, name, kind, dialect));
        id
    }

    pub fn remove_device(&mut self, id: DevId) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.id != id);
        let removed = self.devices.len() != before;
        if removed {
            self.links.retain(|l| l.a.0 != id && l.b.0 != id);
        }
        removed
    }

    /// 选取设备上第一个空闲端口。
    fn free_port(&self, dev: DevId) -> Option<PortId> {
        let d = self.device(dev)?;
        (0..d.ports.len()).find(|&idx| d.ports[idx].link.is_none())
    }

    pub fn add_link(&mut self, a: DevId, b: DevId) -> Result<LinkId, AddLinkError> {
        if self.device(a).is_none() || self.device(b).is_none() {
            return Err(AddLinkError::DeviceNotFound);
        }
        if a == b {
            return Err(AddLinkError::SelfLink);
        }
        let pa = self.free_port(a).ok_or(AddLinkError::NoFreePort)?;
        let pb = self.free_port(b).ok_or(AddLinkError::NoFreePort)?;
        let id = self.next_link;
        self.next_link += 1;
        self.links.push(Link {
            id,
            up: true,
            a: (a, pa),
            b: (b, pb),
            capture: false,
            capture_buf: vec![],
            capture_next: 0,
        });
        self.device_mut(a).unwrap().ports[pa].link = Some(id);
        self.device_mut(b).unwrap().ports[pb].link = Some(id);
        Ok(id)
    }

    pub fn remove_link(&mut self, id: LinkId) -> bool {
        let Some(link) = self.link(id).cloned() else {
            return false;
        };
        self.links.retain(|l| l.id != id);
        for (dev, port) in [link.a, link.b] {
            if let Some(d) = self.device_mut(dev) {
                if let Some(p) = d.port_mut(port) {
                    p.link = None;
                }
            }
        }
        true
    }

    /// IPv6 最长前缀匹配：连接路由 + 静态路由（下一跳需落在连接网段内）。
    pub fn lookup_route6(&self, dev: DevId, dst: Ipv6Addr) -> Option<(PortId, Ipv6Addr)> {
        let d = self.device(dev)?;
        let connected: Vec<(PortId, Ipv6Addr, u8)> = d
            .ports
            .iter()
            .filter_map(|p| p.ipv6.map(|ip| (p.idx, ip, p.prefix6)))
            .collect();
        let mut best: Option<(u8, u8, PortId, Ipv6Addr)> = None;
        let mut consider = |len: u8, rank: u8, port: PortId, nh: Ipv6Addr| {
            let better = match best {
                None => true,
                Some((blen, brank, _, _)) => len > blen || (len == blen && rank > brank),
            };
            if better {
                best = Some((len, rank, port, nh));
            }
        };
        for (port, ip, len) in &connected {
            if sim_packets::ipv6_in_prefix(&dst, ip, *len) {
                consider(*len, 3, *port, dst);
            }
        }
        let statics: &[Route6] = match &d.state {
            DeviceState::Router(rs) => &rs.static_routes6,
            _ => &[],
        };
        for r in statics {
            if !sim_packets::ipv6_in_prefix(&dst, &r.dst, r.len) {
                continue;
            }
            if let Some((port, _, len)) = connected
                .iter()
                .find(|(_, ip, len)| sim_packets::ipv6_in_prefix(&r.nexthop, ip, *len))
            {
                let _ = len;
                consider(r.len, 2, *port, r.nexthop);
            }
        }
        best.map(|(_, _, port, nh)| (port, nh))
    }

    /// 最长前缀匹配（连接路由 + 静态路由 + OSPF）。
    /// 同长度优先连接 > 静态 > OSPF；静态/OSPF 的下一跳需落在某连接网段内。
    /// 返回 (出端口, 下一跳, 出 VLAN)。
    pub fn lookup_route(
        &self,
        dev: DevId,
        dst: Ipv4Addr,
    ) -> Option<(PortId, Ipv4Addr, Option<VlanId>)> {
        let d = self.device(dev)?;
        let connected = d.connected_routes();
        // (前缀长度, 来源优先级, 端口, 下一跳, VLAN)
        let mut best: Option<(u8, u8, PortId, Ipv4Addr, Option<VlanId>)> = None;
        let mut consider =
            |len: u8, rank: u8, port: PortId, nexthop: Ipv4Addr, vlan: Option<VlanId>| {
                let better = match best {
                    None => true,
                    Some((blen, brank, _, _, _)) => len > blen || (len == blen && rank > brank),
                };
                if better {
                    best = Some((len, rank, port, nexthop, vlan));
                }
            };
        for r in &connected {
            if (dst & sim_packets::mask_from_prefix(r.len)) == r.dst {
                consider(r.len, 3, r.port, dst, r.vlan);
            }
        }
        let (statics, ospf): (&[Route], &[Route]) = match &d.state {
            DeviceState::Router(rs) => (&rs.static_routes, &rs.ospf_routes),
            DeviceState::Switch(sw) => (&sw.static_routes, &[]),
            _ => (&[], &[]),
        };
        for (routes, rank) in [(statics, 2u8), (ospf, 1u8)] {
            for r in routes {
                if (dst & sim_packets::mask_from_prefix(r.len)) != r.dst {
                    continue;
                }
                if let Some(c) = connected
                    .iter()
                    .find(|c| (r.nexthop & sim_packets::mask_from_prefix(c.len)) == c.dst)
                {
                    consider(r.len, rank, c.port, r.nexthop, c.vlan);
                }
            }
        }
        best.map(|(_, _, port, nh, vlan)| (port, nh, vlan))
    }
}
