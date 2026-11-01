//! 防火墙：区域划分、区域间策略、状态化放行。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::{Engine, PING_HORIZON_NS};
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn ip(s: &str) -> u32 {
    ipv4_from_str(s).unwrap()
}

fn firewall_topo(dialect: Dialect) -> (Engine, u32, u32, u32) {
    let mut net = Net::new();
    let pc = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let fw = net.add_device("FW1".into(), DeviceKind::Firewall, dialect);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc, fw).unwrap(); // FW p0
    net.add_link(fw, srv).unwrap(); // FW p1
    net.set_host(pc, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(srv, Some(ip("203.0.113.10")), 24, Some(ip("203.0.113.1")));
    (Engine::new(net), pc, fw, srv)
}

fn run(eng: &mut Engine, s: &mut Session, dev: u32, lines: &[&str], label: &str) {
    for line in lines {
        let out = exec_and_run(eng, s, dev, line);
        assert!(
            !out.contains("Error:") && !out.contains("% "),
            "{} {}: {}",
            label,
            line,
            out
        );
    }
}

fn host_ping(eng: &mut Engine, from: u32, to: &str) -> u32 {
    eng.start_ping(from, ip(to));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let d = eng.net.device(from).unwrap();
    match &d.state {
        DeviceState::Host(h) => h.pings.last().map(|p| p.received()).unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn vrp_firewall_zone_policy_stateful() {
    let (mut eng, pc, fw, srv) = firewall_topo(Dialect::Vrp);
    let mut s = Session::default();
    run(
        &mut eng,
        &mut s,
        fw,
        &[
            "system-view",
            "interface g0/0/0",
            "ip address 192.168.1.1 24",
            "undo shutdown",
            "quit",
            "interface g0/0/1",
            "ip address 203.0.113.1 24",
            "undo shutdown",
            "quit",
            "firewall zone trust",
            "add interface g0/0/0",
            "quit",
            "firewall zone untrust",
            "add interface g0/0/1",
            "quit",
            "security-policy",
            "rule name allow-out",
            "source-zone trust",
            "destination-zone untrust",
            "action permit",
            "quit",
            "quit",
        ],
        "FW1",
    );

    // 反向首包应被默认拒绝；正向放行后状态化回包才允许
    assert_eq!(
        host_ping(&mut eng, srv, "192.168.1.10"),
        0,
        "untrust→trust 默认拒绝"
    );
    assert_eq!(
        host_ping(&mut eng, pc, "203.0.113.10"),
        5,
        "trust→untrust 放行"
    );
    let zone_show = exec_and_run(&mut eng, &mut s, fw, "display firewall zone");
    assert!(
        zone_show.contains("trust") && zone_show.contains("untrust"),
        "{}",
        zone_show
    );
    let policy_show = exec_and_run(&mut eng, &mut s, fw, "display security-policy");
    assert!(policy_show.contains("allow-out"), "{}", policy_show);
    assert!(policy_show.contains("action permit"), "{}", policy_show);

    let mut pc_sess = Session::default();
    let tcp_open = exec_and_run(&mut eng, &mut pc_sess, pc, "tcp 203.0.113.10 80");
    assert!(tcp_open.contains("连接成功"), "{}", tcp_open);
    let tcp_refused = exec_and_run(&mut eng, &mut pc_sess, pc, "tcp 203.0.113.10 81");
    assert!(tcp_refused.contains("连接被拒绝"), "{}", tcp_refused);
    if let DeviceState::Router(rs) = &eng.net.device(fw).unwrap().state {
        assert!(!rs.tcp_flows.is_empty(), "防火墙应记录 TCP 状态");
        assert!(rs.tcp_flows.iter().all(|f| matches!(
            f.state,
            sim_core::device::TcpFlowState::FinWait | sim_core::device::TcpFlowState::Closed
        )));
    }

    let sum = eng.net.summary();
    let mut net2 = Net::new();
    net2.apply_summary(&sum).unwrap();
    let mut eng2 = Engine::new(net2);
    assert_eq!(
        host_ping(&mut eng2, pc, "203.0.113.10"),
        5,
        "摘要重载后策略仍生效"
    );
}

#[test]
fn ios_zone_pair_policy() {
    let (mut eng, pc, fw, srv) = firewall_topo(Dialect::Ios);
    let mut s = Session::default();
    run(
        &mut eng,
        &mut s,
        fw,
        &[
            "enable",
            "configure terminal",
            "interface g0/0",
            "ip address 192.168.1.1 255.255.255.0",
            "no shutdown",
            "zone-member security INSIDE",
            "exit",
            "interface g0/1",
            "ip address 203.0.113.1 255.255.255.0",
            "no shutdown",
            "zone-member security OUTSIDE",
            "exit",
            "zone-pair security IN-OUT",
            "source INSIDE",
            "destination OUTSIDE",
            "action permit",
            "exit",
            "end",
        ],
        "FW1",
    );
    assert_eq!(
        host_ping(&mut eng, srv, "192.168.1.10"),
        0,
        "OUTSIDE→INSIDE 默认拒绝"
    );
    assert_eq!(
        host_ping(&mut eng, pc, "203.0.113.10"),
        5,
        "INSIDE→OUTSIDE 放行"
    );
    let zones = exec_and_run(&mut eng, &mut s, fw, "show zone security");
    assert!(
        zones.contains("INSIDE") && zones.contains("OUTSIDE"),
        "{}",
        zones
    );
    let pairs = exec_and_run(&mut eng, &mut s, fw, "show zone-pair security");
    assert!(pairs.contains("IN-OUT"), "{}", pairs);
}

#[test]
fn firewall_tcp_denied_without_policy() {
    let (mut eng, pc, fw, _srv) = firewall_topo(Dialect::Vrp);
    let mut s = Session::default();
    run(
        &mut eng,
        &mut s,
        fw,
        &[
            "system-view",
            "interface g0/0/0",
            "ip address 192.168.1.1 24",
            "undo shutdown",
            "quit",
            "interface g0/0/1",
            "ip address 203.0.113.1 24",
            "undo shutdown",
            "quit",
            "firewall zone trust",
            "add interface g0/0/0",
            "quit",
            "firewall zone untrust",
            "add interface g0/0/1",
            "quit",
        ],
        "FW1",
    );
    let mut pcs = Session::default();
    let out = exec_and_run(&mut eng, &mut pcs, pc, "tcp 203.0.113.10 80");
    assert!(out.contains("无响应"), "{}", out);
}

#[test]
fn asa_security_level_default_policy() {
    let (mut eng, pc, fw, srv) = firewall_topo(Dialect::Ios);
    let mut s = Session::default();
    run(
        &mut eng,
        &mut s,
        fw,
        &[
            "enable",
            "configure terminal",
            "interface g0/0",
            "ip address 192.168.1.1 255.255.255.0",
            "no shutdown",
            "nameif inside",
            "security-level 100",
            "exit",
            "interface g0/1",
            "ip address 203.0.113.1 255.255.255.0",
            "no shutdown",
            "nameif outside",
            "security-level 0",
            "exit",
            "end",
        ],
        "FW1",
    );
    assert_eq!(
        host_ping(&mut eng, srv, "192.168.1.10"),
        0,
        "低级别→高级别默认拒绝"
    );
    assert_eq!(
        host_ping(&mut eng, pc, "203.0.113.10"),
        5,
        "高级别→低级别默认放行"
    );
    let mut pcs = Session::default();
    let open = exec_and_run(&mut eng, &mut pcs, pc, "tcp 203.0.113.10 443");
    assert!(open.contains("连接成功"), "{}", open);
}

#[test]
fn sslvpn_remote_access_tunnel() {
    let mut net = Net::new();
    let pc = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let fw = net.add_device("FW1".into(), DeviceKind::Firewall, Dialect::Vrp);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc, fw).unwrap(); // FW G0/0/0 outside
    net.add_link(fw, srv).unwrap(); // FW G0/0/1 inside
    net.set_host(pc, Some(ip("203.0.113.10")), 24, Some(ip("203.0.113.1")));
    net.set_host(srv, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    let mut eng = Engine::new(net);
    let mut s = Session::default();
    run(
        &mut eng,
        &mut s,
        fw,
        &[
            "system-view",
            "interface g0/0/0",
            "ip address 203.0.113.1 24",
            "undo shutdown",
            "quit",
            "interface g0/0/1",
            "ip address 192.168.1.1 24",
            "undo shutdown",
            "quit",
            "firewall zone untrust",
            "add interface g0/0/0",
            "quit",
            "firewall zone trust",
            "add interface g0/0/1",
            "quit",
            "aaa",
            "local-user admin password cipher Admin@123",
            "quit",
            "sslvpn virtual-ip 10.10.0.0 24",
        ],
        "FW1",
    );
    assert_eq!(
        host_ping(&mut eng, pc, "192.168.1.10"),
        0,
        "未连接 VPN 时不可达"
    );
    let mut pcs = Session::default();
    let bad = exec_and_run(
        &mut eng,
        &mut pcs,
        pc,
        "sslvpn connect 203.0.113.1 admin wrong",
    );
    assert!(bad.contains("失败"), "{}", bad);
    let ok = exec_and_run(
        &mut eng,
        &mut pcs,
        pc,
        "sslvpn connect 203.0.113.1 admin Admin@123",
    );
    assert!(ok.contains("虚拟 IP 10.10.0.2"), "{}", ok);
    assert_eq!(
        host_ping(&mut eng, pc, "192.168.1.10"),
        5,
        "SSL VPN 隧道应连通内网"
    );
    let sessions = exec_and_run(&mut eng, &mut s, fw, "display sslvpn session");
    assert!(sessions.contains("10.10.0.2"), "{}", sessions);
    assert!(sessions.contains("203.0.113.10"), "{}", sessions);
}

#[test]
fn comware_security_zone_policy_parse_and_display() {
    let (mut eng, _pc, fw, _srv) = firewall_topo(Dialect::Comware);
    let mut s = Session::default();
    run(
        &mut eng,
        &mut s,
        fw,
        &[
            "system-view",
            "interface g1/0/1",
            "ip address 192.168.1.1 24",
            "undo shutdown",
            "quit",
            "interface g1/0/2",
            "ip address 203.0.113.1 24",
            "undo shutdown",
            "quit",
            "security-zone name trust",
            "import interface g1/0/1",
            "quit",
            "security-zone name untrust",
            "import interface g1/0/2",
            "quit",
            "security-policy ip",
            "rule name allow-out",
            "source-zone trust",
            "destination-zone untrust",
            "action permit",
            "quit",
            "quit",
        ],
        "FW1",
    );
    let policy = exec_and_run(&mut eng, &mut s, fw, "display security-policy");
    assert!(policy.contains("allow-out"), "{}", policy);
    let zones = exec_and_run(&mut eng, &mut s, fw, "display security-zone");
    assert!(
        zones.contains("trust") && zones.contains("GE1/0/1"),
        "{}",
        zones
    );
}

#[test]
fn firewall_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "zone security INSIDE"),
        (Dialect::Ios, Mode::If, "zone-member security INSIDE"),
        (Dialect::Ios, Mode::If, "nameif inside"),
        (Dialect::Ios, Mode::If, "security-level 100"),
        (Dialect::Ios, Mode::Config, "zone-pair security IN-OUT"),
        (Dialect::Ios, Mode::SecurityRule, "source INSIDE"),
        (Dialect::Ios, Mode::SecurityRule, "destination OUTSIDE"),
        (Dialect::Ios, Mode::SecurityRule, "action permit"),
        (Dialect::Ios, Mode::Exec, "show zone security"),
        (Dialect::Ios, Mode::Exec, "show zone-pair security"),
        (Dialect::Vrp, Mode::Config, "firewall zone trust"),
        (Dialect::Vrp, Mode::Zone, "add interface g0/0/0"),
        (Dialect::Vrp, Mode::Config, "security-policy"),
        (Dialect::Vrp, Mode::SecurityPolicy, "rule name allow-out"),
        (Dialect::Vrp, Mode::SecurityRule, "source-zone trust"),
        (Dialect::Vrp, Mode::SecurityRule, "destination-zone untrust"),
        (Dialect::Vrp, Mode::SecurityRule, "action permit"),
        (Dialect::Vrp, Mode::Exec, "display security-policy"),
        (Dialect::Vrp, Mode::Exec, "display firewall zone"),
        (Dialect::Comware, Mode::Config, "security-zone name trust"),
        (Dialect::Comware, Mode::Zone, "import interface g1/0/1"),
        (Dialect::Comware, Mode::Config, "security-policy ip"),
        (
            Dialect::Comware,
            Mode::SecurityPolicy,
            "rule name allow-out",
        ),
        (Dialect::Comware, Mode::Exec, "display security-zone"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}

#[test]
fn firewall_prompt_modes() {
    let (mut eng, _pc, fw, _srv) = firewall_topo(Dialect::Vrp);
    let mut s = Session::default();
    exec_and_run(&mut eng, &mut s, fw, "system-view");
    exec_and_run(&mut eng, &mut s, fw, "firewall zone trust");
    assert_eq!(prompt(&eng, &s, fw), "[FW1-zone-trust]");
    exec_and_run(&mut eng, &mut s, fw, "quit");
    exec_and_run(&mut eng, &mut s, fw, "security-policy");
    assert_eq!(prompt(&eng, &s, fw), "[FW1-security-policy]");
    exec_and_run(&mut eng, &mut s, fw, "rule name r1");
    assert_eq!(prompt(&eng, &s, fw), "[FW1-security-policy-rule-r1]");
}
