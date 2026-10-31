//! BGP：三方言配置、邻居与路由表展示。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, Dialect};
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

#[test]
fn ios_bgp_config_and_show() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "router bgp 65001",
        "neighbor 10.0.0.2 remote-as 65002",
        "network 10.0.0.0 mask 255.255.255.0",
    ] {
        let out = exec_and_run(&mut eng, &mut s1, r1, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    assert_eq!(prompt(&eng, &s1, r1), "R1(config-router)#");
    exec_and_run(&mut eng, &mut s1, r1, "end");
    for line in [
        "enable",
        "configure terminal",
        "router bgp 65002",
        "neighbor 10.0.0.1 remote-as 65001",
        "network 10.0.0.0 mask 255.255.255.0",
        "end",
    ] {
        exec_and_run(&mut eng, &mut s2, r2, line);
    }
    for _ in 0..3 {
        let h = eng.drain_horizon();
        eng.run_to_quiescence(h);
    }
    let sum = exec_and_run(&mut eng, &mut s1, r1, "show ip bgp summary");
    assert!(sum.contains("local AS number 65001"), "{}", sum);
    assert!(sum.contains("Established"), "{}", sum);
    let table = exec_and_run(&mut eng, &mut s1, r1, "show ip bgp");
    assert!(table.contains("10.0.0.0/24"), "{}", table);
}

#[test]
fn vrp_bgp_config_display() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Vrp);
    for line in [
        "system-view",
        "bgp 65001",
        "peer 10.0.0.2 as-number 65002",
        "network 10.0.0.0 255.255.255.0",
    ] {
        let out = exec_and_run(&mut eng, &mut s1, r1, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    assert_eq!(prompt(&eng, &s1, r1), "[R1-bgp-65001]");
    exec_and_run(&mut eng, &mut s1, r1, "quit");
    for line in [
        "system-view",
        "bgp 65002",
        "peer 10.0.0.1 as-number 65001",
        "network 10.0.0.0 255.255.255.0",
        "quit",
    ] {
        exec_and_run(&mut eng, &mut s2, r2, line);
    }
    for _ in 0..3 {
        let h = eng.drain_horizon();
        eng.run_to_quiescence(h);
    }
    let peer = exec_and_run(&mut eng, &mut s1, r1, "display bgp peer");
    assert!(
        peer.contains("65002") && peer.contains("Established"),
        "{}",
        peer
    );
    let table = exec_and_run(&mut eng, &mut s1, r1, "display bgp routing-table");
    assert!(table.contains("10.0.0.0/24"), "{}", table);
}

#[test]
fn bgp_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "router bgp 65001"),
        (Dialect::Ios, Mode::Bgp, "neighbor 10.0.0.2 remote-as 65002"),
        (
            Dialect::Ios,
            Mode::Bgp,
            "network 192.168.1.0 mask 255.255.255.0",
        ),
        (Dialect::Ios, Mode::Exec, "show ip bgp summary"),
        (Dialect::Ios, Mode::Exec, "show ip bgp"),
        (Dialect::Vrp, Mode::Config, "bgp 65001"),
        (Dialect::Vrp, Mode::Bgp, "peer 10.0.0.2 as-number 65002"),
        (Dialect::Vrp, Mode::Bgp, "network 192.168.1.0 255.255.255.0"),
        (Dialect::Vrp, Mode::Exec, "display bgp peer"),
        (Dialect::Vrp, Mode::Exec, "display bgp routing-table"),
        (Dialect::Comware, Mode::Config, "bgp 65001"),
        (Dialect::Comware, Mode::Bgp, "peer 10.0.0.2 as-number 65002"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
