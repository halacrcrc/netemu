//! OSPF：三方言配置、邻居与 LSDB 展示。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn two_routers(dialect: Dialect) -> (Engine, u32, u32, Session, Session) {
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, dialect);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, dialect);
    net.add_link(r1, r2).unwrap();
    let (a, b) = match dialect {
        Dialect::Ios => (("10.0.0.1", 0usize), ("10.0.0.2", 0usize)),
        _ => (("10.0.0.1", 0usize), ("10.0.0.2", 0usize)),
    };
    net.set_port_admin(r1, a.1, true);
    net.set_port_ip(r1, a.1, Some(ipv4_from_str(a.0).unwrap()), 24);
    net.set_port_admin(r2, b.1, true);
    net.set_port_ip(r2, b.1, Some(ipv4_from_str(b.0).unwrap()), 24);
    (
        Engine::new(net),
        r1,
        r2,
        Session::default(),
        Session::default(),
    )
}

#[test]
fn ios_ospf_config_show() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Ios);
    run(
        &mut eng,
        &mut s1,
        r1,
        &["enable", "configure terminal", "router ospf 1"],
    );
    assert_eq!(prompt(&eng, &s1, r1), "R1(config-router)#");
    let out = exec_and_run(&mut eng, &mut s1, r1, "network 10.0.0.0 0.0.0.255 area 0");
    assert_eq!(out, "");
    exec_and_run(&mut eng, &mut s1, r1, "end");
    run(
        &mut eng,
        &mut s2,
        r2,
        &[
            "enable",
            "configure terminal",
            "router ospf 1",
            "network 10.0.0.0 0.0.0.255 area 0",
            "end",
        ],
    );
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
    let show = exec_and_run(&mut eng, &mut s1, r1, "show ip ospf neighbor");
    assert!(show.contains("FULL"), "{}", show);
    assert!(show.contains("10.0.0.2"), "{}", show);
    let db = exec_and_run(&mut eng, &mut s1, r1, "show ip ospf database");
    assert!(db.contains("OSPF Router with ID"), "{}", db);
}

#[test]
fn vrp_ospf_config_display() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Vrp);
    run(&mut eng, &mut s1, r1, &["system-view", "ospf 1", "area 0"]);
    assert_eq!(prompt(&eng, &s1, r1), "[R1-ospf-1-area-0.0.0.0]");
    let out = exec_and_run(&mut eng, &mut s1, r1, "network 10.0.0.0 0.0.0.255");
    assert_eq!(out, "");
    exec_and_run(&mut eng, &mut s1, r1, "quit");
    run(
        &mut eng,
        &mut s2,
        r2,
        &[
            "system-view",
            "ospf 1",
            "area 0",
            "network 10.0.0.0 0.0.0.255",
            "quit",
        ],
    );
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
    let show = exec_and_run(&mut eng, &mut s1, r1, "display ospf peer");
    assert!(
        show.contains("10.0.0.2") && show.contains("Full"),
        "{}",
        show
    );
    let db = exec_and_run(&mut eng, &mut s1, r1, "display ospf lsdb");
    assert!(db.contains("OSPF LSDB"), "{}", db);
}

fn run(eng: &mut Engine, sess: &mut Session, dev: u32, lines: &[&str]) {
    for l in lines {
        let out = exec_and_run(eng, sess, dev, l);
        assert!(
            !out.contains("% ") && !out.contains("Error:"),
            "命令 {:?} 意外报错: {}",
            l,
            out
        );
    }
}

#[test]
fn ospf_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "router ospf 1"),
        (
            Dialect::Ios,
            Mode::Ospf,
            "network 10.0.0.0 0.0.0.255 area 0",
        ),
        (Dialect::Ios, Mode::Exec, "show ip ospf neighbor"),
        (Dialect::Ios, Mode::Exec, "show ip ospf database"),
        (Dialect::Vrp, Mode::Config, "ospf 1"),
        (Dialect::Vrp, Mode::Ospf, "area 0"),
        (Dialect::Vrp, Mode::OspfArea, "network 10.0.0.0 0.0.0.255"),
        (Dialect::Vrp, Mode::Exec, "display ospf peer"),
        (Dialect::Vrp, Mode::Exec, "display ospf lsdb"),
        (Dialect::Comware, Mode::Config, "ospf 1"),
        (
            Dialect::Comware,
            Mode::OspfArea,
            "network 192.168.1.0 0.0.0.255",
        ),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}

#[test]
fn ios_route_table_includes_ospf_code() {
    use sim_core::device::{Route, RouteSrc};
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Ios);
    {
        let d = eng.net.device_mut(r1).unwrap();
        if let DeviceState::Router(rs) = &mut d.state {
            rs.ospf_routes.push(Route {
                dst: ipv4_from_str("192.168.9.0").unwrap(),
                len: 24,
                nexthop: ipv4_from_str("10.0.0.2").unwrap(),
                vlan: None,
                port: 0,
                src: RouteSrc::Ospf,
            });
        }
    }
    let show = exec_and_run(&mut eng, &mut s1, r1, "show ip route");
    assert!(show.contains("O    192.168.9.0/24"), "{}", show);
    let _ = (&mut s2, r2);
}
