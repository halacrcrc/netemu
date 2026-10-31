//! VRRP：三方言配置与状态展示。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn router(dialect: Dialect) -> (Engine, u32, Session) {
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, dialect);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, dialect);
    net.add_link(r1, r2).unwrap();
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ipv4_from_str("192.168.1.1").unwrap()), 24);
    (Engine::new(net), r1, Session::default())
}

#[test]
fn ios_vrrp_config_and_show() {
    let (mut eng, r1, mut s) = router(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "interface g0/0",
        "vrrp 1 ip 192.168.1.254",
        "vrrp 1 priority 120",
        "vrrp 1 preempt",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    assert_eq!(prompt(&eng, &s, r1), "R1(config-if)#");
    exec_and_run(&mut eng, &mut s, r1, "end");
    let show = exec_and_run(&mut eng, &mut s, r1, "show vrrp");
    assert!(show.contains("192.168.1.254"), "{}", show);
    assert!(show.contains("Master"), "{}", show);
}

#[test]
fn vrp_vrrp_config_and_display() {
    let (mut eng, r1, mut s) = router(Dialect::Vrp);
    for line in [
        "system-view",
        "interface g0/0/0",
        "vrrp vrid 1 virtual-ip 192.168.1.254",
        "vrrp vrid 1 priority 120",
        "vrrp vrid 1 preempt-mode",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    let show = exec_and_run(&mut eng, &mut s, r1, "display vrrp");
    assert!(show.contains("192.168.1.254"), "{}", show);
    assert!(show.contains("Master"), "{}", show);
}

#[test]
fn vrrp_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::If, "vrrp 1 ip 192.168.1.254"),
        (Dialect::Ios, Mode::If, "vrrp 1 priority 120"),
        (Dialect::Ios, Mode::If, "vrrp 1 preempt"),
        (Dialect::Ios, Mode::Exec, "show vrrp"),
        (
            Dialect::Vrp,
            Mode::If,
            "vrrp vrid 1 virtual-ip 192.168.1.254",
        ),
        (Dialect::Vrp, Mode::If, "vrrp vrid 1 priority 120"),
        (Dialect::Vrp, Mode::If, "vrrp vrid 1 preempt-mode"),
        (Dialect::Vrp, Mode::Exec, "display vrrp"),
        (
            Dialect::Comware,
            Mode::If,
            "vrrp vrid 1 virtual-ip 192.168.1.254",
        ),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
