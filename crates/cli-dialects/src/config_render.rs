// 三方言的 running/current 配置渲染：从 DevSummary（完整配置快照）实时生成（ADR D12 单一事实源）。
// 覆盖核心段：名称/VLAN/接口（IP·模式·tag·shutdown）/SVI/静态路由/OSPF·BGP/DHCP/ACL/NAT/
// mgmt（本地用户·VTY）/VRRP/VXLAN·EVPN/LTM/M-LAG/堆叠/IS-IS/SRv6/防火墙策略。

use crate::ifname;
use crate::Dialect;
use sim_core::device::{DeviceKind, PortMode};
use sim_core::summary::DevSummary;

fn is_l3(kind: DeviceKind) -> bool {
    matches!(
        kind,
        DeviceKind::Router
            | DeviceKind::Firewall
            | DeviceKind::LoadBalancer
            | DeviceKind::WlanController
    )
}

fn vlans_of(d: &DevSummary) -> Vec<u16> {
    let mut set: Vec<u16> = Vec::new();
    for p in &d.ports {
        match p.mode {
            sim_core::device::PortMode::Access => {
                if !set.contains(&p.pvid) {
                    set.push(p.pvid);
                }
            }
            sim_core::device::PortMode::Trunk => {
                if let Some(list) = &p.trunk_allowed {
                    for v in list {
                        if !set.contains(v) {
                            set.push(*v);
                        }
                    }
                }
            }
        }
    }
    for s in &d.svis {
        if !set.contains(&s.vlan) {
            set.push(s.vlan);
        }
    }
    set.sort_unstable();
    set
}

fn port_lines_ios(d: &DevSummary, out: &mut Vec<String>) {
    let l3 = is_l3(d.kind);
    for p in &d.ports {
        out.push(format!(
            "interface {}",
            ifname::canonical(Dialect::Ios, d.kind, p.idx)
        ));
        if let Some(ip) = &p.ip {
            let mask = sim_packets::mask_from_prefix(p.prefix);
            out.push(format!(" ip address {} {}", ip, sim_packets::ipv4_str(mask)));
        }
        if let Some(v6) = &p.ipv6 {
            out.push(format!(" ipv6 address {}/{}", v6, p.prefix6));
        }
        if !l3 {
            match p.mode {
                PortMode::Access => {
                    out.push(" switchport mode access".into());
                    out.push(format!(" switchport access vlan {}", p.pvid));
                }
                PortMode::Trunk => {
                    out.push(" switchport mode trunk".into());
                    if let Some(list) = &p.trunk_allowed {
                        let vs: Vec<String> = list.iter().map(|v| v.to_string()).collect();
                        out.push(format!(" switchport trunk allowed vlan {}", vs.join(",")));
                    }
                }
            }
        }
        if let Some(acl) = p.acl_in {
            out.push(format!(" ip access-group {} in", acl));
        }
        if let Some(z) = &p.zone {
            out.push(format!(" zone-member security {}", z));
        }
        if let Some(n) = &p.nameif {
            out.push(format!(" nameif {}", n));
        }
        if let Some(l) = p.security_level {
            out.push(format!(" security-level {}", l));
        }
        if let Some(g) = p.lag {
            out.push(format!(" channel-group {} mode active", g));
        }
        if !p.admin_up {
            out.push(" shutdown".into());
        }
        out.push(" exit".into());
    }
}

fn port_lines_vrp(d: &DevSummary, dialect: Dialect, out: &mut Vec<String>) {
    let l3 = is_l3(d.kind);
    for p in &d.ports {
        out.push(format!(
            "interface {}",
            ifname::canonical(dialect, d.kind, p.idx)
        ));
        if let Some(ip) = &p.ip {
            out.push(format!(" ip address {} {}", ip, p.prefix));
        }
        if let Some(v6) = &p.ipv6 {
            out.push(format!(" ipv6 address {}/{}", v6, p.prefix6));
        }
        if !l3 {
            match p.mode {
                PortMode::Access => {
                    out.push(" port link-type access".into());
                    out.push(format!(" port default vlan {}", p.pvid));
                }
                PortMode::Trunk => {
                    out.push(" port link-type trunk".into());
                    if let Some(list) = &p.trunk_allowed {
                        let vs: Vec<String> = list.iter().map(|v| v.to_string()).collect();
                        out.push(format!(" port trunk allow-pass vlan {}", vs.join(" ")));
                    }
                }
            }
        }
        if let Some(z) = &p.zone {
            out.push(format!(" firewall zone {}", z));
        }
        if !p.admin_up {
            out.push(" shutdown".into());
        }
        out.push(" quit".into());
    }
}

/// 生成 running/current 配置文本（按方言风格）。
pub fn render(d: &DevSummary, dialect: Dialect) -> String {
    let mut out: Vec<String> = Vec::new();
    match dialect {
        Dialect::Ios => out.push(format!("hostname {}", d.name)),
        _ => out.push(format!("sysname {}", d.name)),
    }
    // VLAN（从端口与 SVI 收集）
    let vlans = vlans_of(d);
    if !vlans.is_empty() {
        match dialect {
            Dialect::Ios => {
                for v in &vlans {
                    out.push(format!("vlan {}", v));
                }
            }
            _ => {
                let vs: Vec<String> = vlans.iter().map(|v| v.to_string()).collect();
                out.push(format!("vlan batch {}", vs.join(" ")));
            }
        }
    }
    // 接口
    match dialect {
        Dialect::Ios => port_lines_ios(d, &mut out),
        _ => port_lines_vrp(d, dialect, &mut out),
    }
    // SVI
    for s in &d.svis {
        match dialect {
            Dialect::Ios => {
                out.push(format!("interface {}", ifname::svi_canonical(Dialect::Ios, s.vlan)));
                out.push(format!(" ip address {} {}", s.ip, s.prefix));
                if !s.admin_up {
                    out.push(" shutdown".into());
                }
                out.push(" exit".into());
            }
            _ => {
                out.push(format!("interface {}", ifname::svi_canonical(dialect, s.vlan)));
                out.push(format!(" ip address {} {}", s.ip, s.prefix));
                if !s.admin_up {
                    out.push(" shutdown".into());
                }
                out.push(" quit".into());
            }
        }
    }
    // 静态路由
    for r in &d.static_routes {
        match dialect {
            Dialect::Ios => out.push(format!("ip route {} {} {}", r.dst, r.len, r.nexthop)),
            _ => out.push(format!("ip route-static {} {} {}", r.dst, r.len, r.nexthop)),
        }
    }
    for r in &d.static_routes6 {
        match dialect {
            Dialect::Ios => out.push(format!("ipv6 route {}/{} {}", r.dst, r.len, r.nexthop)),
            _ => out.push(format!("ipv6 route-static {} {} {}", r.dst, r.len, r.nexthop)),
        }
    }
    // OSPF（v4）
    if !d.ospf_networks.is_empty() {
        match dialect {
            Dialect::Ios => {
                out.push("router ospf 1".into());
                for (net, wc) in &d.ospf_networks {
                    out.push(format!(" network {} {}", net, wc));
                }
            }
            _ => {
                out.push("ospf 1".into());
                out.push(" area 0".into());
                for (net, wc) in &d.ospf_networks {
                    out.push(format!(" network {} {}", net, wc));
                }
            }
        }
    }
    // OSPFv3
    if d.ospfv3_enabled {
        match dialect {
            Dialect::Ios => out.push("ipv6 router ospf".into()),
            _ => out.push("ospfv3 1".into()),
        }
    }
    // BGP
    if let Some(asn) = d.bgp_asn {
        match dialect {
            Dialect::Ios => out.push(format!("router bgp {}", asn)),
            _ => out.push(format!("bgp {}", asn)),
        }
        for (ip, ra) in &d.bgp_neighbors {
            out.push(format!(" neighbor {} as-number {}", ip, ra));
        }
        for (net, len) in &d.bgp_networks {
            let mask = sim_packets::ipv4_str(sim_packets::mask_from_prefix(*len));
            out.push(format!(" network {} {}", net, mask));
        }
    }
    // DHCP pool
    for pool in &d.dhcp_pools {
        let mask = sim_packets::ipv4_str(sim_packets::mask_from_prefix(pool.prefix));
        out.push(format!("ip dhcp pool {}", pool.name));
        out.push(format!(" network {} {}", pool.network, mask));
        match dialect {
            Dialect::Ios => out.push(format!(" default-router {}", pool.gateway)),
            _ => out.push(format!(" gateway {}", pool.gateway)),
        }
    }
    // ACL（占位行：规则细节以 show access-lists / display acl 呈现）
    for acl in &d.acls {
        match dialect {
            Dialect::Ios => out.push(format!("access-list {}", acl.number)),
            _ => out.push(format!("acl {}", acl.number)),
        }
    }
    // NAT
    for idx in &d.nat_inside {
        match dialect {
            Dialect::Ios => {
                out.push(format!("interface GigabitEthernet0/0/{} nat inside", idx + 1))
            }
            _ => out.push(format!(
                "interface GigabitEthernet0/0/{} nat inbound",
                idx + 1
            )),
        }
    }
    for idx in &d.nat_outside {
        match dialect {
            Dialect::Ios => {
                out.push(format!("interface GigabitEthernet0/0/{} nat outside", idx + 1))
            }
            _ => out.push(format!(
                "interface GigabitEthernet0/0/{} nat outbound",
                idx + 1
            )),
        }
    }
    // mgmt：本地用户与 VTY
    if !d.mgmt.users.is_empty() || d.mgmt.auth_local {
        match dialect {
            Dialect::Ios => {
                for u in &d.mgmt.users {
                    out.push(format!("username {} privilege 15 secret ***", u.name));
                }
                out.push("line vty 0 4".into());
                if d.mgmt.auth_local {
                    out.push(" login local".into());
                }
                match d.mgmt.transport {
                    sim_core::device::VtyTransport::Ssh => out.push(" transport input ssh".into()),
                    sim_core::device::VtyTransport::Telnet => {
                        out.push(" transport input telnet".into())
                    }
                    sim_core::device::VtyTransport::All => out.push(" transport input all".into()),
                    sim_core::device::VtyTransport::None => {
                        out.push(" transport input none".into())
                    }
                }
            }
            _ => {
                for u in &d.mgmt.users {
                    out.push(format!("local-user {}", u.name));
                }
                if d.mgmt.auth_local {
                    out.push("authentication-mode aaa".into());
                }
                match d.mgmt.transport {
                    sim_core::device::VtyTransport::Ssh => out.push("protocol inbound ssh".into()),
                    sim_core::device::VtyTransport::Telnet => {
                        out.push("protocol inbound telnet".into())
                    }
                    sim_core::device::VtyTransport::All => out.push("protocol inbound all".into()),
                    sim_core::device::VtyTransport::None => {
                        out.push("protocol inbound none".into())
                    }
                }
            }
        }
    }
    // VRRP
    for g in &d.vrrp {
        match dialect {
            Dialect::Ios => out.push(format!(
                "interface GigabitEthernet0/0/{}: vrrp {} ip {}",
                g.port + 1,
                g.vrid,
                g.vip
            )),
            _ => out.push(format!(
                "interface GigabitEthernet0/0/{}: vrrp vrid {} virtual-ip {}",
                g.port + 1,
                g.vrid,
                g.vip
            )),
        }
    }
    // VXLAN / EVPN
    if let Some(vx) = &d.vxlan {
        if let Some(local) = &vx.local {
            out.push(format!("vxlan local {}", local));
        }
        for (vni, vlan) in &vx.mappings {
            out.push(format!("vxlan vni {} vlan {}", vni, vlan));
        }
        for peer in &vx.peers {
            out.push(format!("vxlan peer {}", peer));
        }
    }
    if let Some(ev) = &d.evpn {
        if ev.enabled {
            out.push("evpn enable".into());
        }
        for peer in &ev.peers {
            out.push(format!("evpn peer {}", peer));
        }
    }
    // LTM
    if let Some(ltm) = &d.ltm {
        for pool in &ltm.pools {
            for (ip, port) in &pool.members {
                out.push(format!("ltm pool {} member {} {}", pool.name, ip, port));
            }
        }
        for v in &ltm.virtuals {
            out.push(format!(
                "ltm virtual {} vip {} {} pool {}",
                v.name, v.vip, v.port, v.pool
            ));
        }
    }
    // M-LAG / 堆叠
    if let Some(m) = &d.mlag {
        out.push(format!("mlag domain {}", m.domain));
        for p in &m.peer_links {
            out.push(format!("interface GigabitEthernet0/0/{} mlag peer-link", p + 1));
        }
    }
    if let Some(st) = &d.stack {
        out.push(format!("stack domain {}", st.domain));
        out.push(format!("stack member {}", st.member));
        out.push(format!("stack priority {}", st.priority));
    }
    // IS-IS / SRv6
    if let Some(net) = &d.isis_net {
        out.push(format!("network {}", net));
    }
    if let Some((loc, len)) = &d.srv6_locator {
        out.push(format!("srv6 locator {}/{}", loc, len));
    }
    for sid in &d.srv6_sids {
        out.push(format!("srv6 sid {}", sid));
    }
    for r in &d.srv6_routes {
        out.push(format!("srv6 route {}/{} sid {}", r.dst, r.len, r.sid));
    }
    // 防火墙策略
    for sp in &d.security_policies {
        out.push(format!(
            "security-policy rule {} {} -> {} permit={}",
            sp.name, sp.src_zone, sp.dst_zone, sp.permit
        ));
    }
    out.join("\n")
}
