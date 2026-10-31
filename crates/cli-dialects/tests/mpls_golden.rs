//! MPLS：三方言静态 LSP 配置与展示。

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
fn ios_mpls_static_lsp() {
    let (mut eng, r1, mut s) = router(Dialect::Ios);
    for line in [
        "enable",
        "configure terminal",
        "mpls static-lsp ingress to-pc2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100",
        "mpls static-lsp transit trans1 incoming-label 100 nexthop 10.0.1.2 out-label 200",
        "mpls static-lsp egress eg1 incoming-label 200",
        "end",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("% "), "{}: {}", line, out);
    }
    let show = exec_and_run(&mut eng, &mut s, r1, "show mpls lsp");
    assert!(show.contains("to-pc2") && show.contains("100"), "{}", show);
}

#[test]
fn vrp_mpls_static_lsp() {
    let (mut eng, r1, mut s) = router(Dialect::Vrp);
    for line in [
        "system-view",
        "static-lsp ingress to-pc2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100",
        "static-lsp transit trans1 incoming-label 100 nexthop 10.0.1.2 out-label 200",
        "static-lsp egress eg1 incoming-label 200",
    ] {
        let out = exec_and_run(&mut eng, &mut s, r1, line);
        assert!(!out.contains("Error:"), "{}: {}", line, out);
    }
    let show = exec_and_run(&mut eng, &mut s, r1, "display mpls lsp");
    assert!(show.contains("to-pc2") && show.contains("100"), "{}", show);
}

#[test]
fn mpls_l3vpn_ce_to_ce() {
    let mut net = Net::new();
    let ce1 = net.add_device("CE1".into(), DeviceKind::Host, Dialect::Ios);
    let pe1 = net.add_device("PE1".into(), DeviceKind::Router, Dialect::Vrp);
    let p = net.add_device("P".into(), DeviceKind::Router, Dialect::Vrp);
    let pe2 = net.add_device("PE2".into(), DeviceKind::Router, Dialect::Vrp);
    let ce2 = net.add_device("CE2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(ce1, pe1).unwrap(); // PE1 p0
    net.add_link(pe1, p).unwrap(); // PE1 p1 / P p0
    net.add_link(p, pe2).unwrap(); // P p1 / PE2 p0
    net.add_link(pe2, ce2).unwrap(); // PE2 p1
    net.set_host(
        ce1,
        Some(ipv4_from_str("192.168.1.10").unwrap()),
        24,
        Some(ipv4_from_str("192.168.1.1").unwrap()),
    );
    net.set_host(
        ce2,
        Some(ipv4_from_str("192.168.2.10").unwrap()),
        24,
        Some(ipv4_from_str("192.168.2.1").unwrap()),
    );
    let mut eng = Engine::new(net);
    let mut s_pe1 = Session::default();
    let mut s_p = Session::default();
    let mut s_pe2 = Session::default();
    for line in [
        "system-view",
        "ip vpn-instance CUST1",
        "interface g0/0/0",
        "ip binding vpn-instance CUST1",
        "ip address 192.168.1.1 24",
        "undo shutdown",
        "quit",
        "interface g0/0/1",
        "ip address 10.0.0.1 24",
        "undo shutdown",
        "quit",
        "static-lsp ingress to-ce2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100 vrf CUST1 vpn-label 1001",
        "static-lsp egress trans400 incoming-label 400",
        "static-lsp egress vpn1002 incoming-label 1002 vrf CUST1",
    ] {
        let out = exec_and_run(&mut eng, &mut s_pe1, pe1, line);
        assert!(!out.contains("Error:"), "PE1 {}: {}", line, out);
    }
    for line in [
        "system-view",
        "interface g0/0/0",
        "ip address 10.0.0.2 24",
        "undo shutdown",
        "quit",
        "interface g0/0/1",
        "ip address 10.0.1.1 24",
        "undo shutdown",
        "quit",
        "static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200",
        "static-lsp transit t2 incoming-label 300 nexthop 10.0.0.1 out-label 400",
    ] {
        let out = exec_and_run(&mut eng, &mut s_p, p, line);
        assert!(!out.contains("Error:"), "P {}: {}", line, out);
    }
    for line in [
        "system-view",
        "ip vpn-instance CUST1",
        "interface g0/0/0",
        "ip address 10.0.1.2 24",
        "undo shutdown",
        "quit",
        "interface g0/0/1",
        "ip binding vpn-instance CUST1",
        "ip address 192.168.2.1 24",
        "undo shutdown",
        "quit",
        "static-lsp egress trans200 incoming-label 200",
        "static-lsp egress vpn1001 incoming-label 1001 vrf CUST1",
        "static-lsp ingress to-ce1 destination 192.168.1.0 24 nexthop 10.0.1.1 out-label 300 vrf CUST1 vpn-label 1002",
    ] {
        let out = exec_and_run(&mut eng, &mut s_pe2, pe2, line);
        assert!(!out.contains("Error:"), "PE2 {}: {}", line, out);
    }

    let ping = |eng: &mut Engine, from: u32, to: &str| -> u32 {
        eng.start_ping(from, ipv4_from_str(to).unwrap());
        eng.run_to_quiescence(sim_core::engine::PING_HORIZON_NS);
        let d = eng.net.device(from).unwrap();
        match &d.state {
            sim_core::device::DeviceState::Host(h) => {
                h.pings.last().map(|p| p.received()).unwrap_or(0)
            }
            _ => 0,
        }
    };
    assert_eq!(ping(&mut eng, ce1, "192.168.2.10"), 5, "CE1 → CE2 应通");
    assert_eq!(ping(&mut eng, ce2, "192.168.1.10"), 5, "CE2 → CE1 应通");

    let pe1_show = exec_and_run(&mut eng, &mut s_pe1, pe1, "display mpls lsp");
    assert!(pe1_show.contains("to-ce2"), "{}", pe1_show);
    assert!(pe1_show.contains("CUST1"), "{}", pe1_show);
    assert!(pe1_show.contains("1001"), "{}", pe1_show);
    let pe2_vrf = exec_and_run(
        &mut eng,
        &mut s_pe2,
        pe2,
        "display ip routing-table vpn-instance CUST1",
    );
    assert!(pe2_vrf.contains("192.168.2.0/24"), "{}", pe2_vrf);

    // 摘要重载后 VPN 标签映射与数据面仍生效
    let sum = eng.net.summary();
    let mut net2 = Net::new();
    net2.apply_summary(&sum).unwrap();
    let mut eng2 = Engine::new(net2);
    eng2.after_load();
    assert_eq!(
        ping(&mut eng2, ce1, "192.168.2.10"),
        5,
        "摘要重载后 CE1 → CE2 仍应通"
    );
}

#[test]
fn mpls_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> =
        vec![
        (
            Dialect::Ios,
            Mode::Config,
            "mpls static-lsp ingress a destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100",
        ),
        (
            Dialect::Ios,
            Mode::Config,
            "mpls static-lsp transit b incoming-label 100 nexthop 10.0.1.2 out-label 200",
        ),
        (Dialect::Ios, Mode::Config, "mpls static-lsp egress c incoming-label 200"),
        (Dialect::Ios, Mode::Exec, "show mpls lsp"),
        (
            Dialect::Vrp,
            Mode::Config,
            "static-lsp ingress a destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100",
        ),
        (
            Dialect::Vrp,
            Mode::Config,
            "static-lsp transit b incoming-label 100 nexthop 10.0.1.2 out-label 200",
        ),
        (Dialect::Vrp, Mode::Config, "static-lsp egress c incoming-label 200"),
        (
            Dialect::Vrp,
            Mode::Config,
            "static-lsp ingress vpn1 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100 vrf CUST1 vpn-label 1001",
        ),
        (
            Dialect::Vrp,
            Mode::Config,
            "static-lsp egress vpn2 incoming-label 1001 vrf CUST1",
        ),
        (Dialect::Vrp, Mode::Exec, "display mpls lsp"),
        (Dialect::Comware, Mode::Config, "static-lsp egress c incoming-label 200"),
        (
            Dialect::Comware,
            Mode::Config,
            "static-lsp egress vpn2 incoming-label 1001 vrf CUST1",
        ),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
