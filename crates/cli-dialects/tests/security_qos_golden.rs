//! DHCP Snooping / 端口安全 / QoS：三方言配置与展示。

use cli_dialects::{exec_and_run, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;

fn switch(dialect: Dialect) -> (Engine, u32, Session) {
    let mut net = Net::new();
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, dialect);
    (Engine::new(net), sw, Session::default())
}

#[test]
fn ios_security_qos_config_show() {
    let (mut eng, sw, mut s) = switch(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "ip dhcp snooping",
        "interface g0/0",
        "ip dhcp snooping trust",
        "switchport port-security maximum 2",
        "switchport port-security violation shutdown",
        "storm-control broadcast level 100",
        "rate-limit input 1000",
        "end",
    ] {
        let out = exec_and_run(&mut eng, &mut s, sw, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    let ps = exec_and_run(&mut eng, &mut s, sw, "show port-security");
    assert!(ps.contains("GigabitEthernet0/0"), "{}", ps);
    assert!(ps.contains("shutdown"), "{}", ps);
    let qos = exec_and_run(&mut eng, &mut s, sw, "show storm-control");
    assert!(qos.contains("100") && qos.contains("1000"), "{}", qos);
    let b = exec_and_run(&mut eng, &mut s, sw, "show ip dhcp snooping binding");
    assert!(b.contains("无") || b.contains("MacAddress"), "{}", b);
}

#[test]
fn vrp_security_qos_config_display() {
    let (mut eng, sw, mut s) = switch(Dialect::Vrp);
    for line in [
        "system-view",
        "dhcp snooping enable",
        "interface g0/0/1",
        "dhcp snooping trusted",
        "port-security enable",
        "port-security max-mac-num 2",
        "port-security protect-action shutdown",
        "broadcast-suppression 100",
        "qos car inbound cir 1000",
        "quit",
    ] {
        let out = exec_and_run(&mut eng, &mut s, sw, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    let ps = exec_and_run(&mut eng, &mut s, sw, "display port-security");
    assert!(ps.contains("GigabitEthernet0/0/1"), "{}", ps);
    let qos = exec_and_run(&mut eng, &mut s, sw, "display qos");
    assert!(qos.contains("100") && qos.contains("1000"), "{}", qos);
    let b = exec_and_run(&mut eng, &mut s, sw, "display dhcp snooping user-binding");
    assert!(b.contains("No") || b.contains("MAC"), "{}", b);
}

#[test]
fn security_qos_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "ip dhcp snooping"),
        (Dialect::Ios, Mode::If, "ip dhcp snooping trust"),
        (Dialect::Ios, Mode::If, "switchport port-security"),
        (Dialect::Ios, Mode::If, "switchport port-security maximum 2"),
        (
            Dialect::Ios,
            Mode::If,
            "switchport port-security violation restart",
        ),
        (
            Dialect::Ios,
            Mode::If,
            "switchport port-security mac-address sticky",
        ),
        (Dialect::Ios, Mode::If, "storm-control broadcast level 100"),
        (Dialect::Ios, Mode::If, "rate-limit input 1000"),
        (Dialect::Ios, Mode::Exec, "show port-security"),
        (Dialect::Ios, Mode::Exec, "show storm-control"),
        (Dialect::Ios, Mode::Exec, "show ip dhcp snooping binding"),
        (Dialect::Vrp, Mode::Config, "dhcp snooping enable"),
        (Dialect::Vrp, Mode::If, "dhcp snooping trusted"),
        (Dialect::Vrp, Mode::If, "port-security enable"),
        (Dialect::Vrp, Mode::If, "port-security max-mac-num 2"),
        (
            Dialect::Vrp,
            Mode::If,
            "port-security protect-action shutdown",
        ),
        (Dialect::Vrp, Mode::If, "broadcast-suppression 100"),
        (Dialect::Vrp, Mode::If, "qos car inbound cir 1000"),
        (Dialect::Vrp, Mode::If, "qos lr inbound cir 1000"),
        (Dialect::Vrp, Mode::Exec, "display port-security"),
        (Dialect::Vrp, Mode::Exec, "display storm-control"),
        (Dialect::Comware, Mode::Config, "dhcp snooping enable"),
        (Dialect::Comware, Mode::If, "dhcp snooping trust"),
        (Dialect::Comware, Mode::If, "port-security max-mac-num 2"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
