//! IPv6：三方言地址/静态路由/ping6 与 show。

use cli_dialects::{exec_and_run, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::{ipv4_from_str, ipv6_from_str};

fn lab(dialect: Dialect) -> (Engine, u32, Session) {
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, dialect);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, dialect);
    net.add_link(r1, r2).unwrap();
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ipv4_from_str("10.0.0.1").unwrap()), 24);
    net.set_port_admin(r2, 0, true);
    net.set_port_ip(r2, 0, Some(ipv4_from_str("10.0.0.2").unwrap()), 24);
    (Engine::new(net), r1, Session::default())
}

#[test]
fn ios_ipv6_config_and_show() {
    let (mut eng, r1, mut s) = lab(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "interface g0/0",
        "ipv6 address 2001:db8:12::1/64",
        "exit",
        "ipv6 route 2001:db8:2::/64 2001:db8:12::2",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    exec_and_run(&mut eng, &mut s, r1, "end");
    let brief = exec_and_run(&mut eng, &mut s, r1, "show ipv6 interface brief");
    assert!(brief.contains("2001:db8:12::1/64"), "{}", brief);
    let route = exec_and_run(&mut eng, &mut s, r1, "show ipv6 route");
    assert!(route.contains("2001:db8:2::/64"), "{}", route);
    assert!(route.contains("S "), "{}", route);
    let ping = exec_and_run(&mut eng, &mut s, r1, "ping ipv6 2001:db8:2::2");
    assert!(ping.contains("ICMPv6"), "{}", ping);
}

#[test]
fn vrp_ipv6_config_display() {
    let (mut eng, r1, mut s) = lab(Dialect::Vrp);
    for line in [
        "system-view",
        "interface g0/0/0",
        "ipv6 address 2001:db8:12::1/64",
        "quit",
        "ipv6 route-static 2001:db8:2:: 64 2001:db8:12::2",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    let brief = exec_and_run(&mut eng, &mut s, r1, "display ipv6 interface brief");
    assert!(brief.contains("2001:db8:12::1/64"), "{}", brief);
    let route = exec_and_run(&mut eng, &mut s, r1, "display ipv6 routing-table");
    assert!(route.contains("2001:db8:2::/64"), "{}", route);
    let ping = exec_and_run(&mut eng, &mut s, r1, "ping ipv6 2001:db8:2::2");
    assert!(ping.contains("PING6"), "{}", ping);
}

#[test]
fn ipv6_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::If, "ipv6 address 2001:db8::1/64"),
        (
            Dialect::Ios,
            Mode::Config,
            "ipv6 route 2001:db8:2::/64 2001:db8:1::2",
        ),
        (Dialect::Ios, Mode::Exec, "ping ipv6 2001:db8::2"),
        (Dialect::Ios, Mode::Exec, "show ipv6 interface brief"),
        (Dialect::Ios, Mode::Exec, "show ipv6 route"),
        (Dialect::Vrp, Mode::If, "ipv6 address 2001:db8::1/64"),
        (
            Dialect::Vrp,
            Mode::Config,
            "ipv6 route-static 2001:db8:2:: 64 2001:db8:1::2",
        ),
        (Dialect::Vrp, Mode::Exec, "display ipv6 interface brief"),
        (Dialect::Vrp, Mode::Exec, "display ipv6 routing-table"),
        (Dialect::Comware, Mode::If, "ipv6 address 2001:db8::1/64"),
        (
            Dialect::Comware,
            Mode::Config,
            "ipv6 route-static 2001:db8:2:: 64 2001:db8:1::2",
        ),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
    // 地址工具与引擎地址一致
    assert!(ipv6_from_str("2001:db8::1").is_some());
}
