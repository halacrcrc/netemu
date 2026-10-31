//! IS-IS：三方言配置、邻居与 LSDB 展示。

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
fn ios_isis_config_and_show() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "router isis",
        "net 49.0001.0000.0000.0001.00",
        "exit",
        "interface g0/0",
        "ip router isis",
        "end",
    ] {
        let out = exec_and_run(&mut eng, &mut s1, r1, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    for line in [
        "enable",
        "configure terminal",
        "router isis",
        "net 49.0001.0000.0000.0002.00",
        "exit",
        "interface g0/0",
        "ip router isis",
        "end",
    ] {
        exec_and_run(&mut eng, &mut s2, r2, line);
    }
    for _ in 0..3 {
        let h = eng.drain_horizon();
        eng.run_to_quiescence(h);
    }
    let nb = exec_and_run(&mut eng, &mut s1, r1, "show isis neighbors");
    assert!(
        nb.contains("000000000002"),
        "nb={} sys1={:?} sys2={:?} lsdb={:?}",
        nb,
        eng.isis_system_of(r1),
        eng.isis_system_of(r2),
        eng.isis_lsdb_of(r1)
    );
    let db = exec_and_run(&mut eng, &mut s1, r1, "show isis database");
    assert!(db.contains("000000000002"), "{}", db);
}

#[test]
fn vrp_isis_config_display() {
    let (mut eng, r1, r2, mut s1, mut s2) = two_routers(Dialect::Vrp);
    for line in [
        "system-view",
        "isis 1",
        "network-entity 49.0001.0000.0000.0001.00",
        "quit",
        "interface g0/0/0",
        "isis enable",
        "quit",
    ] {
        let out = exec_and_run(&mut eng, &mut s1, r1, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    for line in [
        "system-view",
        "isis 1",
        "network-entity 49.0001.0000.0000.0002.00",
        "quit",
        "interface g0/0/0",
        "isis enable",
        "quit",
    ] {
        exec_and_run(&mut eng, &mut s2, r2, line);
    }
    exec_and_run(&mut eng, &mut s1, r1, "system-view");
    exec_and_run(&mut eng, &mut s1, r1, "isis 1");
    assert_eq!(prompt(&eng, &s1, r1), "[R1-isis-1]");
    exec_and_run(&mut eng, &mut s1, r1, "quit");
    for _ in 0..3 {
        let h = eng.drain_horizon();
        eng.run_to_quiescence(h);
    }
    let peer = exec_and_run(&mut eng, &mut s1, r1, "display isis peer");
    assert!(peer.contains("000000000002"), "{}", peer);
    let db = exec_and_run(&mut eng, &mut s1, r1, "display isis lsdb");
    assert!(db.contains("000000000002"), "{}", db);
}

#[test]
fn isis_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "router isis"),
        (Dialect::Ios, Mode::Isis, "net 49.0001.0000.0000.0001.00"),
        (Dialect::Ios, Mode::If, "ip router isis"),
        (Dialect::Ios, Mode::Exec, "show isis neighbors"),
        (Dialect::Ios, Mode::Exec, "show isis database"),
        (Dialect::Vrp, Mode::Config, "isis 1"),
        (
            Dialect::Vrp,
            Mode::Isis,
            "network-entity 49.0001.0000.0000.0001.00",
        ),
        (Dialect::Vrp, Mode::If, "isis enable"),
        (Dialect::Vrp, Mode::Exec, "display isis peer"),
        (Dialect::Vrp, Mode::Exec, "display isis lsdb"),
        (Dialect::Comware, Mode::Config, "isis 1"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
