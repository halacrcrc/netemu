//! route-policy：三方言配置、BGP import/export 过滤与 local-preference 选路。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{
    BgpConfig, BgpPeerPolicy, DeviceKind, DeviceState, Dialect, RoutePolicy, RoutePolicyNode,
};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn two_routers(dialect: Dialect) -> (Engine, u32, u32, Session, Session) {
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, dialect);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, dialect);
    net.add_link(r1, r2).unwrap();
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ipv4_from_str("10.0.0.1").unwrap()), 24);
    net.set_port_admin(r2, 0, true);
    net.set_port_ip(r2, 0, Some(ipv4_from_str("10.0.0.2").unwrap()), 24);
    (
        Engine::new(net),
        r1,
        r2,
        Session::default(),
        Session::default(),
    )
}

fn converge(eng: &mut Engine) {
    for _ in 0..4 {
        let h = eng.drain_horizon();
        eng.run_to_quiescence(h);
    }
}

#[test]
fn ios_route_map_filters_bgp_import() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "router bgp 65001",
        "neighbor 10.0.0.2 remote-as 65002",
        "network 10.1.0.0 mask 255.255.255.0",
        "network 10.2.0.0 mask 255.255.255.0",
        "end",
    ] {
        exec_and_run(&mut eng, &mut s1, r1, line);
    }
    for line in [
        "enable",
        "configure terminal",
        "access-list 100 permit 10.2.0.0 0.0.0.255 0.0.0.0 255.255.255.255",
        "route-map FILTER deny 10",
        "match ip address 100",
        "exit",
        "route-map FILTER permit 20",
        "exit",
        "router bgp 65002",
        "neighbor 10.0.0.1 remote-as 65001",
        "neighbor 10.0.0.1 route-map FILTER in",
        "end",
    ] {
        let out = exec_and_run(&mut eng, &mut s2, r2, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    converge(&mut eng);

    let table = exec_and_run(&mut eng, &mut s2, r2, "show ip bgp");
    assert!(table.contains("10.1.0.0/24"), "{}", table);
    assert!(!table.contains("10.2.0.0/24"), "{}", table);
    assert!(table.contains("LocPrf"), "{}", table);

    let shown = exec_and_run(&mut eng, &mut s2, r2, "show route-map");
    assert!(shown.contains("route-map FILTER deny 10"), "{}", shown);
    assert!(shown.contains("ip address 100"), "{}", shown);
    assert!(shown.contains("route-map FILTER permit 20"), "{}", shown);
}

#[test]
fn vrp_route_policy_filters_bgp_import() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Vrp);
    for line in [
        "system-view",
        "bgp 65001",
        "peer 10.0.0.2 as-number 65002",
        "network 10.1.0.0 255.255.255.0",
        "network 10.2.0.0 255.255.255.0",
        "quit",
    ] {
        exec_and_run(&mut eng, &mut s1, r1, line);
    }
    for line in [
        "system-view",
        "acl 2000",
        "rule permit source 10.2.0.0 0.0.0.255",
        "quit",
        "route-policy FILTER deny node 10",
        "if-match acl 2000",
        "quit",
        "route-policy FILTER permit node 20",
        "quit",
        "bgp 65002",
        "peer 10.0.0.1 as-number 65001",
        "peer 10.0.0.1 route-policy FILTER import",
        "quit",
    ] {
        let out = exec_and_run(&mut eng, &mut s2, r2, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    assert_eq!(prompt(&eng, &s2, r2), "[R2]");
    converge(&mut eng);

    let table = exec_and_run(&mut eng, &mut s2, r2, "display bgp routing-table");
    assert!(table.contains("10.1.0.0/24"), "{}", table);
    assert!(!table.contains("10.2.0.0/24"), "{}", table);

    let shown = exec_and_run(&mut eng, &mut s2, r2, "display route-policy");
    assert!(shown.contains("Route-policy: FILTER"), "{}", shown);
    assert!(shown.contains("node 10 deny"), "{}", shown);
    assert!(shown.contains("if-match acl 2000"), "{}", shown);
    assert!(shown.contains("node 20 permit"), "{}", shown);
}

#[test]
fn ios_route_map_filters_bgp_export() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "access-list 100 permit 10.2.0.0 0.0.0.255 0.0.0.0 255.255.255.255",
        "route-map OUT deny 10",
        "match ip address 100",
        "exit",
        "route-map OUT permit 20",
        "exit",
        "router bgp 65001",
        "neighbor 10.0.0.2 remote-as 65002",
        "neighbor 10.0.0.2 route-map OUT out",
        "network 10.1.0.0 mask 255.255.255.0",
        "network 10.2.0.0 mask 255.255.255.0",
        "end",
    ] {
        let out = exec_and_run(&mut eng, &mut s1, r1, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    for line in [
        "enable",
        "configure terminal",
        "router bgp 65002",
        "neighbor 10.0.0.1 remote-as 65001",
        "end",
    ] {
        exec_and_run(&mut eng, &mut s2, r2, line);
    }
    converge(&mut eng);

    let table = exec_and_run(&mut eng, &mut s2, r2, "show ip bgp");
    assert!(table.contains("10.1.0.0/24"), "{}", table);
    assert!(!table.contains("10.2.0.0/24"), "{}", table);
}

#[test]
fn local_preference_wins_best_path() {
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Vrp);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Vrp);
    let r3 = net.add_device("R3".into(), DeviceKind::Router, Dialect::Vrp);
    net.add_link(r1, r2).unwrap();
    net.add_link(r2, r3).unwrap();
    for (dev, port, ip) in [
        (r1, 0, "10.0.0.1"),
        (r2, 0, "10.0.0.2"),
        (r2, 1, "10.0.1.2"),
        (r3, 0, "10.0.1.1"),
    ] {
        net.set_port_admin(dev, port, true);
        net.set_port_ip(dev, port, Some(ipv4_from_str(ip).unwrap()), 24);
    }
    let ip = |s: &str| ipv4_from_str(s).unwrap();
    for (dev, asn, neighbors) in [
        (r1, 65001, vec![(ip("10.0.0.2"), 65002)]),
        (
            r2,
            65002,
            vec![(ip("10.0.0.1"), 65001), (ip("10.0.1.1"), 65003)],
        ),
        (r3, 65003, vec![(ip("10.0.1.2"), 65002)]),
    ] {
        if let DeviceState::Router(rs) = &mut net.device_mut(dev).unwrap().state {
            rs.bgp = Some(BgpConfig {
                asn,
                neighbors,
                networks: if dev == r2 {
                    vec![]
                } else {
                    vec![(ip("10.9.0.0"), 24)]
                },
                peer_policies: vec![],
            });
        }
    }
    if let DeviceState::Router(rs) = &mut net.device_mut(r2).unwrap().state {
        rs.route_policies.push(RoutePolicy {
            name: "PREF".into(),
            nodes: vec![RoutePolicyNode {
                seq: 10,
                permit: true,
                match_acl: None,
                set_local_pref: Some(200),
            }],
        });
        rs.bgp.as_mut().unwrap().peer_policies.push(BgpPeerPolicy {
            neighbor: ip("10.0.1.1"),
            import: true,
            name: "PREF".into(),
        });
    }
    let mut eng = Engine::new(net);
    for dev in [r1, r2, r3] {
        eng.bgp_config_changed(dev);
    }
    converge(&mut eng);

    let routes = eng.bgp_routes_of(r2);
    assert_eq!(routes.len(), 1, "{:?}", routes);
    let (net, len, _, nexthop, local_pref) = &routes[0];
    assert_eq!((*net, *len), (ip("10.9.0.0"), 24));
    assert_eq!(*nexthop, ip("10.0.1.1"), "应优选 local-pref=200 的 R3 路径");
    assert_eq!(*local_pref, 200);
}

#[test]
fn route_policy_survives_summary_reload() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Vrp);
    for line in [
        "system-view",
        "bgp 65001",
        "peer 10.0.0.2 as-number 65002",
        "network 10.1.0.0 255.255.255.0",
        "network 10.2.0.0 255.255.255.0",
        "quit",
    ] {
        exec_and_run(&mut eng, &mut s1, r1, line);
    }
    for line in [
        "system-view",
        "acl 2000",
        "rule permit source 10.2.0.0 0.0.0.255",
        "quit",
        "route-policy FILTER deny node 10",
        "if-match acl 2000",
        "apply local-preference 200",
        "quit",
        "route-policy FILTER permit node 20",
        "quit",
        "bgp 65002",
        "peer 10.0.0.1 as-number 65001",
        "peer 10.0.0.1 route-policy FILTER import",
        "quit",
    ] {
        exec_and_run(&mut eng, &mut s2, r2, line);
    }
    let sum = eng.net.summary();
    let mut net2 = Net::new();
    net2.apply_summary(&sum).unwrap();
    let mut eng2 = Engine::new(net2);
    eng2.after_load();
    converge(&mut eng2);

    let mut s = Session::default();
    let policy = exec_and_run(&mut eng2, &mut s, r2, "display route-policy");
    assert!(policy.contains("node 10 deny"), "{}", policy);
    assert!(policy.contains("apply local-preference 200"), "{}", policy);
    let table = exec_and_run(&mut eng2, &mut s, r2, "display bgp routing-table");
    assert!(table.contains("10.1.0.0/24"), "{}", table);
    assert!(!table.contains("10.2.0.0/24"), "{}", table);
}

#[test]
fn lab7_route_policy_end_to_end() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Vrp);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.add_link(r1, r2).unwrap();
    net.add_link(r2, pc2).unwrap();
    net.add_link(r1, srv).unwrap();
    net.set_host(
        pc1,
        Some(ipv4_from_str("192.168.1.10").unwrap()),
        24,
        Some(ipv4_from_str("192.168.1.1").unwrap()),
    );
    net.set_host(
        pc2,
        Some(ipv4_from_str("192.168.2.10").unwrap()),
        24,
        Some(ipv4_from_str("192.168.2.1").unwrap()),
    );
    net.set_host(
        srv,
        Some(ipv4_from_str("172.16.9.10").unwrap()),
        24,
        Some(ipv4_from_str("172.16.9.1").unwrap()),
    );
    let mut eng = Engine::new(net);
    let mut s1 = Session::default();
    let mut s2 = Session::default();
    for line in [
        "system-view",
        "interface g0/0/0",
        "ip address 192.168.1.1 24",
        "undo shutdown",
        "quit",
        "interface g0/0/1",
        "ip address 10.0.0.1 24",
        "undo shutdown",
        "quit",
        "interface g0/0/2",
        "ip address 172.16.9.1 24",
        "undo shutdown",
        "quit",
        "bgp 65001",
        "peer 10.0.0.2 as-number 65002",
        "network 192.168.1.0 255.255.255.0",
        "network 172.16.9.0 255.255.255.0",
        "quit",
    ] {
        let out = exec_and_run(&mut eng, &mut s1, r1, line);
        assert!(!out.contains("Error:"), "R1 {}: {}", line, out);
    }
    for line in [
        "system-view",
        "interface g0/0/0",
        "ip address 10.0.0.2 24",
        "undo shutdown",
        "quit",
        "interface g0/0/1",
        "ip address 192.168.2.1 24",
        "undo shutdown",
        "quit",
        "acl 2000",
        "rule permit source 172.16.9.0 0.0.0.255",
        "quit",
        "route-policy FILTER deny node 10",
        "if-match acl 2000",
        "quit",
        "route-policy FILTER permit node 20",
        "quit",
        "bgp 65002",
        "peer 10.0.0.1 as-number 65001",
        "network 192.168.2.0 255.255.255.0",
        "peer 10.0.0.1 route-policy FILTER import",
        "quit",
    ] {
        let out = exec_and_run(&mut eng, &mut s2, r2, line);
        assert!(!out.contains("Error:"), "R2 {}: {}", line, out);
    }
    converge(&mut eng);

    let r2_routes = eng.bgp_routes_of(r2);
    assert!(
        r2_routes
            .iter()
            .any(|r| r.0 == ipv4_from_str("192.168.1.0").unwrap()),
        "{:?}",
        r2_routes
    );
    assert!(
        r2_routes
            .iter()
            .all(|r| r.0 != ipv4_from_str("172.16.9.0").unwrap()),
        "{:?}",
        r2_routes
    );

    let ping_ok = {
        eng.start_ping(pc2, ipv4_from_str("192.168.1.10").unwrap());
        eng.run_to_quiescence(sim_core::engine::PING_HORIZON_NS);
        last_ping_received(&eng, pc2)
    };
    assert_eq!(ping_ok, 5, "PC2 应能 ping 通 PC1");
    let ping_denied = {
        eng.start_ping(pc2, ipv4_from_str("172.16.9.10").unwrap());
        eng.run_to_quiescence(sim_core::engine::PING_HORIZON_NS);
        last_ping_received(&eng, pc2)
    };
    assert_eq!(ping_denied, 0, "PC2 不应能访问被过滤的服务器");
}

fn last_ping_received(eng: &Engine, dev: u32) -> u32 {
    let d = eng.net.device(dev).unwrap();
    match &d.state {
        DeviceState::Host(h) => h.pings.last().map(|p| p.received()).unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn route_policy_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "route-map FILTER permit 10"),
        (Dialect::Ios, Mode::RoutePolicy, "match ip address 10"),
        (Dialect::Ios, Mode::RoutePolicy, "set local-preference 200"),
        (
            Dialect::Ios,
            Mode::Bgp,
            "neighbor 10.0.0.1 route-map FILTER in",
        ),
        (Dialect::Ios, Mode::Exec, "show route-map"),
        (
            Dialect::Vrp,
            Mode::Config,
            "route-policy FILTER deny node 10",
        ),
        (Dialect::Vrp, Mode::RoutePolicy, "if-match acl 2000"),
        (
            Dialect::Vrp,
            Mode::RoutePolicy,
            "apply local-preference 200",
        ),
        (
            Dialect::Vrp,
            Mode::Bgp,
            "peer 10.0.0.1 route-policy FILTER import",
        ),
        (Dialect::Vrp, Mode::Exec, "display route-policy"),
        (
            Dialect::Comware,
            Mode::Config,
            "route-policy FILTER permit node 20",
        ),
        (
            Dialect::Comware,
            Mode::Bgp,
            "peer 10.0.0.1 route-policy FILTER export",
        ),
    ];
    for (d, m, line) in cases {
        let r = parse(trees(d).root_for(m), line);
        let detail = match &r {
            Ok(Some(_)) => String::new(),
            Ok(None) => "Ok(None)".into(),
            Err(e) => format!("{:?}", e),
        };
        assert!(
            detail.is_empty(),
            "{:?} {:?} '{}' 应可解析: {}",
            d,
            m,
            line,
            detail
        );
    }
}
