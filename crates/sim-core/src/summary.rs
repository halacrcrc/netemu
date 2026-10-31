//! 拓扑与配置的摘要序列化（保存/加载/前后端传输用）。
//!
//! 只保留"配置态"（设备、接口 IP/口型/VLAN、SVI、主机网关、静态路由、链路），
//! 瞬态（ARP 表、MAC 表、ping 会话、事件队列、抓包缓冲）在重建时自然清空。

use serde::{Deserialize, Serialize};
use sim_packets::{ipv4_from_str, ipv4_str, Ipv4Addr};

use crate::device::*;
use crate::net::{Link, Net};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortSummary {
    pub idx: usize,
    pub admin_up: bool,
    /// 点分十进制，未配置为 null
    pub ip: Option<String>,
    pub prefix: u8,
    pub linked: bool,
    pub mode: PortMode,
    pub pvid: VlanId,
    /// None = 全部放行
    pub trunk_allowed: Option<Vec<VlanId>>,
    #[serde(default)]
    pub acl_in: Option<u32>,
    #[serde(default)]
    pub acl_out: Option<u32>,
    /// 所属聚合组 id
    #[serde(default)]
    pub lag: Option<u16>,
    /// IPv6 地址（压缩文本）与前缀
    #[serde(default)]
    pub ipv6: Option<String>,
    #[serde(default)]
    pub prefix6: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DevSummary {
    pub id: DevId,
    pub name: String,
    pub kind: DeviceKind,
    pub dialect: Dialect,
    pub ports: Vec<PortSummary>,
    /// 仅主机
    pub gw: Option<String>,
    /// 路由器与三层交换机
    pub static_routes: Vec<StaticRouteSummary>,
    /// 仅三层交换机：SVI (vlan, ip, prefix, admin)
    #[serde(default)]
    pub svis: Vec<SviSummary>,
    #[serde(default)]
    pub dhcp_pools: Vec<DhcpPoolSummary>,
    #[serde(default)]
    pub nat_inside: Vec<usize>,
    #[serde(default)]
    pub nat_outside: Vec<usize>,
    #[serde(default)]
    pub nat_static: Vec<(String, String)>,
    #[serde(default)]
    pub acls: Vec<AclSummary>,
    #[serde(default)]
    pub dhcp_enable: bool,
    /// 仅交换机：链路聚合组
    #[serde(default)]
    pub lags: Vec<LagSummary>,
    /// 管理面（本地用户 / VTY）
    #[serde(default)]
    pub mgmt: MgmtState,
    /// OSPF network 语句（网段, 通配符）
    #[serde(default)]
    pub ospf_networks: Vec<(String, String)>,
    /// VRRP 组配置
    #[serde(default)]
    pub vrrp: Vec<VrrpSummary>,
    /// 主机 IPv6 网关
    #[serde(default)]
    pub gw6: Option<String>,
    /// IPv6 静态路由
    #[serde(default)]
    pub static_routes6: Vec<StaticRoute6Summary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticRoute6Summary {
    pub dst: String,
    pub len: u8,
    pub nexthop: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VrrpSummary {
    pub vrid: u8,
    pub vip: String,
    pub port: usize,
    pub priority: u8,
    pub preempt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LagSummary {
    pub id: u16,
    pub mode: LagMode,
    pub members: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhcpPoolSummary {
    pub name: String,
    pub network: String,
    pub prefix: u8,
    pub gateway: String,
    pub dns: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclSummary {
    pub number: u32,
    pub rules: Vec<AclRuleSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AclRuleSummary {
    pub permit: bool,
    pub proto: String,
    pub src: String,
    pub src_wildcard: String,
    pub dst: String,
    pub dst_wildcard: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticRouteSummary {
    pub dst: String,
    pub len: u8,
    pub nexthop: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SviSummary {
    pub vlan: VlanId,
    pub ip: String,
    pub prefix: u8,
    pub admin_up: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinkSummary {
    pub id: LinkId,
    pub up: bool,
    pub a: (DevId, usize),
    pub b: (DevId, usize),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NetSummary {
    pub devices: Vec<DevSummary>,
    pub links: Vec<LinkSummary>,
}

impl Net {
    pub fn summary(&self) -> NetSummary {
        NetSummary {
            devices: self
                .devices
                .iter()
                .map(|d| DevSummary {
                    id: d.id,
                    name: d.name.clone(),
                    kind: d.kind,
                    dialect: d.dialect,
                    ports: d
                        .ports
                        .iter()
                        .map(|p| PortSummary {
                            idx: p.idx,
                            admin_up: p.admin_up,
                            ip: p.ip.map(ipv4_str),
                            prefix: p.prefix,
                            linked: p.link.is_some(),
                            mode: p.mode,
                            pvid: p.pvid,
                            trunk_allowed: p.trunk_allowed.clone(),
                            acl_in: p.acl_in,
                            acl_out: p.acl_out,
                            lag: p.lag,
                            ipv6: p.ipv6.map(|a| sim_packets::ipv6_str(&a)),
                            prefix6: p.prefix6,
                        })
                        .collect(),
                    gw: match &d.state {
                        DeviceState::Host(h) => h.gw.map(ipv4_str),
                        _ => None,
                    },
                    static_routes: match &d.state {
                        DeviceState::Router(rs) => static_summaries(&rs.static_routes),
                        DeviceState::Switch(sw) => static_summaries(&sw.static_routes),
                        _ => vec![],
                    },
                    dhcp_pools: match &d.state {
                        DeviceState::Router(rs) => rs
                            .dhcp_pools
                            .iter()
                            .map(|p| DhcpPoolSummary {
                                name: p.name.clone(),
                                network: ipv4_str(p.network),
                                prefix: p.prefix,
                                gateway: ipv4_str(p.gateway),
                                dns: ipv4_str(p.dns),
                            })
                            .collect(),
                        _ => vec![],
                    },
                    nat_inside: match &d.state {
                        DeviceState::Router(rs) => rs.nat_inside.clone(),
                        _ => vec![],
                    },
                    nat_outside: match &d.state {
                        DeviceState::Router(rs) => rs.nat_outside.clone(),
                        _ => vec![],
                    },
                    nat_static: match &d.state {
                        DeviceState::Router(rs) => rs
                            .nat_static
                            .iter()
                            .map(|(x, y)| (ipv4_str(*x), ipv4_str(*y)))
                            .collect(),
                        _ => vec![],
                    },
                    acls: match &d.state {
                        DeviceState::Router(rs) => rs
                            .acls
                            .iter()
                            .map(|a| AclSummary {
                                number: a.number,
                                rules: a
                                    .rules
                                    .iter()
                                    .map(|r| AclRuleSummary {
                                        permit: r.permit,
                                        proto: match r.proto {
                                            AclProto::Any => "any".into(),
                                            AclProto::Icmp => "icmp".into(),
                                        },
                                        src: ipv4_str(r.src),
                                        src_wildcard: ipv4_str(r.src_wildcard),
                                        dst: ipv4_str(r.dst),
                                        dst_wildcard: ipv4_str(r.dst_wildcard),
                                    })
                                    .collect(),
                            })
                            .collect(),
                        _ => vec![],
                    },
                    dhcp_enable: match &d.state {
                        DeviceState::Host(h) => h.dhcp_enable,
                        _ => false,
                    },
                    svis: match &d.state {
                        DeviceState::Switch(sw) => sw
                            .svi
                            .iter()
                            .map(|(vlan, s)| SviSummary {
                                vlan: *vlan,
                                ip: ipv4_str(s.ip),
                                prefix: s.prefix,
                                admin_up: s.admin_up,
                            })
                            .collect(),
                        _ => vec![],
                    },
                    lags: match &d.state {
                        DeviceState::Switch(sw) => {
                            let mut v: Vec<LagSummary> = sw
                                .lags
                                .values()
                                .map(|g| LagSummary {
                                    id: g.id,
                                    mode: g.mode,
                                    members: g.members.clone(),
                                })
                                .collect();
                            v.sort_by_key(|g| g.id);
                            v
                        }
                        _ => vec![],
                    },
                    mgmt: d.mgmt.clone(),
                    ospf_networks: match &d.state {
                        DeviceState::Router(rs) => rs
                            .ospf_networks
                            .iter()
                            .map(|(n, w)| (ipv4_str(*n), ipv4_str(*w)))
                            .collect(),
                        _ => vec![],
                    },
                    vrrp: match &d.state {
                        DeviceState::Router(rs) => rs
                            .vrrp
                            .iter()
                            .map(|g| VrrpSummary {
                                vrid: g.vrid,
                                vip: ipv4_str(g.vip),
                                port: g.port,
                                priority: g.priority,
                                preempt: g.preempt,
                            })
                            .collect(),
                        _ => vec![],
                    },
                    gw6: match &d.state {
                        DeviceState::Host(h) => h.gw6.map(|a| sim_packets::ipv6_str(&a)),
                        _ => None,
                    },
                    static_routes6: match &d.state {
                        DeviceState::Router(rs) => rs
                            .static_routes6
                            .iter()
                            .map(|r| StaticRoute6Summary {
                                dst: sim_packets::ipv6_str(&r.dst),
                                len: r.len,
                                nexthop: sim_packets::ipv6_str(&r.nexthop),
                            })
                            .collect(),
                        _ => vec![],
                    },
                })
                .collect(),
            links: self
                .links
                .iter()
                .map(|l| LinkSummary {
                    id: l.id,
                    up: l.up,
                    a: l.a,
                    b: l.b,
                })
                .collect(),
        }
    }

    /// 用摘要重建拓扑（设备/链路 id 原样保留）。
    pub fn apply_summary(&mut self, s: &NetSummary) -> Result<(), String> {
        self.devices.clear();
        self.links.clear();
        let mut next_dev = 0;
        for ds in &s.devices {
            let id = self.add_device(ds.name.clone(), ds.kind, ds.dialect);
            if id != ds.id {
                return Err(format!("设备 id 不连续：期望 {} 实际 {}", ds.id, id));
            }
            next_dev = next_dev.max(ds.id + 1);
            let d = self.device_mut(id).unwrap();
            d.mgmt = ds.mgmt.clone();
            for ps in &ds.ports {
                if let Some(p) = d.port_mut(ps.idx) {
                    p.admin_up = ps.admin_up;
                    if let Some(txt) = &ps.ip {
                        p.ip = Some(ipv4_from_str(txt).ok_or("非法 IP")?);
                        p.prefix = ps.prefix;
                    }
                    p.mode = ps.mode;
                    p.pvid = ps.pvid;
                    p.trunk_allowed = ps.trunk_allowed.clone();
                    p.acl_in = ps.acl_in;
                    p.acl_out = ps.acl_out;
                    p.lag = ps.lag;
                    p.prefix6 = ps.prefix6;
                    if let Some(txt) = &ps.ipv6 {
                        p.ipv6 = sim_packets::ipv6_from_str(txt);
                    }
                }
            }
            if ds.dhcp_enable {
                if let DeviceState::Host(h) = &mut d.state {
                    h.dhcp_enable = true;
                    h.dhcp = Some(HostDhcp { discovering: false });
                }
            }
            if let (DeviceState::Host(h), Some(gw)) = (&mut d.state, &ds.gw) {
                h.gw = Some(ipv4_from_str(gw).ok_or("非法网关")?);
            }
            if let (DeviceState::Host(h), Some(gw6)) = (&mut d.state, &ds.gw6) {
                h.gw6 = sim_packets::ipv6_from_str(gw6);
            }
            match &mut d.state {
                DeviceState::Router(rs) => {
                    for r in &ds.static_routes6 {
                        if let (Some(dst), Some(nh)) = (
                            sim_packets::ipv6_from_str(&r.dst),
                            sim_packets::ipv6_from_str(&r.nexthop),
                        ) {
                            rs.static_routes6.push(Route6 {
                                dst,
                                len: r.len,
                                nexthop: nh,
                                src: RouteSrc::Static,
                            });
                        }
                    }
                    for r in &ds.static_routes {
                        rs.static_routes.push(static_from(r));
                    }
                    for pool in &ds.dhcp_pools {
                        rs.dhcp_pools.push(DhcpPool {
                            name: pool.name.clone(),
                            network: ipv4_from_str(&pool.network).unwrap_or(0),
                            prefix: pool.prefix,
                            gateway: ipv4_from_str(&pool.gateway).unwrap_or(0),
                            dns: ipv4_from_str(&pool.dns).unwrap_or(0),
                        });
                    }
                    rs.nat_inside = ds.nat_inside.clone();
                    rs.nat_outside = ds.nat_outside.clone();
                    for (n, w) in &ds.ospf_networks {
                        if let (Some(net), Some(wc)) = (ipv4_from_str(n), ipv4_from_str(w)) {
                            rs.ospf_networks.push((net, wc));
                        }
                    }
                    for g in &ds.vrrp {
                        if let Some(vip) = ipv4_from_str(&g.vip) {
                            rs.vrrp.push(VrrpGroup {
                                vrid: g.vrid,
                                vip,
                                port: g.port,
                                priority: g.priority,
                                preempt: g.preempt,
                                state: VrrpState::Init,
                                last_advert_ns: 0,
                            });
                        }
                    }
                    for (x, y) in &ds.nat_static {
                        if let (Some(a), Some(b)) = (ipv4_from_str(x), ipv4_from_str(y)) {
                            rs.nat_static.push((a, b));
                        }
                    }
                    for acl in &ds.acls {
                        rs.acls.push(Acl {
                            number: acl.number,
                            rules: acl
                                .rules
                                .iter()
                                .map(|r| AclRule {
                                    permit: r.permit,
                                    proto: match r.proto.as_str() {
                                        "icmp" => AclProto::Icmp,
                                        _ => AclProto::Any,
                                    },
                                    src: ipv4_from_str(&r.src).unwrap_or(0),
                                    src_wildcard: ipv4_from_str(&r.src_wildcard).unwrap_or(0),
                                    dst: ipv4_from_str(&r.dst).unwrap_or(0),
                                    dst_wildcard: ipv4_from_str(&r.dst_wildcard).unwrap_or(0),
                                    hits: 0,
                                })
                                .collect(),
                        });
                    }
                }
                DeviceState::Switch(sw) => {
                    // 端口引用的 VLAN 需要登记进 VLAN 表
                    for ps in &ds.ports {
                        sw.ensure_vlan(ps.pvid);
                        if let Some(list) = &ps.trunk_allowed {
                            for v in list {
                                sw.ensure_vlan(*v);
                            }
                        }
                    }
                    for r in &ds.static_routes {
                        sw.static_routes.push(static_from(r));
                    }
                    for sv in &ds.svis {
                        let ip = ipv4_from_str(&sv.ip).ok_or("非法 SVI IP")?;
                        sw.ensure_vlan(sv.vlan);
                        sw.svi.insert(
                            sv.vlan,
                            SviState {
                                ip,
                                prefix: sv.prefix,
                                admin_up: sv.admin_up,
                            },
                        );
                    }
                }
                _ => {}
            }
            // 聚合组：模式来自摘要，成员以端口上的 lag 归属为准
            let lag_ports: Vec<(u16, usize)> = d
                .ports
                .iter()
                .filter_map(|p| p.lag.map(|g| (g, p.idx)))
                .collect();
            if let DeviceState::Switch(sw) = &mut d.state {
                for lg in &ds.lags {
                    sw.ensure_lag(lg.id).mode = lg.mode;
                }
                for (gid, port) in lag_ports {
                    let g = sw.ensure_lag(gid);
                    if !g.members.contains(&port) {
                        g.members.push(port);
                    }
                }
                for g in sw.lags.values_mut() {
                    g.members.sort_unstable();
                    g.members.dedup();
                }
            }
        }
        for ls in &s.links {
            if self.device(ls.a.0).is_none() || self.device(ls.b.0).is_none() {
                return Err(format!("链路 {} 引用了不存在的设备", ls.id));
            }
            let id = self.next_link;
            self.next_link += 1;
            self.links.push(Link {
                id,
                up: ls.up,
                a: ls.a,
                b: ls.b,
                capture: false,
                capture_buf: vec![],
                capture_next: 0,
            });
            self.device_mut(ls.a.0).unwrap().ports[ls.a.1].link = Some(id);
            self.device_mut(ls.b.0).unwrap().ports[ls.b.1].link = Some(id);
        }
        self.next_link = self.links.iter().map(|l| l.id + 1).max().unwrap_or(0);
        Ok(())
    }

    // ---------- 配置便捷接口（CLI 层与测试使用） ----------

    pub fn set_port_ip(&mut self, dev: DevId, port: PortId, ip: Option<Ipv4Addr>, prefix: u8) {
        if let Some(d) = self.device_mut(dev) {
            if let Some(p) = d.port_mut(port) {
                p.ip = ip;
                p.prefix = prefix;
            }
        }
    }

    pub fn set_port_ipv6(
        &mut self,
        dev: DevId,
        port: PortId,
        ip: Option<sim_packets::Ipv6Addr>,
        prefix: u8,
    ) {
        if let Some(d) = self.device_mut(dev) {
            if let Some(p) = d.port_mut(port) {
                p.ipv6 = ip;
                p.prefix6 = prefix;
            }
        }
    }

    pub fn add_static_route6(
        &mut self,
        dev: DevId,
        dst: sim_packets::Ipv6Addr,
        len: u8,
        nexthop: sim_packets::Ipv6Addr,
    ) -> bool {
        let Some(d) = self.device_mut(dev) else {
            return false;
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return false;
        };
        if rs
            .static_routes6
            .iter()
            .any(|r| r.dst == dst && r.len == len && r.nexthop == nexthop)
        {
            return true;
        }
        rs.static_routes6.push(Route6 {
            dst,
            len,
            nexthop,
            src: RouteSrc::Static,
        });
        true
    }

    pub fn set_host_v6(
        &mut self,
        dev: DevId,
        ip: Option<sim_packets::Ipv6Addr>,
        prefix: u8,
        gw: Option<sim_packets::Ipv6Addr>,
    ) {
        let Some(d) = self.device_mut(dev) else {
            return;
        };
        if let Some(p) = d.port_mut(0) {
            p.ipv6 = ip;
            p.prefix6 = prefix;
        }
        if let DeviceState::Host(h) = &mut d.state {
            h.gw6 = gw;
        }
    }

    pub fn set_port_admin(&mut self, dev: DevId, port: PortId, up: bool) {
        let link = if let Some(d) = self.device_mut(dev) {
            if let Some(p) = d.port_mut(port) {
                p.admin_up = up;
            }
            if !up {
                // 端口关闭：指向它的 MAC 表项立即失效（真实交换机行为）
                flush_port_macs(d, port);
            }
            d.port(port).and_then(|p| p.link)
        } else {
            None
        };
        // 双端载波检测：本端关闭后，对端也会立刻失效该链路学到的 MAC
        if !up {
            if let Some(lid) = link {
                if let Some(l) = self.link(lid).cloned() {
                    let peer = if l.a == (dev, port) { l.b } else { l.a };
                    if let Some(d) = self.device_mut(peer.0) {
                        flush_port_macs(d, peer.1);
                    }
                }
            }
        }
    }

    /// 设置端口口型。Trunk 时清掉该端口学到的 MAC（VLAN 归属可能变化）。
    pub fn set_port_mode(&mut self, dev: DevId, port: PortId, mode: PortMode) {
        let Some(d) = self.device_mut(dev) else {
            return;
        };
        if let Some(p) = d.port_mut(port) {
            p.mode = mode;
        }
        flush_port_macs(d, port);
    }

    pub fn set_port_access_vlan(&mut self, dev: DevId, port: PortId, vlan: VlanId) {
        let Some(d) = self.device_mut(dev) else {
            return;
        };
        if let Some(p) = d.port_mut(port) {
            p.mode = PortMode::Access;
            p.pvid = vlan;
        }
        flush_port_macs(d, port);
        if let DeviceState::Switch(sw) = &mut d.state {
            sw.ensure_vlan(vlan);
        }
    }

    /// Trunk 放行列表；空列表 = 全部放行。
    pub fn set_port_trunk_allowed(&mut self, dev: DevId, port: PortId, vlans: &[VlanId]) {
        let Some(d) = self.device_mut(dev) else {
            return;
        };
        if let Some(p) = d.port_mut(port) {
            p.mode = PortMode::Trunk;
            p.trunk_allowed = if vlans.is_empty() {
                None
            } else {
                let mut v = vlans.to_vec();
                v.sort_unstable();
                v.dedup();
                Some(v)
            };
        }
        flush_port_macs(d, port);
        if let DeviceState::Switch(sw) = &mut d.state {
            for v in vlans {
                sw.ensure_vlan(*v);
            }
        }
    }

    /// 端口加入聚合组（自动从旧组迁出）。返回 false 表示不是交换机。
    pub fn lag_bind(&mut self, dev: DevId, port: PortId, gid: u16, mode: LagMode) -> bool {
        let Some(d) = self.device_mut(dev) else {
            return false;
        };
        let old = d.ports.get(port).and_then(|p| p.lag);
        let DeviceState::Switch(sw) = &mut d.state else {
            return false;
        };
        if let Some(old) = old.filter(|o| *o != gid) {
            if let Some(g) = sw.lags.get_mut(&old) {
                g.members.retain(|p| *p != port);
                g.selected.retain(|p| *p != port);
                g.partners.remove(&port);
            }
        }
        let g = sw.ensure_lag(gid);
        g.mode = mode;
        if !g.members.contains(&port) {
            g.members.push(port);
            g.members.sort_unstable();
        }
        if let Some(p) = d.port_mut(port) {
            p.lag = Some(gid);
        }
        flush_port_macs(d, port);
        true
    }

    /// 端口退出其聚合组，返回原来的组 id。
    pub fn lag_unbind(&mut self, dev: DevId, port: PortId) -> Option<u16> {
        let d = self.device_mut(dev)?;
        let old = d.ports.get(port).and_then(|p| p.lag)?;
        if let DeviceState::Switch(sw) = &mut d.state {
            if let Some(g) = sw.lags.get_mut(&old) {
                g.members.retain(|p| *p != port);
                g.selected.retain(|p| *p != port);
                g.partners.remove(&port);
            }
        }
        if let Some(p) = d.port_mut(port) {
            p.lag = None;
        }
        flush_port_macs(d, port);
        Some(old)
    }

    /// 设置聚合组工作模式。返回 false 表示不是交换机或组不存在。
    pub fn set_lag_mode(&mut self, dev: DevId, gid: u16, mode: LagMode) -> bool {
        let Some(d) = self.device_mut(dev) else {
            return false;
        };
        let DeviceState::Switch(sw) = &mut d.state else {
            return false;
        };
        match sw.lags.get_mut(&gid) {
            Some(g) => {
                g.mode = mode;
                true
            }
            None => false,
        }
    }

    pub fn ensure_vlan(&mut self, dev: DevId, vlan: VlanId) -> bool {
        let Some(d) = self.device_mut(dev) else {
            return false;
        };
        let DeviceState::Switch(sw) = &mut d.state else {
            return false;
        };
        sw.ensure_vlan(vlan);
        true
    }

    pub fn set_svi(&mut self, dev: DevId, vlan: VlanId, ip: Option<Ipv4Addr>, prefix: u8) -> bool {
        let Some(d) = self.device_mut(dev) else {
            return false;
        };
        let DeviceState::Switch(sw) = &mut d.state else {
            return false;
        };
        sw.ensure_vlan(vlan);
        match (sw.svi.get_mut(&vlan), ip) {
            (Some(s), Some(ip)) => {
                s.ip = ip;
                s.prefix = prefix;
            }
            (Some(s), None) => s.ip = 0,
            (None, Some(ip)) => {
                sw.svi.insert(
                    vlan,
                    SviState {
                        ip,
                        prefix,
                        admin_up: true,
                    },
                );
            }
            (None, None) => {}
        }
        true
    }

    pub fn set_svi_admin(&mut self, dev: DevId, vlan: VlanId, up: bool) -> bool {
        let Some(d) = self.device_mut(dev) else {
            return false;
        };
        let DeviceState::Switch(sw) = &mut d.state else {
            return false;
        };
        if let Some(s) = sw.svi.get_mut(&vlan) {
            s.admin_up = up;
        }
        true
    }

    pub fn add_static_route(
        &mut self,
        dev: DevId,
        dst: Ipv4Addr,
        len: u8,
        nexthop: Ipv4Addr,
    ) -> bool {
        let Some(d) = self.device_mut(dev) else {
            return false;
        };
        let routes = match &mut d.state {
            DeviceState::Router(rs) => &mut rs.static_routes,
            DeviceState::Switch(sw) => &mut sw.static_routes,
            _ => return false,
        };
        if routes
            .iter()
            .any(|r| r.dst == dst && r.len == len && r.nexthop == nexthop)
        {
            return true; // 幂等
        }
        routes.push(Route {
            dst,
            len,
            nexthop,
            vlan: None,
            port: 0,
            src: RouteSrc::Static,
        });
        true
    }

    pub fn set_host(&mut self, dev: DevId, ip: Option<Ipv4Addr>, prefix: u8, gw: Option<Ipv4Addr>) {
        let Some(d) = self.device_mut(dev) else {
            return;
        };
        if let Some(p) = d.port_mut(0) {
            p.ip = ip;
            p.prefix = prefix;
        }
        if let DeviceState::Host(h) = &mut d.state {
            h.gw = gw;
        }
    }

    /// 直连两台同网段主机并配好地址（测试脚手架）。
    pub fn connect_hosts(
        &mut self,
        name_a: &str,
        ip_a: Ipv4Addr,
        name_b: &str,
        ip_b: Ipv4Addr,
        prefix: u8,
    ) -> (DevId, DevId) {
        let a = self.add_device(name_a.into(), DeviceKind::Host, Dialect::Ios);
        let b = self.add_device(name_b.into(), DeviceKind::Host, Dialect::Ios);
        self.set_host(a, Some(ip_a), prefix, None);
        self.set_host(b, Some(ip_b), prefix, None);
        self.add_link(a, b).unwrap();
        (a, b)
    }
}

fn static_summaries(routes: &[Route]) -> Vec<StaticRouteSummary> {
    routes
        .iter()
        .map(|r| StaticRouteSummary {
            dst: ipv4_str(r.dst),
            len: r.len,
            nexthop: ipv4_str(r.nexthop),
        })
        .collect()
}

fn static_from(s: &StaticRouteSummary) -> Route {
    Route {
        dst: ipv4_from_str(&s.dst).unwrap_or(0),
        len: s.len,
        nexthop: ipv4_from_str(&s.nexthop).unwrap_or(0),
        vlan: None,
        port: 0,
        src: RouteSrc::Static,
    }
}

/// 端口口型/VLAN 变化后清掉该端口学到的 MAC（交换机）。
fn flush_port_macs(d: &mut Device, port: PortId) {
    if let DeviceState::Switch(sw) = &mut d.state {
        sw.mac_table.retain(|_, p| *p != port);
    }
}
