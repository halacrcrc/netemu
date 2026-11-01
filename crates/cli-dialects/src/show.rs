//! `show` / `display` 输出渲染与 ping 结果渲染。
//!
//! 原则：所有输出都从模拟状态实时渲染，绝不硬编码。

use sim_packets::Mac;

use sim_core::device::{
    AclProto, DevId, DeviceKind, DeviceState, Dialect, LagMode, PingProbeState, PingSession,
    PortId, PortMode, Route, RouteSrc, StpMode, StpRole, StpSt, SviState, TcpProbe, VlanId,
    DEFAULT_VLAN, STP_COST_1G,
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

fn route_rows(eng: &Engine, dev: DevId, dialect: Dialect, vrf: Option<u16>) -> Vec<RouteRow> {
    let Some(d) = eng.net.device(dev) else {
        return vec![];
    };
    let svi_name = |vlan: VlanId| ifname::svi_short(dialect, vlan);
    let mut rows: Vec<RouteRow> = d
        .connected_routes_vrf(vrf)
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
    let (statics, ospf, bgp, isis): (&[Route], &[Route], &[Route], &[Route]) = match &d.state {
        DeviceState::Router(rs) => {
            if let Some(id) = vrf {
                let st = rs
                    .vrfs
                    .iter()
                    .find(|v| v.id == id)
                    .map(|v| v.static_routes.as_slice())
                    .unwrap_or(&[]);
                (st, &[], &[], &[])
            } else {
                (
                    &rs.static_routes,
                    &rs.ospf_routes,
                    &rs.bgp_routes,
                    &rs.isis_routes,
                )
            }
        }
        DeviceState::Switch(sw) => (&sw.static_routes, &[], &[], &[]),
        _ => (&[], &[], &[], &[]),
    };
    let mut add_dynamic = |routes: &[Route], src: RouteSrc| {
        for r in routes {
            // 出接口 = 下一跳所在连接网段
            let port = d.connected_routes_vrf(vrf).iter().find_map(|c| {
                if (r.nexthop & mask_from_prefix(c.len)) == c.dst {
                    Some(c.port)
                } else {
                    None
                }
            });
            rows.push(RouteRow {
                dst: ipv4_str(r.dst),
                len: r.len,
                src,
                nexthop: ipv4_str(r.nexthop),
                iface: port
                    .map(|p| ifname::display(dialect, d.kind, p))
                    .unwrap_or_else(|| "?".into()),
            });
        }
    };
    add_dynamic(statics, RouteSrc::Static);
    add_dynamic(ospf, RouteSrc::Ospf);
    add_dynamic(bgp, RouteSrc::Bgp);
    add_dynamic(isis, RouteSrc::Isis);
    rows.sort_by(|a, b| {
        let rank = |s: RouteSrc| match s {
            RouteSrc::Connected => 5,
            RouteSrc::Static => 4,
            RouteSrc::Bgp => 3,
            RouteSrc::Ospf => 2,
            RouteSrc::Isis => 1,
        };
        b.len
            .cmp(&a.len)
            .then(a.dst.cmp(&b.dst))
            .then(rank(b.src).cmp(&rank(a.src)))
    });
    rows
}

/// `show ip route` / `display ip routing-table`。
pub fn route_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = route_rows(eng, dev, dialect, None);
    render_route_rows(&rows, dialect)
}

/// 指定 VRF 的路由表。
pub fn route_table_vrf(eng: &Engine, dev: DevId, dialect: Dialect, vrf: u16) -> String {
    let rows = route_rows(eng, dev, dialect, Some(vrf));
    render_route_rows(&rows, dialect)
}

fn render_route_rows(rows: &[RouteRow], dialect: Dialect) -> String {
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("Codes: C - connected, S - static\n\n");
            out.push_str("Gateway of last resort is not set\n\n");
            for r in rows {
                let code = match r.src {
                    RouteSrc::Connected => 'C',
                    RouteSrc::Static => 'S',
                    RouteSrc::Ospf => 'O',
                    RouteSrc::Bgp => 'B',
                    RouteSrc::Isis => 'i',
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
            for r in rows {
                let (proto, pre, flags) = match r.src {
                    RouteSrc::Connected => ("Direct", 0, "D"),
                    RouteSrc::Static => ("Static", 60, "RD"),
                    RouteSrc::Ospf => ("OSPF", 10, "D"),
                    RouteSrc::Bgp => ("BGP", 20, "D"),
                    RouteSrc::Isis => ("ISIS", 115, "D"),
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
            for r in rows {
                let proto = match r.src {
                    RouteSrc::Connected => "Direct",
                    RouteSrc::Static => "Static",
                    RouteSrc::Ospf => "OSPF",
                    RouteSrc::Bgp => "BGP",
                    RouteSrc::Isis => "ISIS",
                };
                out.push_str(&format!(
                    "{:<18} {:<7} {:<4} {:<5} {:<15} {}\n",
                    format!("{}/{}", r.dst, r.len),
                    proto,
                    match r.src {
                        RouteSrc::Connected => 0,
                        RouteSrc::Static => 60,
                        RouteSrc::Ospf => 10,
                        RouteSrc::Bgp => 20,
                        RouteSrc::Isis => 115,
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

/// PC `tcp <ip> <port>` 结果。
pub fn render_tcp_probe(p: &TcpProbe) -> String {
    let target = format!("{}:{}", ipv4_str(p.target), p.target_port);
    match p.opened {
        Some(true) => format!("TCP {target} 连接成功（SYN → SYN-ACK → ACK，FIN 已发送）"),
        Some(false) => format!("TCP {target} 连接被拒绝（RST）"),
        None => format!("TCP {target} 无响应（可能被策略丢弃或在途）"),
    }
}

/// 设备是否具有 L3 能力（show ip route / ping 的前提）。
pub fn is_l3_device(d: &Device) -> bool {
    matches!(
        d.kind,
        DeviceKind::Router | DeviceKind::Switch | DeviceKind::Firewall | DeviceKind::WlanController
    )
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
    if stp.mode == StpMode::Mstp {
        return mst_table(eng, dev, dialect);
    }
    let is_root = stp.root.mac == port_mac(dev, 0) && stp.root_cost == 0;
    let my_bridge = stp.bridge_id(dev);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("VLAN0001\n");
            let protocol = if stp.mode == StpMode::Rstp {
                "rapid-pvst"
            } else {
                "ieee"
            };
            out.push_str(&format!("  Spanning tree enabled protocol {}\n", protocol));
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
                // 聚合组成员只在代表端口上显示一行
                if p.lag.is_some() && eng.logical_port(dev, p.idx) != p.idx {
                    continue;
                }
                let role = stp.role_of(p.idx);
                let state = stp.state_of(p.idx);
                if role == StpRole::Disabled && p.link.is_none() {
                    continue; // IOS 不显示未接线的端口
                }
                let name = match p.lag {
                    Some(gid) => ifname::lag_canonical(dialect, gid),
                    None => ifname::display(dialect, d.kind, p.idx),
                };
                out.push_str(&format!(
                    "{:<19} {:<4} {:<3} {:<9} {:<8} {}\n",
                    name,
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
                " Mode: {}\n\n",
                if stp.mode == StpMode::Rstp {
                    "RSTP"
                } else {
                    "STP"
                }
            ));
            out.push_str(&format!(
                " {:<6} {:<28} {:<5} {:<12} {}\n",
                "MSTID", "Port", "Role", "STP State", "Protection"
            ));
            out.push_str(&format!(
                " {:<6} {:<28} {:<5} {:<12} {}\n",
                "-----", "----------------------------", "-----", "------------", "----------"
            ));
            for p in &d.ports {
                if p.lag.is_some() && eng.logical_port(dev, p.idx) != p.idx {
                    continue;
                }
                let role = stp.role_of(p.idx);
                let state = stp.state_of(p.idx);
                if role == StpRole::Disabled && p.link.is_none() {
                    continue;
                }
                let name = match p.lag {
                    Some(gid) => ifname::lag_canonical(dialect, gid),
                    None => ifname::display(dialect, d.kind, p.idx),
                };
                out.push_str(&format!(
                    " {:<6} {:<28} {:<5} {:<12} {}\n",
                    0,
                    name,
                    stp_role_str(dialect, role),
                    stp_state_str(dialect, state),
                    "NONE"
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show ipv6 interface brief` / `display ipv6 interface brief`。
pub fn ipv6_int_brief(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<24}{:<26}{:<10}{}\n",
                "Interface", "IPv6-Address", "Status", "Protocol"
            ));
            for p in &d.ports {
                let Some(ip) = p.ipv6 else { continue };
                let oper = p.oper_up(p.link.map(|l| eng.net.link_up(l)).unwrap_or(false));
                out.push_str(&format!(
                    "{:<24}{:<26}{:<10}{}\n",
                    ifname::display(dialect, d.kind, p.idx),
                    format!("{}/{}", sim_packets::ipv6_str(&ip), p.prefix6),
                    if p.admin_up { "up" } else { "down" },
                    if oper { "up" } else { "down" }
                ));
            }
            if !d.ports.iter().any(|p| p.ipv6.is_some()) {
                out.push_str("（无 IPv6 接口）");
            }
        }
        _ => {
            out.push_str(&format!(
                " {:<26}{:<28}{:<10}{}\n",
                "Interface", "IPv6 Address/Mask", "Physical", "Protocol"
            ));
            for p in &d.ports {
                let Some(ip) = p.ipv6 else { continue };
                let oper = p.oper_up(p.link.map(|l| eng.net.link_up(l)).unwrap_or(false));
                out.push_str(&format!(
                    " {:<26}{:<28}{:<10}{}\n",
                    ifname::display(dialect, d.kind, p.idx),
                    format!("{}/{}", sim_packets::ipv6_str(&ip), p.prefix6),
                    if oper { "up" } else { "down" },
                    if oper { "up" } else { "down" }
                ));
            }
            if !d.ports.iter().any(|p| p.ipv6.is_some()) {
                out.push_str(" Info: No IPv6 interface.\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// `show ipv6 route` / `display ipv6 routing-table`。
pub fn ipv6_route_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    struct Row6 {
        dst: String,
        len: u8,
        proto: &'static str,
        nh: String,
        iface: String,
    }
    let mut rows: Vec<Row6> = vec![];
    for p in &d.ports {
        if let Some(ip) = p.ipv6 {
            let net = sim_packets::ipv6_net_addr(&ip, p.prefix6);
            rows.push(Row6 {
                dst: sim_packets::ipv6_str(&net),
                len: p.prefix6,
                proto: "C",
                nh: "::".into(),
                iface: ifname::display(dialect, d.kind, p.idx),
            });
        }
    }
    if let DeviceState::Router(rs) = &d.state {
        for r in &rs.static_routes6 {
            let port = d.ports.iter().find(|p| {
                p.ipv6
                    .map(|ip| sim_packets::ipv6_in_prefix(&r.nexthop, &ip, p.prefix6))
                    .unwrap_or(false)
            });
            rows.push(Row6 {
                dst: sim_packets::ipv6_str(&r.dst),
                len: r.len,
                proto: "S",
                nh: sim_packets::ipv6_str(&r.nexthop),
                iface: port
                    .map(|p| ifname::display(dialect, d.kind, p.idx))
                    .unwrap_or_else(|| "?".into()),
            });
        }
    }
    rows.sort_by(|a, b| b.len.cmp(&a.len).then(a.dst.cmp(&b.dst)));
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("IPv6 Routing Table - default\nCodes: C - connected, S - static\n\n");
            for r in &rows {
                out.push_str(&format!(
                    "{}   {}/{} [0/0] via {}, {}\n",
                    r.proto, r.dst, r.len, r.nh, r.iface
                ));
            }
            if rows.is_empty() {
                out.push_str("（无 IPv6 路由）");
            }
        }
        _ => {
            out.push_str(&format!(
                " {:<34}{:<7}{:<18}{}\n",
                "Destination/Mask", "Proto", "NextHop", "Interface"
            ));
            for r in &rows {
                out.push_str(&format!(
                    " {:<34}{:<7}{:<18}{}\n",
                    format!("{}/{}", r.dst, r.len),
                    if r.proto == "C" { "Direct" } else { "Static" },
                    r.nh,
                    r.iface
                ));
            }
            if rows.is_empty() {
                out.push_str(" Info: No IPv6 route.\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// IPv6 ping 结果（路由器三方言）。
pub fn render_ping6(dialect: Dialect, s: &sim_core::device::Ping6Session) -> String {
    let target = sim_packets::ipv6_str(&s.target);
    let rate = if s.sent == 0 {
        0
    } else {
        (s.received as f64 / s.sent as f64 * 100.0) as u64
    };
    match dialect {
        Dialect::Ios => format!(
            "Sending {}, 100-byte ICMPv6 Echos to {}\nSuccess rate is {} percent ({}/{})",
            s.sent, target, rate, s.received, s.sent
        ),
        Dialect::Vrp => format!(
            "  PING6 {}: 56 data bytes\n    {} packet(s) transmitted\n    {} packet(s) received\n    {:.2}% packet loss",
            target,
            s.sent,
            s.received,
            if s.sent == 0 {
                100.0
            } else {
                (s.sent - s.received) as f64 / s.sent as f64 * 100.0
            }
        ),
        Dialect::Comware => format!(
            "PING6 {}: 56 data bytes\n{} packet(s) transmitted, {} packet(s) received, {:.1}% packet loss",
            target,
            s.sent,
            s.received,
            if s.sent == 0 {
                100.0
            } else {
                (s.sent - s.received) as f64 / s.sent as f64 * 100.0
            }
        ),
    }
}

/// 主机的 IPv6 ping（linux 风格）。
pub fn render_ping6_generic(s: &sim_core::device::Ping6Session) -> String {
    let target = sim_packets::ipv6_str(&s.target);
    let loss = if s.sent == 0 {
        100.0
    } else {
        (s.sent - s.received) as f64 / s.sent as f64 * 100.0
    };
    format!(
        "PING6 {}: 56 data bytes\n{} packets transmitted, {} received, {:.0}% packet loss",
        target, s.sent, s.received, loss
    )
}

/// `show ip dhcp snooping binding` / `display dhcp snooping user-binding`。
pub fn dhcp_snooping_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = eng.dhcp_bindings_of(dev);
    if rows.is_empty() {
        return match dialect {
            Dialect::Ios => "（无 DHCP Snooping 绑定）".into(),
            _ => " Info: No DHCP snooping binding.\n".into(),
        };
    }
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<20}{:<18}{:<8}{}\n",
                "MacAddress", "IPAddress", "VLAN", "Interface"
            ));
            for b in rows {
                out.push_str(&format!(
                    "{:<20}{:<18}{:<8}{}\n",
                    sim_packets::mac_str_cisco(&b.mac),
                    ipv4_str(b.ip),
                    b.vlan,
                    ifname::display(dialect, DeviceKind::Switch, b.port)
                ));
            }
        }
        _ => {
            out.push_str(&format!(
                " {:<20}{:<18}{:<8}{}\n",
                "MAC Address", "IP Address", "VLAN", "Interface"
            ));
            for b in rows {
                out.push_str(&format!(
                    " {:<20}{:<18}{:<8}{}\n",
                    sim_packets::mac_str_huawei(&b.mac),
                    ipv4_str(b.ip),
                    b.vlan,
                    ifname::display(dialect, DeviceKind::Switch, b.port)
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show port-security` / `display port-security`。
pub fn port_security_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let mut out = String::new();
    let mut any = false;
    for p in &d.ports {
        let Some((cfg, current, viol)) = eng.port_security_of(dev, p.idx) else {
            continue;
        };
        if !any {
            any = true;
            if dialect == Dialect::Ios {
                out.push_str(&format!(
                    "{:<22}{:<8}{:<10}{:<12}{}\n",
                    "Port", "Max", "Current", "Violations", "Action"
                ));
            } else {
                out.push_str(&format!(
                    " {:<20}{:<8}{:<10}{:<12}{}\n",
                    "Interface", "Max", "Current", "Violations", "Action"
                ));
            }
        }
        out.push_str(&format!(
            "{:<22}{:<8}{:<10}{:<12}{}\n",
            ifname::display(dialect, DeviceKind::Switch, p.idx),
            cfg.max,
            current,
            viol,
            match cfg.violation {
                sim_core::device::PortSecViolation::Shutdown => "shutdown",
                sim_core::device::PortSecViolation::Restrict => "restrict",
            }
        ));
    }
    if !any {
        return match dialect {
            Dialect::Ios => "（未启用端口安全）".into(),
            _ => " Info: Port security is disabled.\n".into(),
        };
    }
    out.trim_end().to_string()
}

/// `show storm-control` / `display storm-control`（含入口限速统计）。
pub fn qos_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = eng.qos_stats_of(dev);
    if rows.is_empty() {
        return match dialect {
            Dialect::Ios => "（未配置风暴抑制 / 限速）".into(),
            _ => " Info: No QoS policy.\n".into(),
        };
    }
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<22}{:<14}{:<14}{:<10}{}\n",
                "Interface", "BcastLimit", "RateLimit(kbps)", "Drops", "BcastDrops"
            ));
            for (port, bcast, rate, drops, bdrops) in rows {
                out.push_str(&format!(
                    "{:<22}{:<14}{:<14}{:<10}{}\n",
                    ifname::display(dialect, DeviceKind::Switch, port),
                    bcast.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    rate.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    drops,
                    bdrops
                ));
            }
        }
        _ => {
            out.push_str(&format!(
                " {:<20}{:<14}{:<16}{:<10}{}\n",
                "Interface", "BcastLimit", "RateLimit(kbps)", "Drops", "BcastDrops"
            ));
            for (port, bcast, rate, drops, bdrops) in rows {
                out.push_str(&format!(
                    " {:<20}{:<14}{:<16}{:<10}{}\n",
                    ifname::display(dialect, DeviceKind::Switch, port),
                    bcast.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    rate.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                    drops,
                    bdrops
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show vrrp` / `display vrrp`。
pub fn vrrp_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(rs) = &d.state else {
        return String::new();
    };
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<22}{:<6}{:<8}{:<12}{}\n",
                "Interface", "Grp", "Prio", "State", "Virtual addr"
            ));
            for g in &rs.vrrp {
                out.push_str(&format!(
                    "{:<22}{:<6}{:<8}{:<12}{}\n",
                    ifname::display(dialect, d.kind, g.port),
                    g.vrid,
                    g.priority,
                    match g.state {
                        sim_core::device::VrrpState::Master => "Master",
                        sim_core::device::VrrpState::Backup => "Backup",
                        sim_core::device::VrrpState::Init => "Initialize",
                    },
                    ipv4_str(g.vip)
                ));
            }
            if rs.vrrp.is_empty() {
                out.push_str("（未配置 VRRP）");
            }
        }
        _ => {
            out.push_str(&format!(
                " {:<16}{:<6}{:<8}{:<10}{}\n",
                "Interface", "VRID", "Priority", "State", "Virtual IP"
            ));
            for g in &rs.vrrp {
                out.push_str(&format!(
                    " {:<16}{:<6}{:<8}{:<10}{}\n",
                    ifname::display(dialect, d.kind, g.port),
                    g.vrid,
                    g.priority,
                    match g.state {
                        sim_core::device::VrrpState::Master => "Master",
                        sim_core::device::VrrpState::Backup => "Backup",
                        sim_core::device::VrrpState::Init => "Initialize",
                    },
                    ipv4_str(g.vip)
                ));
            }
            if rs.vrrp.is_empty() {
                out.push_str(" Info: No VRRP group.\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// `show mpls lsp` / `display mpls lsp`。
pub fn mpls_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = eng.mpls_lsps_of(dev);
    if rows.is_empty() {
        return match dialect {
            Dialect::Ios => "（无静态 LSP）".into(),
            _ => " Info: No static LSP.\n".into(),
        };
    }
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<16}{:<10}{:<20}{:<10}{:<10}{:<14}{:<10}{}\n",
                "Name", "Type", "FEC", "In", "Out", "NextHop", "VRF", "VPN"
            ));
            for l in rows {
                let kind = if l.in_label.is_none() {
                    "ingress"
                } else if l.out_label.is_none() {
                    "egress"
                } else {
                    "transit"
                };
                out.push_str(&format!(
                    "{:<16}{:<10}{:<20}{:<10}{:<10}{:<14}{:<10}{}\n",
                    l.name,
                    kind,
                    l.net
                        .map(|(n, len)| format!("{}/{}", ipv4_str(n), len))
                        .unwrap_or_else(|| "-".into()),
                    l.in_label
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    l.out_label
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    l.nexthop.map(ipv4_str).unwrap_or_else(|| "-".into()),
                    l.vrf.clone().unwrap_or_else(|| "-".into()),
                    l.vpn_label
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into())
                ));
            }
        }
        _ => {
            out.push_str(&format!(
                " {:<16}{:<10}{:<20}{:<8}{:<8}{:<14}{:<10}{}\n",
                "Name", "Type", "FEC", "In", "Out", "NextHop", "VRF", "VPN"
            ));
            for l in rows {
                let kind = if l.in_label.is_none() {
                    "Ingress"
                } else if l.out_label.is_none() {
                    "Egress"
                } else {
                    "Transit"
                };
                out.push_str(&format!(
                    " {:<16}{:<10}{:<20}{:<8}{:<8}{:<14}{:<10}{}\n",
                    l.name,
                    kind,
                    l.net
                        .map(|(n, len)| format!("{}/{}", ipv4_str(n), len))
                        .unwrap_or_else(|| "-".into()),
                    l.in_label
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    l.out_label
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    l.nexthop.map(ipv4_str).unwrap_or_else(|| "-".into()),
                    l.vrf.clone().unwrap_or_else(|| "-".into()),
                    l.vpn_label
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into())
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show isis neighbors` / `display isis peer`。
pub fn isis_neighbors(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let sys = eng
        .isis_system_of(dev)
        .map(|s| {
            s.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_else(|| "-".into());
    let rows = eng.isis_neighbors_of(dev);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("System Id      Interface        State     Holdtime\n");
        }
        _ => {
            out.push_str(&format!(
                "System ID: {}\n\n {:<22}{:<24}{}\n",
                sys, "System ID", "Interface", "State"
            ));
        }
    }
    for (sid, _ip, port) in &rows {
        let id = sid
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");
        if dialect == Dialect::Ios {
            out.push_str(&format!(
                "{:<14}{:<16}{:<9}{}\n",
                id,
                ifname::display(dialect, DeviceKind::Router, *port),
                "UP",
                30
            ));
        } else {
            out.push_str(&format!(
                " {:<22}{:<24}{}\n",
                id,
                ifname::display(dialect, DeviceKind::Router, *port),
                "Up"
            ));
        }
    }
    if rows.is_empty() {
        out.push_str(match dialect {
            Dialect::Ios => "（无 IS-IS 邻居）",
            _ => " Info: No IS-IS neighbor.\n",
        });
    }
    out.trim_end().to_string()
}

/// `show isis database` / `display isis lsdb`。
pub fn isis_lsdb(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = eng.isis_lsdb_of(dev);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => out.push_str(&format!(
            "{:<16}{:<8}{}\n",
            "System Id", "Links", "Neighbors"
        )),
        _ => out.push_str(&format!(
            " {:<16}{:<8}{}\n",
            "System ID", "Links", "Neighbors"
        )),
    }
    for (sid, links, nbs) in &rows {
        let id = sid
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");
        if dialect == Dialect::Ios {
            out.push_str(&format!("{:<16}{:<8}{}\n", id, links, nbs));
        } else {
            out.push_str(&format!(" {:<16}{:<8}{}\n", id, links, nbs));
        }
    }
    if rows.is_empty() {
        out.push_str(match dialect {
            Dialect::Ios => "（LSDB 为空）",
            _ => " Info: The LSDB is empty.\n",
        });
    }
    out.trim_end().to_string()
}

/// `show ip bgp summary` / `display bgp peer`。
pub fn bgp_summary(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(rs) = &d.state else {
        return String::new();
    };
    let Some(cfg) = &rs.bgp else {
        return match dialect {
            Dialect::Ios => "% BGP is not configured.".into(),
            _ => " Info: BGP is not configured.\n".into(),
        };
    };
    let rid = ipv4_str(eng.ospf_router_id_of(dev));
    let sessions = eng.bgp_sessions_of(dev);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "BGP router identifier {}, local AS number {}\n\n{:<17}{:<4}{:<8}{:<10}{}\n",
                rid, cfg.asn, "Neighbor", "V", "AS", "MsgRcvd", "State"
            ));
            for (ip, asn, est) in sessions {
                out.push_str(&format!(
                    "{:<17}{:<4}{:<8}{:<10}{}\n",
                    ipv4_str(ip),
                    4,
                    asn,
                    "-",
                    if est { "Established" } else { "Idle" }
                ));
            }
        }
        _ => {
            out.push_str(&format!(
                "BGP local router ID : {}\n Local AS number : {}\n\n {:<17}{:<8}{}\n",
                rid, cfg.asn, "Peer", "AS", "State"
            ));
            for (ip, asn, est) in sessions {
                out.push_str(&format!(
                    " {:<17}{:<8}{}\n",
                    ipv4_str(ip),
                    asn,
                    if est { "Established" } else { "Idle" }
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show ip bgp` / `display bgp routing-table`。
pub fn bgp_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rows = eng.bgp_routes_of(dev);
    if rows.is_empty() {
        return match dialect {
            Dialect::Ios => "（无 BGP 路由）".into(),
            _ => " Info: No BGP route.\n".into(),
        };
    }
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "{:<20}{:<16}{:<8}{}\n",
                "Network", "Next Hop", "LocPrf", "AS Path"
            ));
            for (net, len, path, nh, lp) in rows {
                out.push_str(&format!(
                    "{:<20}{:<16}{:<8}{}\n",
                    format!("{}/{}", ipv4_str(net), len),
                    ipv4_str(nh),
                    lp,
                    path.iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        }
        _ => {
            out.push_str(&format!(
                " {:<20}{:<16}{:<8}{}\n",
                "Network", "NextHop", "LocPrf", "AS_PATH"
            ));
            for (net, len, path, nh, lp) in rows {
                out.push_str(&format!(
                    " {:<20}{:<16}{:<8}{}\n",
                    format!("{}/{}", ipv4_str(net), len),
                    ipv4_str(nh),
                    lp,
                    path.iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show route-map` / `display route-policy`。
pub fn route_policies(eng: &Engine, dev: DevId, dialect: Dialect, only: Option<&str>) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(rs) = &d.state else {
        return String::new();
    };
    let mut list: Vec<&sim_core::device::RoutePolicy> = rs
        .route_policies
        .iter()
        .filter(|p| only.is_none_or(|n| p.name == n))
        .collect();
    if list.is_empty() {
        return match dialect {
            Dialect::Ios => "% No route-map configured.".into(),
            _ => " Info: No route-policy.\n".into(),
        };
    }
    list.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            for rp in list {
                let mut nodes: Vec<&sim_core::device::RoutePolicyNode> = rp.nodes.iter().collect();
                nodes.sort_by_key(|n| n.seq);
                for node in nodes {
                    out.push_str(&format!(
                        "route-map {} {} {}\n",
                        rp.name,
                        if node.permit { "permit" } else { "deny" },
                        node.seq
                    ));
                    out.push_str("  Match clauses:\n");
                    match node.match_acl {
                        Some(acl) => out.push_str(&format!("    ip address {}\n", acl)),
                        None => out.push_str("    (none)\n"),
                    }
                    out.push_str("  Set clauses:\n");
                    match node.set_local_pref {
                        Some(lp) => out.push_str(&format!("    local-preference {}\n", lp)),
                        None => out.push_str("    (none)\n"),
                    }
                }
            }
        }
        _ => {
            for rp in list {
                out.push_str(&format!("Route-policy: {}\n", rp.name));
                let mut nodes: Vec<&sim_core::device::RoutePolicyNode> = rp.nodes.iter().collect();
                nodes.sort_by_key(|n| n.seq);
                for node in nodes {
                    out.push_str(&format!(
                        "  node {} {}:\n",
                        node.seq,
                        if node.permit { "permit" } else { "deny" }
                    ));
                    if let Some(acl) = node.match_acl {
                        out.push_str(&format!("    if-match acl {}\n", acl));
                    }
                    if let Some(lp) = node.set_local_pref {
                        out.push_str(&format!("    apply local-preference {}\n", lp));
                    }
                }
            }
        }
    }
    out.trim_end().to_string()
}

/// `display firewall zone` / `display security-zone` / `show zone security`。
pub fn security_zones(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(_) = &d.state else {
        return String::new();
    };
    let mut zones: Vec<(String, Vec<usize>)> = vec![];
    for p in &d.ports {
        let Some(z) = &p.zone else { continue };
        match zones.iter_mut().find(|(n, _)| n == z) {
            Some((_, ports)) => ports.push(p.idx),
            None => zones.push((z.clone(), vec![p.idx])),
        }
    }
    if zones.is_empty() {
        return match dialect {
            Dialect::Ios => "No security zone configured.".into(),
            _ => " Info: No security zone.\n".into(),
        };
    }
    let mut out = String::new();
    for (name, mut ports) in zones {
        ports.sort_unstable();
        out.push_str(&format!("Zone: {}\n", name));
        for p in ports {
            out.push_str(&format!(
                "  Interface: {}\n",
                ifname::display(dialect, d.kind, p)
            ));
        }
    }
    out.trim_end().to_string()
}

/// `display security-policy` / `show zone-pair security`。
pub fn security_policies(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(rs) = &d.state else {
        return String::new();
    };
    if rs.security_policies.is_empty() {
        return match dialect {
            Dialect::Ios => "No zone-pair configured.".into(),
            _ => " Info: No security policy.\n".into(),
        };
    }
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            for p in &rs.security_policies {
                out.push_str(&format!("Zone-pair: {}\n", p.name));
                out.push_str(&format!("  source {}\n", p.src_zone));
                out.push_str(&format!("  destination {}\n", p.dst_zone));
                out.push_str(&format!(
                    "  action {}\n",
                    if p.permit { "permit" } else { "deny" }
                ));
            }
        }
        _ => {
            out.push_str("Security-policy\n");
            for p in &rs.security_policies {
                out.push_str(&format!("  rule name {}\n", p.name));
                out.push_str(&format!("    source-zone {}\n", p.src_zone));
                out.push_str(&format!("    destination-zone {}\n", p.dst_zone));
                out.push_str(&format!(
                    "    action {}\n",
                    if p.permit { "permit" } else { "deny" }
                ));
            }
        }
    }
    out.trim_end().to_string()
}

/// `show ipsec tunnel` / `display ipsec tunnel`。
pub fn ipsec_tunnels(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Router(rs) = &d.state else {
        return String::new();
    };
    if rs.ipsec_tunnels.is_empty() {
        return match dialect {
            Dialect::Ios => "No IPsec tunnel configured.".into(),
            _ => " Info: No IPsec tunnel.\n".into(),
        };
    }
    let mut out = match dialect {
        Dialect::Ios => format!(
            "{:<14}{:<16}{:<16}{:<20}{}\n",
            "Name", "Local", "Remote", "Local Subnet", "Remote Subnet"
        ),
        _ => format!(
            " {:<14}{:<16}{:<16}{:<20}{}\n",
            "Name", "Local", "Remote", "Local Subnet", "Remote Subnet"
        ),
    };
    for t in &rs.ipsec_tunnels {
        let lsub = format!("{}/{}", ipv4_str(t.local_subnet.0), t.local_subnet.1);
        let rsub = format!("{}/{}", ipv4_str(t.remote_subnet.0), t.remote_subnet.1);
        let row = format!(
            "{:<14}{:<16}{:<16}{:<20}{}",
            t.name,
            ipv4_str(t.local),
            ipv4_str(t.remote),
            lsub,
            rsub
        );
        out.push_str(&row);
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// `show sslvpn session` / `display sslvpn session`。
pub fn sslvpn_sessions(eng: &Engine, dev: DevId, _dialect: Dialect) -> String {
    let rows = eng.sslvpn_clients_of(dev);
    if rows.is_empty() {
        return " Info: No SSL VPN session.\n".into();
    }
    let mut out = String::from("Virtual IP      Client IP\n");
    for (v, c) in rows {
        out.push_str(&format!("{:<15} {}\n", ipv4_str(v), ipv4_str(c)));
    }
    out.trim_end().to_string()
}

/// `show wlan` / `display wlan`（AC 与 AP）。
pub fn wlan_table(eng: &Engine, dev: DevId, _dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let mut out = String::from("WLAN profiles:\n");
    if d.wlan.profiles.is_empty() {
        out.push_str("  (none)\n");
    }
    for p in &d.wlan.profiles {
        let sec = match p.security {
            sim_core::device::WlanSecurity::Open => "open",
            sim_core::device::WlanSecurity::Wpa2Psk => "wpa2-psk",
            sim_core::device::WlanSecurity::Wpa3Sae => "wpa3-sae",
        };
        out.push_str(&format!(
            "  SSID: {:<12} Security: {:<10} VLAN: {}\n",
            p.ssid, sec, p.vlan
        ));
    }
    match d.kind {
        DeviceKind::WlanController => {
            out.push_str("AP list:\n");
            let aps = eng.wlan_aps_of(dev);
            if aps.is_empty() {
                out.push_str("  (none)\n");
            }
            for (name, ip, joined) in aps {
                out.push_str(&format!(
                    "  {:<10}{:<16}{}\n",
                    name,
                    ip.map(ipv4_str).unwrap_or_else(|| "-".into()),
                    if joined { "Joined" } else { "Join" }
                ));
            }
        }
        DeviceKind::AccessPoint => {
            out.push_str(&format!(
                "AC: {}\nState: {}\nPriority: {}\n",
                d.wlan.ac_ip.map(ipv4_str).unwrap_or_else(|| "-".into()),
                if d.wlan.joined { "Joined" } else { "Idle" },
                d.wlan.priority
            ));
        }
        _ => {}
    }
    out.trim_end().to_string()
}

/// `show ip ospf neighbor` / `display ospf peer`。
pub fn ospf_neighbors(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let rid = ipv4_str(eng.ospf_router_id_of(dev));
    let nbs = eng.ospf_neighbors_of(dev);
    let iface_for = |port: sim_core::device::PortId| ifname::display(dialect, d.kind, port);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!(
                "OSPF process 1, Router ID {}\n\n{:<16}{:<5}{:<16}{:<12}{:<16}{}\n",
                rid, "Neighbor ID", "Pri", "State", "Dead Time", "Address", "Interface"
            ));
            for (nrid, ip, port) in &nbs {
                out.push_str(&format!(
                    "{:<16}{:<5}{:<16}{:<12}{:<16}{}\n",
                    ipv4_str(*nrid),
                    "1",
                    "FULL/DR",
                    "00:00:04",
                    ipv4_str(*ip),
                    iface_for(*port)
                ));
            }
            if nbs.is_empty() {
                out.push_str("（无邻居）");
            }
        }
        _ => {
            out.push_str(&format!("OSPF Process 1 with Router ID {}\n", rid));
            out.push_str(" Neighbors\n\n");
            out.push_str(&format!(
                " {:<16}{:<18}{:<18}{}\n",
                "Router ID", "Address", "Interface", "State"
            ));
            for (nrid, ip, port) in &nbs {
                out.push_str(&format!(
                    " {:<16}{:<18}{:<18}{}\n",
                    ipv4_str(*nrid),
                    ipv4_str(*ip),
                    iface_for(*port),
                    "Full"
                ));
            }
            if nbs.is_empty() {
                out.push_str(" Info: No OSPF neighbor.\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// `show ip ospf database` / `display ospf lsdb`（Router-LSA 摘要）。
pub fn ospf_lsdb(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let rid = ipv4_str(eng.ospf_router_id_of(dev));
    let entries = eng.ospf_lsdb_of(dev);
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str(&format!("OSPF Router with ID ({})\n\n", rid));
            out.push_str(&format!("{:<18}{:<8}{}\n", "Link ID", "Links", "Neighbors"));
            for (r, links, nbs) in &entries {
                out.push_str(&format!("{:<18}{:<8}{}\n", ipv4_str(*r), links, nbs));
            }
        }
        _ => {
            out.push_str(&format!("OSPF LSDB (Router ID {})\n", rid));
            out.push_str(&format!(
                " {:<18}{:<8}{}\n",
                "Router ID", "Links", "Neighbors"
            ));
            for (r, links, nbs) in &entries {
                out.push_str(&format!(" {:<18}{:<8}{}\n", ipv4_str(*r), links, nbs));
            }
            if entries.is_empty() {
                out.push_str(" Info: The LSDB is empty.\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// `show etherchannel summary` / `display eth-trunk` / `display link-aggregation`。
pub fn lag_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Switch(sw) = &d.state else {
        return String::new();
    };
    let mut ids: Vec<u16> = sw.lags.keys().copied().collect();
    ids.sort_unstable();
    if ids.is_empty() {
        return match dialect {
            Dialect::Ios => String::new(),
            Dialect::Vrp => "Info: No Eth-Trunk exists.".into(),
            Dialect::Comware => "Info: No aggregation group exists.".into(),
        };
    }
    let mut out = String::new();
    match dialect {
        Dialect::Ios => {
            out.push_str("Flags:  D - down        P - bundled in port-channel\n");
            out.push_str("        U - in use      s - suspended\n\n");
            out.push_str(&format!("Number of channel-groups in use: {}\n", ids.len()));
            out.push_str("Group  Port-channel  Protocol    Ports\n");
            out.push_str(
                "------+-------------+-----------+-----------------------------------------------\n",
            );
            for id in ids {
                let Some(g) = sw.lags.get(&id) else { continue };
                let proto = match g.mode {
                    LagMode::Static => "Static",
                    _ => "LACP",
                };
                let flags = if g.selected.is_empty() { "SD" } else { "SU" };
                let ports = g
                    .members
                    .iter()
                    .map(|m| {
                        format!(
                            "{}({})",
                            ifname::short(dialect, d.kind, *m),
                            if g.selected.contains(m) { "P" } else { "D" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!(
                    "{:<6} {:<13} {:<11} {}\n",
                    id,
                    format!("{}({})", ifname::lag_short(dialect, id), flags),
                    proto,
                    ports
                ));
            }
        }
        Dialect::Vrp => {
            for id in ids {
                let Some(g) = sw.lags.get(&id) else { continue };
                out.push_str(&format!("Eth-Trunk{}'s state information is:\n", id));
                out.push_str(&format!(
                    "WorkingMode: {}\n",
                    if g.mode == LagMode::Static {
                        "NORMAL"
                    } else {
                        "LACP"
                    }
                ));
                out.push_str(&format!(
                    "Operate status: {}\n",
                    if g.selected.is_empty() { "down" } else { "up" }
                ));
                out.push_str(&format!(
                    "Number Of Up Port In Trunk: {}\n\n",
                    g.selected.len()
                ));
                out.push_str(&format!(
                    "{:<24}{:<10}{}\n",
                    "ActorPortName", "Status", "PortType"
                ));
                for m in &g.members {
                    out.push_str(&format!(
                        "{:<24}{:<10}{}\n",
                        ifname::canonical(dialect, d.kind, *m),
                        if g.selected.contains(m) { "Up" } else { "Down" },
                        "Ethernet"
                    ));
                }
                out.push('\n');
            }
        }
        Dialect::Comware => {
            for id in ids {
                let Some(g) = sw.lags.get(&id) else { continue };
                out.push_str(&format!(
                    "Aggregation Interface: {}\n",
                    ifname::lag_canonical(dialect, id)
                ));
                out.push_str(&format!(
                    "Aggregation Mode: {}\n",
                    if g.mode == LagMode::Static {
                        "Static"
                    } else {
                        "Dynamic"
                    }
                ));
                out.push_str("Loadsharing Type: Shar\n");
                out.push_str("Local:\n");
                out.push_str(&format!("  {:<16}{:<8}{}\n", "Port", "Status", "LAG-ID"));
                for m in &g.members {
                    out.push_str(&format!(
                        "  {:<16}{:<8}{}\n",
                        ifname::display(dialect, d.kind, *m),
                        if g.selected.contains(m) { "S" } else { "U" },
                        id
                    ));
                }
                out.push('\n');
            }
        }
    }
    out.trim_end().to_string()
}

/// MSTP：按实例展示根桥与端口角色（0 = CIST）。
fn mst_table(eng: &Engine, dev: DevId, dialect: Dialect) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Switch(sw) = &d.state else {
        return String::new();
    };
    let mac_fmt = |m: &Mac| match dialect {
        Dialect::Ios => sim_packets::mac_str_cisco(m),
        _ => sim_packets::mac_str_huawei(m),
    };
    let mut out = String::new();
    out.push_str(if dialect == Dialect::Ios {
        "MSTP configuration\n"
    } else {
        " Mode: MSTP\n"
    });
    let mut ids: Vec<u16> = sw.stp.mst.keys().copied().collect();
    ids.sort_unstable();
    let mut render =
        |inst: u16,
         vlans: &[VlanId],
         root: sim_packets::BridgeId,
         cost: u32,
         root_port: Option<PortId>,
         ports: &std::collections::HashMap<PortId, sim_core::device::StpPortSt>| {
            let label = if vlans.is_empty() {
                "CIST".to_string()
            } else {
                format!(
                    "VLAN {}",
                    vlans
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            out.push_str(&format!(
                "Instance {} ({})  Root {}.{}  Cost {}  RootPort {}\n",
                inst,
                label,
                root.priority,
                mac_fmt(&root.mac),
                cost,
                root_port
                    .map(|p| ifname::display(dialect, d.kind, p))
                    .unwrap_or_else(|| "-".into())
            ));
            for p in &d.ports {
                if p.lag.is_some() && eng.logical_port(dev, p.idx) != p.idx {
                    continue;
                }
                let Some(st) = ports.get(&p.idx) else {
                    continue;
                };
                if st.role == StpRole::Disabled && p.link.is_none() {
                    continue;
                }
                let name = match p.lag {
                    Some(gid) => ifname::lag_canonical(dialect, gid),
                    None => ifname::display(dialect, d.kind, p.idx),
                };
                out.push_str(&format!(
                    "  {:<24}{:<6}{}\n",
                    name,
                    stp_role_str(dialect, st.role),
                    stp_state_str(dialect, st.state)
                ));
            }
        };
    let cist_ports = sw.stp.ports.clone();
    render(
        0,
        &[],
        sw.stp.root,
        sw.stp.root_cost,
        sw.stp.root_port,
        &cist_ports,
    );
    for id in ids {
        let Some(m) = sw.stp.mst.get(&id) else {
            continue;
        };
        render(
            id,
            &m.vlans,
            m.data.root,
            m.data.root_cost,
            m.data.root_port,
            &m.data.ports,
        );
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
