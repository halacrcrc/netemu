//! `show` / `display` 输出渲染与 ping 结果渲染。
//!
//! 原则：所有输出都从模拟状态实时渲染，绝不硬编码。

use sim_packets::Mac;

use sim_core::device::{
    AclProto, DevId, DeviceKind, DeviceState, Dialect, PingProbeState, PingSession, PortId,
    PortMode, Route, RouteSrc, StpRole, StpSt, SviState, VlanId, DEFAULT_VLAN, STP_COST_1G,
};
use sim_core::engine::Engine;
use sim_core::{port_mac, Device};
use sim_packets::{ipv4_str, mask_from_prefix};

use crate::ifname;

fn port_rows(
    eng: &Engine,
    dev: DevId,
) -> Vec<(&Device, usize, bool, Option<sim_packets::Ipv4Addr>, u8)> {
    let Some(d) = eng.net.device(dev) else {
        return vec![];
    };
    d.ports
        .iter()
        .map(|p| {
            let oper = p.oper_up(p.link.map(|l| eng.net.link_up(l)).unwrap_or(false));
            (d, p.idx, oper, p.ip, p.prefix)
        })
        .collect()
}

/// IP 列文本：IOS 显示纯地址，VRP/Comware 带 /掩码长度。
fn ip_cell(dialect: Dialect, ip: Option<sim_packets::Ipv4Addr>, prefix: u8) -> String {
    match (dialect, ip) {
        (_, None) => "unassigned".into(),
        (Dialect::Ios, Some(ip)) => ipv4_str(ip),
        (_, Some(ip)) => format!("{}/{}", ipv4_str(ip), prefix),
    }
}

/// `show ip interface brief` / `display ip interface brief`。
pub fn int_brief(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = port_rows(eng, dev);
    // 交换机：SVI 行追加在物理口之后
    let svi_rows: Vec<(String, Option<sim_packets::Ipv4Addr>, u8, bool)> = {
        let Some(d) = eng.net.device(dev) else {
            return String::new();
        };
        match &d.state {
            DeviceState::Switch(sw) => {
                let mut v: Vec<(u16, &SviState)> = sw.svi.iter().map(|(k, s)| (*k, s)).collect();
                v.sort_by_key(|(vlan, _)| *vlan);
                v.into_iter()
                    .map(|(vlan, s)| {
                        (
                            ifname::svi_short(dialect, vlan),
                            Some(s.ip),
                            s.prefix,
                            s.admin_up,
                        )
                    })
                    .collect()
            }
            _ => vec![],
        }
    };
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<26}{:<15}{:<4}{:<7}{:<22}{}\n",
                "Interface", "IP-Address", "OK?", "Method", "Status", "Protocol"
            ));
            for (d, idx, oper, ip, prefix) in rows {
                let p = &d.ports[idx];
                let status = if !p.admin_up {
                    "administratively down"
                } else if oper {
                    "up"
                } else {
                    "down"
                };
                let proto = if oper && p.ip.is_some() { "up" } else { "down" };
                out.push_str(&format!(
                    "{:<26}{:<15}{:<4}{:<7}{:<22}{}\n",
                    ifname::display(dialect, d.kind, idx),
                    ip_cell(dialect, ip, prefix),
                    "yes",
                    if p.ip.is_some() { "manual" } else { "unset" },
                    status,
                    proto
                ));
            }
            for (name, ip, prefix, admin) in &svi_rows {
                let status = if *admin {
                    "up"
                } else {
                    "administratively down"
                };
                let proto = if *admin && ip.is_some() { "up" } else { "down" };
                out.push_str(&format!(
                    "{:<26}{:<15}{:<4}{:<7}{:<22}{}\n",
                    name,
                    ip_cell(dialect, *ip, *prefix),
                    "yes",
                    if ip.is_some() { "manual" } else { "unset" },
                    status,
                    proto
                ));
            }
        }
        Dialect::Vrp => {
            out.push_str(&format!(
                "{:<33}{:<20}{:<10}{}\n",
                "Interface", "IP Address/Mask", "Physical", "Protocol"
            ));
            for (d, idx, oper, ip, prefix) in rows {
                let p = &d.ports[idx];
                let physical = if oper {
                    "up"
                } else if p.admin_up {
                    "down"
                } else {
                    "*down"
                };
                let proto = if oper && p.ip.is_some() { "up" } else { "down" };
                out.push_str(&format!(
                    "{:<33}{:<20}{:<10}{}\n",
                    ifname::display(dialect, d.kind, idx),
                    ip_cell(dialect, ip, prefix),
                    physical,
                    proto
                ));
            }
            for (name, ip, prefix, admin) in &svi_rows {
                let physical = if *admin { "up" } else { "*down" };
                let proto = if *admin && ip.is_some() { "up" } else { "down" };
                out.push_str(&format!(
                    "{:<33}{:<20}{:<10}{}\n",
                    name,
                    ip_cell(dialect, *ip, *prefix),
                    physical,
                    proto
                ));
            }
        }
        Dialect::Comware => {
            out.push_str(&format!(
                "{:<24} {:<8} {:<8} {:<16}{}\n",
                "Interface", "Physical", "Protocol", "IP Address", "Description"
            ));
            for (d, idx, oper, ip, prefix) in rows {
                let p = &d.ports[idx];
                let physical = if oper {
                    "up"
                } else if p.admin_up {
                    "down"
                } else {
                    "*down"
                };
                let proto = if oper && p.ip.is_some() { "up" } else { "down" };
                out.push_str(&format!(
                    "{:<24} {:<8} {:<8} {:<16}{}\n",
                    ifname::display(dialect, d.kind, idx),
                    physical,
                    proto,
                    ip_cell(dialect, ip, prefix),
                    "--"
                ));
            }
            for (name, ip, prefix, admin) in &svi_rows {
                let physical = if *admin { "up" } else { "*down" };
                let proto = if *admin && ip.is_some() { "up" } else { "down" };
                out.push_str(&format!(
                    "{:<24} {:<8} {:<8} {:<16}{}\n",
                    name,
                    physical,
                    proto,
                    ip_cell(dialect, *ip, *prefix),
                    "--"
                ));
            }
        }
    }
    out.trim_end().to_string()
}

struct RouteRow {
    dst: String,
    len: u8,
    src: RouteSrc,
    nexthop: String,
    iface: String,
}

fn route_rows(eng: &Engine, dev: DevId, dialect: Dialect) -> Vec<RouteRow> {
    let Some(d) = eng.net.device(dev) else {
        return vec![];
    };
    let svi_name = |vlan: VlanId| ifname::svi_short(dialect, vlan);
    let mut rows: Vec<RouteRow> = d
        .connected_routes()
        .iter()
        .map(|r| RouteRow {
            dst: ipv4_str(r.dst),
            len: r.len,
            src: RouteSrc::Connected,
            nexthop: ipv4_str(r.nexthop),
            iface: match r.vlan {
                Some(v) => svi_name(v),
                None => ifname::display(dialect, d.kind, r.port),
            },
        })
        .collect();
    let statics: &[Route] = match &d.state {
        DeviceState::Router(rs) => &rs.static_routes,
        DeviceState::Switch(sw) => &sw.static_routes,
        _ => &[],
    };
    {
        let rs_static = statics;
        for r in rs_static {
            // 出接口 = 下一跳所在连接网段
            let port = d.connected_routes().iter().find_map(|c| {
                if (r.nexthop & mask_from_prefix(c.len)) == c.dst {
                    Some(c.port)
                } else {
                    None
                }
            });
            rows.push(RouteRow {
                dst: ipv4_str(r.dst),
                len: r.len,
                src: RouteSrc::Static,
                nexthop: ipv4_str(r.nexthop),
                iface: port
                    .map(|p| ifname::display(dialect, d.kind, p))
                    .unwrap_or_else(|| "?".into()),
            });
        }
    }
    rows.sort_by(|a, b| {
        b.len
            .cmp(&a.len)
            .then(a.dst.cmp(&b.dst))
            .then(match (a.src, b.src) {
                (RouteSrc::Connected, RouteSrc::Static) => std::cmp::Ordering::Less,
                (RouteSrc::Static, RouteSrc::Connected) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
    });
    rows
}

/// `show ip route` / `display ip routing-table`。
pub fn route_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = route_rows(eng, dev, dialect);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("Codes: C - connected, S - static\n\n");
            out.push_str("Gateway of last resort is not set\n\n");
            for r in &rows {
                let code = match r.src {
                    RouteSrc::Connected => 'C',
                    RouteSrc::Static => 'S',
                };
                if r.src == RouteSrc::Connected {
                    out.push_str(&format!(
                        "{}    {}/{} is directly connected, {}\n",
                        code, r.dst, r.len, r.iface
                    ));
                } else {
                    out.push_str(&format!(
                        "{}    {}/{} [1/0] via {}\n",
                        code, r.dst, r.len, r.nexthop
                    ));
                }
            }
        }
        Dialect::Vrp => {
            out.push_str("Routing Table: Public\n");
            out.push_str(&format!(
                "Destinations : {:<8} Routes : {}\n\n",
                rows.len(),
                rows.len()
            ));
            out.push_str(&format!(
                "{:<19} {:<7} {:<4} {:<6} {:<6} {:<15} {}\n",
                "Destination/Mask", "Proto", "Pre", "Cost", "Flags", "NextHop", "Interface"
            ));
            out.push('\n');
            for r in &rows {
                let (proto, pre, flags) = match r.src {
                    RouteSrc::Connected => ("Direct", 0, "D"),
                    RouteSrc::Static => ("Static", 60, "RD"),
                };
                out.push_str(&format!(
                    "{:<19} {:<7} {:<4} {:<6} {:<6} {:<15} {}\n",
                    format!("{}/{}", r.dst, r.len),
                    proto,
                    pre,
                    0,
                    flags,
                    r.nexthop,
                    r.iface
                ));
            }
        }
        Dialect::Comware => {
            out.push_str(&format!(
                "Destinations : {:<7} Routes : {}\n\n",
                rows.len(),
                rows.len()
            ));
            out.push_str(&format!(
                "{:<18} {:<7} {:<4} {:<5} {:<15} {}\n",
                "Destination/Mask", "Proto", "Pre", "Cost", "NextHop", "Interface"
            ));
            for r in &rows {
                let proto = match r.src {
                    RouteSrc::Connected => "Direct",
                    RouteSrc::Static => "Static",
                };
                out.push_str(&format!(
                    "{:<18} {:<7} {:<4} {:<5} {:<15} {}\n",
                    format!("{}/{}", r.dst, r.len),
                    proto,
                    match r.src {
                        RouteSrc::Connected => 0,
                        RouteSrc::Static => 60,
                    },
                    0,
                    r.nexthop,
                    r.iface
                ));
            }
        }
    }
    out.trim_end().to_string()
}

fn ms(ns: u64) -> u64 {
    (ns as f64 / 1e6).round() as u64
}

fn ms3(ns: u64) -> String {
    format!("{:.3}", ns as f64 / 1e6)
}

fn triplet(s: &PingSession) -> Option<(u64, u64, u64)> {
    s.rtt_ms_triplet()
        .map(|(a, b, c)| (a.round() as u64, b.round() as u64, c.round() as u64))
}

/// ping 结果渲染（含横幅），方言差异集中于此。
pub fn render_ping(dialect: Dialect, s: &PingSession) -> String {
    let target = ipv4_str(s.target);
    let sent = s.total;
    let recv = s.received();
    match dialect {
        Dialect::Ios => {
            let mut out = format!(
                "Type escape sequence to abort.\nSending {}, 100-byte ICMP Echos to {}, timeout is 2 seconds:\n",
                sent, target
            );
            let mut marks = String::new();
            for p in &s.probes {
                marks.push(match p.state {
                    PingProbeState::Replied { .. } => '!',
                    _ => '.',
                });
            }
            out.push_str(&marks);
            out.push('\n');
            let rate = (recv as f64 / sent as f64 * 100.0) as u64;
            out.push_str(&format!(
                "Success rate is {} percent ({}/{})",
                rate, recv, sent
            ));
            if let Some((min, avg, max)) = triplet(s) {
                out.push_str(&format!(
                    ", round-trip min/avg/max = {}/{}/{} ms",
                    min, avg, max
                ));
            }
            out
        }
        Dialect::Vrp => {
            let mut out = format!("  PING {}: 56  data bytes, press CTRL_C to break\n", target);
            for (i, p) in s.probes.iter().enumerate() {
                match p.state {
                    PingProbeState::Replied { ttl, rtt_ns } => out.push_str(&format!(
                        "    Reply from {}: bytes=56 Sequence={} ttl={} time={} ms\n",
                        target,
                        i + 1,
                        ttl,
                        ms(rtt_ns)
                    )),
                    _ => out.push_str("    Request time out.\n"),
                }
            }
            out.push_str(&format!("  --- {} ping statistics ---\n", target));
            out.push_str(&format!("    {} packet(s) transmitted\n", sent));
            out.push_str(&format!("    {} packet(s) received\n", recv));
            out.push_str(&format!(
                "    {:.2}% packet loss\n",
                (sent - recv) as f64 / sent as f64 * 100.0
            ));
            if let Some((min, avg, max)) = triplet(s) {
                out.push_str(&format!(
                    "    round-trip min/avg/max = {}/{}/{} ms",
                    min, avg, max
                ));
            }
            out
        }
        Dialect::Comware => {
            let mut out = format!("Ping {} ({}): 56 data bytes\n", target, target);
            for (i, p) in s.probes.iter().enumerate() {
                match p.state {
                    PingProbeState::Replied { ttl, rtt_ns } => out.push_str(&format!(
                        "56 bytes from {}: icmp_seq={} ttl={} time={} ms\n",
                        target,
                        i,
                        ttl,
                        ms3(rtt_ns)
                    )),
                    _ => out.push_str(&format!("Request time out (icmp_seq={})\n", i)),
                }
            }
            out.push_str(&format!("--- {} ping statistics ---\n", target));
            out.push_str(&format!(
                "{} packet(s) transmitted, {} packet(s) received, {:.1}% packet loss\n",
                sent,
                recv,
                (sent - recv) as f64 / sent as f64 * 100.0
            ));
            if let Some(t) = s.rtt_ms_triplet() {
                out.push_str(&format!(
                    "round-trip min/avg/max = {}/{}/{} ms",
                    ms3((t.0 * 1e6) as u64),
                    ms3((t.1 * 1e6) as u64),
                    ms3((t.2 * 1e6) as u64)
                ));
            }
            out
        }
    }
}

/// 主机（PC）的 ping 输出：linux 风格，与路由器方言无关。
pub fn render_ping_generic(s: &PingSession) -> String {
    let target = ipv4_str(s.target);
    let recv = s.received();
    let mut out = format!("PING {} ({}) 56(84) bytes of data.\n", target, target);
    for (i, p) in s.probes.iter().enumerate() {
        if let PingProbeState::Replied { ttl, rtt_ns } = p.state {
            out.push_str(&format!(
                "64 bytes from {}: icmp_seq={} ttl={} time={} ms\n",
                target,
                i + 1,
                ttl,
                ms3(rtt_ns)
            ));
        }
    }
    out.push_str(&format!("\n--- {} ping statistics ---\n", target));
    out.push_str(&format!(
        "{} packets transmitted, {} received, {:.0}% packet loss\n",
        s.total,
        recv,
        (s.total - recv) as f64 / s.total as f64 * 100.0
    ));
    out
}

/// 设备是否具有 L3 能力（show ip route / ping 的前提）。
pub fn is_l3_device(d: &Device) -> bool {
    matches!(d.kind, DeviceKind::Router | DeviceKind::Switch)
}

/// STP 表格：角色/状态按方言渲染。
fn stp_role_str(dialect: Dialect, r: StpRole) -> &'static str {
    match (dialect, r) {
        (_, StpRole::Designated) => {
            if dialect == Dialect::Ios {
                "Desg"
            } else {
                "DESI"
            }
        }
        (_, StpRole::Root) => {
            if dialect == Dialect::Ios {
                "Root"
            } else {
                "ROOT"
            }
        }
        (_, StpRole::Alternate) => {
            if dialect == Dialect::Ios {
                "Altn"
            } else {
                "ALTE"
            }
        }
        (_, StpRole::Disabled) => {
            if dialect == Dialect::Ios {
                "Dis"
            } else {
                "DISI"
            }
        }
    }
}

fn stp_state_str(dialect: Dialect, st: StpSt) -> &'static str {
    match (dialect, st) {
        (_, StpSt::Forwarding) => {
            if dialect == Dialect::Ios {
                "FWD"
            } else {
                "FORWARDING"
            }
        }
        (_, StpSt::Learning) => {
            if dialect == Dialect::Ios {
                "LRN"
            } else {
                "LEARNING"
            }
        }
        (_, StpSt::Listening) => {
            if dialect == Dialect::Ios {
                "LIS"
            } else {
                "LISTENING"
            }
        }
        (_, StpSt::Blocking) => {
            if dialect == Dialect::Ios {
                "BLK"
            } else {
                "DISCARDING"
            }
        }
    }
}

/// `show spanning-tree` / `display stp brief`。
pub fn stp_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Switch(sw) = &d.state else {
        return String::new();
    };
    let stp = &sw.stp;
    let is_root = stp.root.mac == port_mac(dev, 0) && stp.root_cost == 0;
    let my_bridge = stp.bridge_id(dev);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("VLAN0001\n");
            out.push_str("  Spanning tree enabled protocol ieee\n");
            out.push_str("  Root ID    Priority    ");
            out.push_str(&format!("{}\n", stp.root.priority));
            out.push_str(&format!(
                "             Address     {}\n",
                sim_packets::mac_str_cisco(&stp.root.mac)
            ));
            if !is_root {
                out.push_str(&format!("             Cost        {}\n", stp.root_cost));
                if let Some(rp) = stp.root_port {
                    out.push_str(&format!(
                        "             Port        {} ({})\n",
                        rp + 1,
                        ifname::display(dialect, d.kind, rp)
                    ));
                }
            } else {
                out.push_str("             This bridge is the root\n");
            }
            out.push_str(
                "             Hello Time   2 sec  Max Age 20 sec  Forward Delay 15 sec\n\n",
            );
            out.push_str("  Bridge ID  Priority    ");
            out.push_str(&format!("{}\n", my_bridge.priority));
            out.push_str(&format!(
                "             Address     {}\n",
                sim_packets::mac_str_cisco(&my_bridge.mac)
            ));
            out.push_str(
                "             Hello Time   2 sec  Max Age 20 sec  Forward Delay 15 sec\n\n",
            );
            out.push_str(&format!(
                "{:<19} {:<4} {:<3} {:<9} {:<8} {}\n",
                "Interface", "Role", "Sts", "Cost", "Prio.Nbr", "Type"
            ));
            out.push_str(&format!(
                "{:<19} {:<4} {:<3} {:<9} {:<8} {}\n",
                "-------------------", "----", "---", "---------", "--------", "----"
            ));
            for p in &d.ports {
                let role = stp.role_of(p.idx);
                let state = stp.state_of(p.idx);
                if role == StpRole::Disabled && p.link.is_none() {
                    continue; // IOS 不显示未接线的端口
                }
                out.push_str(&format!(
                    "{:<19} {:<4} {:<3} {:<9} {:<8} {}\n",
                    ifname::display(dialect, d.kind, p.idx),
                    stp_role_str(dialect, role),
                    stp_state_str(dialect, state),
                    p.stp_cost.unwrap_or(STP_COST_1G),
                    format!("128.{}", p.idx + 1),
                    "P2p"
                ));
            }
        }
        _ => {
            // VRP / Comware：display stp brief
            out.push_str(&format!(
                " {:<6} {:<28} {:<5} {:<12} {}\n",
                "MSTID", "Port", "Role", "STP State", "Protection"
            ));
            out.push_str(&format!(
                " {:<6} {:<28} {:<5} {:<12} {}\n",
                "-----", "----------------------------", "-----", "------------", "----------"
            ));
            for p in &d.ports {
                let role = stp.role_of(p.idx);
                let state = stp.state_of(p.idx);
                if role == StpRole::Disabled && p.link.is_none() {
                    continue;
                }
                out.push_str(&format!(
                    " {:<6} {:<28} {:<5} {:<12} {}\n",
                    0,
                    ifname::display(dialect, d.kind, p.idx),
                    stp_role_str(dialect, role),
                    stp_state_str(dialect, state),
                    "NONE"
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show access-lists` / `display acl all`。
pub fn acl_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(rs) = &d.state else {
        return String::new();
    };
    if rs.acls.is_empty() {
        return match dialect {
            Dialect::Ios => String::new(),
            _ => "Info: No ACL exists.".into(),
        };
    }
    let mut out = String::new();
    for acl in &rs.acls {
        let kind = if (100..=199).contains(&acl.number) {
            if dialect == Dialect::Ios {
                "extended"
            } else {
                "advanced"
            }
        } else if dialect == Dialect::Ios {
            "standard"
        } else {
            "basic"
        };
        match dialect {
            Dialect::Ios => {
                out.push_str(&format!("{} IP access list {}\n", kind, acl.number));
                for r in &acl.rules {
                    let action = if r.permit { "permit" } else { "deny" };
                    let proto = match r.proto {
                        AclProto::Icmp => "icmp ",
                        AclProto::Any => "",
                    };
                    if (100..=199).contains(&acl.number) {
                        out.push_str(&format!(
                            "    {} {}{} {} {}\n",
                            action,
                            proto,
                            ipv4_str(r.src),
                            mask_str_wildcard(r.src_wildcard),
                            format_args!(
                                "{} {}",
                                ipv4_str(r.dst),
                                mask_str_wildcard(r.dst_wildcard)
                            )
                        ));
                    } else {
                        out.push_str(&format!("    {} {}\n", action, ipv4_str(r.src)));
                    }
                }
            }
            _ => {
                let kind_cn =
                    if (100..=199).contains(&acl.number) || (3000..=3999).contains(&acl.number) {
                        "advanced"
                    } else {
                        "basic"
                    };
                out.push_str(&format!(" ACL number: {}  Type: {}\n", acl.number, kind_cn));
                for r in &acl.rules {
                    let action = if r.permit { "permit" } else { "deny" };
                    let proto = match r.proto {
                        AclProto::Icmp => " icmp",
                        AclProto::Any => "",
                    };
                    let dst_part = if r.dst != 0 || r.dst_wildcard != 0 {
                        format!(
                            " destination {} {}",
                            ipv4_str(r.dst),
                            ipv4_str(r.dst_wildcard)
                        )
                    } else {
                        String::new()
                    };
                    out.push_str(&format!(
                        " rule 5 {}{} source {} {}{}  (matched {})\n",
                        action,
                        proto,
                        ipv4_str(r.src),
                        ipv4_str(r.src_wildcard),
                        dst_part,
                        r.hits
                    ));
                }
            }
        }
    }
    out.trim_end().to_string()
}

/// 通配符显示：0.0.0.0 → host 形式保留点分
fn mask_str_wildcard(w: u32) -> String {
    ipv4_str(w)
}

/// `show ip nat translations` / `display nat session`（简化）。
pub fn nat_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(rs) = &d.state else {
        return String::new();
    };
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(
                "Pro  Inside global      Inside local       Outside local      Outside global\n",
            );
            for e in &rs.nat_entries {
                out.push_str(&format!(
                    "icmp {}         {}         {}         {}\n",
                    ipv4_str(e.outside_ip),
                    ipv4_str(e.inside_ip),
                    ipv4_str(e.outside_ip),
                    ipv4_str(e.outside_ip)
                ));
            }
            if rs.nat_entries.is_empty() {
                out.push_str("（无转换表项）");
            }
        }
        _ => {
            out.push_str("  Protocol  Inside IP        Outside IP\n");
            for e in &rs.nat_entries {
                out.push_str(&format!(
                    "  icmp      {}     {}\n",
                    ipv4_str(e.inside_ip),
                    ipv4_str(e.outside_ip)
                ));
            }
            if rs.nat_entries.is_empty() {
                out.push_str("Info: No NAT session.");
            }
        }
    }
    out.trim_end().to_string()
}

/// `show vlan brief` / `display vlan`。
pub fn vlan_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Switch(sw) = &d.state else {
        return String::new();
    };
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<5}{:<33}{:<10}{}\n",
                "VLAN", "Name", "Status", "Ports"
            ));
            out.push_str(&format!(
                "{:<5}{:<33}{:<10}{}\n",
                "----",
                "--------------------------------",
                "---------",
                "-----------------------------"
            ));
        }
        _ => {
            out.push_str(&format!("  {:<8}{:<24}{}\n", "VLAN ID", "Name", "Ports"));
            out.push_str(&format!(
                "  {:<8}{:<24}{}\n",
                "--------", "------------------------", "------------------------------------"
            ));
        }
    }
    let mut vlans = sw.vlans.clone();
    vlans.sort_unstable();
    for vlan in vlans {
        let mut members: Vec<String> = vec![];
        // SVI 也是 VLAN 成员
        if sw.svi.contains_key(&vlan) {
            members.push(format!("{}(U)", ifname::svi_short(dialect, vlan)));
        }
        for p in &d.ports {
            let carries = match p.mode {
                PortMode::Access => p.pvid == vlan,
                PortMode::Trunk => {
                    p.pvid == vlan || p.trunk_allowed.as_ref().is_none_or(|v| v.contains(&vlan))
                }
            };
            if !carries {
                continue;
            }
            let name = ifname::short(dialect, d.kind, p.idx);
            let tag = match p.mode {
                PortMode::Access => "(U)".to_string(),
                PortMode::Trunk => {
                    if p.pvid == vlan {
                        "(U)".to_string()
                    } else {
                        "(T)".to_string()
                    }
                }
            };
            members.push(format!("{}{}", name, tag));
        }
        let ports_str = members.join(" ");
        let name = if vlan == DEFAULT_VLAN {
            "default".to_string()
        } else {
            format!("VLAN{:04}", vlan)
        };
        match dialect {
            Dialect::Ios => out.push_str(&format!(
                "{:<5}{:<33}{:<10}{}\n",
                vlan, name, "active", ports_str
            )),
            _ => out.push_str(&format!("  {:<8}{:<24}{}\n", vlan, name, ports_str)),
        }
    }
    out.trim_end().to_string()
}

/// `show mac address-table` / `display mac-address`。
pub fn mac_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Switch(sw) = &d.state else {
        return String::new();
    };
    let mut entries: Vec<((VlanId, Mac), PortId)> =
        sw.mac_table.iter().map(|(k, v)| (*k, *v)).collect();
    entries.sort_by_key(|((vlan, mac), _)| (*vlan, *mac));
    let mac_fmt = |m: &Mac| match dialect {
        Dialect::Ios => sim_packets::mac_str_cisco(m),
        _ => sim_packets::mac_str_huawei(m),
    };
    if entries.is_empty() {
        return match dialect {
            Dialect::Ios => {
                "          Mac Address Table\n-------------------------------------------".into()
            }
            _ => "MAC Address Table\n（空）".into(),
        };
    }
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("          Mac Address Table\n");
            out.push_str("-------------------------------------------\n");
            out.push_str(&format!(
                "{:<5}{:<18}{:<12}{}\n",
                "Vlan", "Mac Address", "Type", "Ports"
            ));
            out.push_str(&format!(
                "{:<5}{:<18}{:<12}{}\n",
                "----", "-----------", "--------", "-----"
            ));
            for ((vlan, mac), port) in entries {
                out.push_str(&format!(
                    "{:<5}{:<18}{:<12}{}\n",
                    vlan,
                    mac_fmt(&mac),
                    "DYNAMIC",
                    ifname::display(dialect, d.kind, port)
                ));
            }
        }
        _ => {
            out.push_str("MAC Address Table\n");
            out.push_str(
                "----------------------------------------------------------------------\n",
            );
            out.push_str(&format!(
                "{:<16}{:<13}{:<20}{}\n",
                "MAC Address", "VLAN/VSI", "Learned-From", "Type"
            ));
            for ((vlan, mac), port) in entries {
                out.push_str(&format!(
                    "{:<16}{:<13}{:<20}{}\n",
                    mac_fmt(&mac),
                    vlan,
                    ifname::display(dialect, d.kind, port),
                    "dynamic"
                ));
            }
        }
    }
    out.trim_end().to_string()
}
