//! VRF：三方言配置与按 VRF 路由表。

use cli_dialects::{exec_and_run, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn router(dialect: Dialect) -> (Engine, u32, Session) {
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, dialect);
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ipv4_from_str("10.0.0.1").unwrap()), 24);
    (Engine::new(net), r1, Session::default())
}

#[test]
fn ios_vrf_config_and_show() {
    let (mut eng, r1, mut s) = router(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "vrf definition CUST1",
        "interface g0/0",
        "ip vrf forwarding CUST1",
        "exit",
        "ip route vrf CUST1 10.9.0.0 255.255.255.0 10.0.0.2",
        "end",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    let show = exec_and_run(&mut eng, &mut s, r1, "show ip route vrf CUST1");
    assert!(show.contains("10.0.0.0/24"), "{}", show);
    assert!(show.contains("10.9.0.0/24"), "{}", show);
}

#[test]
fn vrp_vrf_config_and_display() {
    let (mut eng, r1, mut s) = router(Dialect::Vrp);
    for line in [
        "system-view",
        "ip vpn-instance CUST1",
        "interface g0/0/0",
        "ip binding vpn-instance CUST1",
        "quit",
        "ip route-static vpn-instance CUST1 10.9.0.0 24 10.0.0.2",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    let show = exec_and_run(
        &mut eng,
        &mut s,
        r1,
        "display ip routing-table vpn-instance CUST1",
    );
    assert!(show.contains("10.0.0.0/24"), "{}", show);
    assert!(show.contains("10.9.0.0/24"), "{}", show);
}

#[test]
fn vrf_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "vrf definition CUST1"),
        (Dialect::Ios, Mode::If, "ip vrf forwarding CUST1"),
        (
            Dialect::Ios,
            Mode::Config,
            "ip route vrf CUST1 10.9.0.0 255.255.255.0 10.0.0.2",
        ),
        (Dialect::Ios, Mode::Exec, "show ip route vrf CUST1"),
        (Dialect::Vrp, Mode::Config, "ip vpn-instance CUST1"),
        (Dialect::Vrp, Mode::If, "ip binding vpn-instance CUST1"),
        (
            Dialect::Vrp,
            Mode::Config,
            "ip route-static vpn-instance CUST1 10.9.0.0 24 10.0.0.2",
        ),
        (
            Dialect::Vrp,
            Mode::Exec,
            "display ip routing-table vpn-instance CUST1",
        ),
        (Dialect::Comware, Mode::Config, "ip vpn-instance CUST1"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
