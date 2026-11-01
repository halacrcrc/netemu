//! IPsec 站点隧道（教学 IP-in-IP）：三方言配置与隧道转发。

use cli_dialects::{exec_and_run, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::{Engine, PING_HORIZON_NS};
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn ip(s: &str) -> u32 {
    ipv4_from_str(s).unwrap()
}

fn ping(eng: &mut Engine, from: u32, to: &str) -> u32 {
    eng.start_ping(from, ip(to));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let d = eng.net.device(from).unwrap();
    match &d.state {
        DeviceState::Host(h) => h.pings.last().map(|p| p.received()).unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn ipsec_site_to_site_tunnel() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Vrp);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.add_link(r1, r2).unwrap();
    net.add_link(r2, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(pc2, Some(ip("192.168.2.10")), 24, Some(ip("192.168.2.1")));
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
        "ip address 10.0.0.1 30",
        "undo shutdown",
        "quit",
        "ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2",
        "local-subnet 192.168.1.0 24",
        "remote-subnet 192.168.2.0 24",
        "quit",
    ] {
        let out = exec_and_run(&mut eng, &mut s1, r1, line);
        assert!(!out.contains("Error:"), "R1 {}: {}", line, out);
    }
    for line in [
        "system-view",
        "interface g0/0/0",
        "ip address 10.0.0.2 30",
        "undo shutdown",
        "quit",
        "interface g0/0/1",
        "ip address 192.168.2.1 24",
        "undo shutdown",
        "quit",
        "ipsec tunnel T1 local 10.0.0.2 remote 10.0.0.1",
        "local-subnet 192.168.2.0 24",
        "remote-subnet 192.168.1.0 24",
        "quit",
    ] {
        let out = exec_and_run(&mut eng, &mut s2, r2, line);
        assert!(!out.contains("Error:"), "R2 {}: {}", line, out);
    }
    assert_eq!(
        ping(&mut eng, pc1, "192.168.2.10"),
        5,
        "IPsec 隧道应连通两站点"
    );
    let show = exec_and_run(&mut eng, &mut s1, r1, "display ipsec tunnel");
    assert!(show.contains("T1"), "{}", show);
    assert!(show.contains("192.168.1.0/24"), "{}", show);
    assert!(show.contains("192.168.2.0/24"), "{}", show);
}

#[test]
fn ipsec_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (
            Dialect::Ios,
            Mode::Config,
            "ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2",
        ),
        (Dialect::Ios, Mode::Ipsec, "local-subnet 192.168.1.0 24"),
        (Dialect::Ios, Mode::Ipsec, "remote-subnet 192.168.2.0 24"),
        (Dialect::Ios, Mode::Exec, "show ipsec tunnel"),
        (
            Dialect::Vrp,
            Mode::Config,
            "ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2",
        ),
        (Dialect::Vrp, Mode::Ipsec, "local-subnet 192.168.1.0 24"),
        (Dialect::Vrp, Mode::Exec, "display ipsec tunnel"),
        (
            Dialect::Comware,
            Mode::Config,
            "ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2",
        ),
        (
            Dialect::Comware,
            Mode::Ipsec,
            "remote-subnet 192.168.2.0 24",
        ),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
