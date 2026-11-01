//! VXLAN：双 VTEP、VNI 映射与跨 DC 二层互通。

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

#[test]
fn vxlan_cross_dc_ping() {
    let mut net = Net::new();
    let h1 = net.add_device("H1".into(), DeviceKind::Host, Dialect::Ios);
    let d1 = net.add_device("DSW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let d2 = net.add_device("DSW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let h2 = net.add_device("H2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(h1, d1).unwrap(); // DSW1 p0 VLAN10
    net.add_link(d1, d2).unwrap(); // DSW1 p1 / DSW2 p0 underlay VLAN100
    net.add_link(d2, h2).unwrap(); // DSW2 p1 VLAN10
    net.set_host(h1, Some(ip("192.168.10.10")), 24, None);
    net.set_host(h2, Some(ip("192.168.10.20")), 24, None);
    let mut eng = Engine::new(net);
    let mut s1 = Session::default();
    let mut s2 = Session::default();
    run(
        &mut eng,
        &mut s1,
        d1,
        &[
            "system-view",
            "vlan batch 10 100",
            "interface g0/0/1",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type access",
            "port default vlan 100",
            "undo shutdown",
            "quit",
            "interface vlanif 100",
            "ip address 10.0.0.1 24",
            "undo shutdown",
            "quit",
            "vxlan local 10.0.0.1",
            "vxlan vni 10000 vlan 10",
            "vxlan peer 10.0.0.2",
        ],
        "DSW1",
    );
    run(
        &mut eng,
        &mut s2,
        d2,
        &[
            "system-view",
            "vlan batch 10 100",
            "interface g0/0/1",
            "port link-type access",
            "port default vlan 100",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface vlanif 100",
            "ip address 10.0.0.2 24",
            "undo shutdown",
            "quit",
            "vxlan local 10.0.0.2",
            "vxlan vni 10000 vlan 10",
            "vxlan peer 10.0.0.1",
        ],
        "DSW2",
    );

    assert_eq!(ping(&mut eng, h1, "192.168.10.20"), 5, "VXLAN 跨 DC 应连通");
    let show = exec_and_run(&mut eng, &mut s1, d1, "display vxlan");
    assert!(show.contains("10000"), "{}", show);
    assert!(show.contains("10.0.0.2"), "{}", show);
}

#[test]
fn evpn_learns_remote_vtep() {
    let mut net = Net::new();
    let h1 = net.add_device("H1".into(), DeviceKind::Host, Dialect::Ios);
    let d1 = net.add_device("DSW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let d2 = net.add_device("DSW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let h2 = net.add_device("H2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(h1, d1).unwrap();
    net.add_link(d1, d2).unwrap();
    net.add_link(d2, h2).unwrap();
    net.set_host(h1, Some(ip("192.168.10.10")), 24, None);
    net.set_host(h2, Some(ip("192.168.10.20")), 24, None);
    let mut eng = Engine::new(net);
    let mut s1 = Session::default();
    let mut s2 = Session::default();
    run(
        &mut eng,
        &mut s1,
        d1,
        &[
            "system-view",
            "vlan batch 10 100",
            "interface g0/0/1",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type access",
            "port default vlan 100",
            "undo shutdown",
            "quit",
            "interface vlanif 100",
            "ip address 10.0.0.1 24",
            "undo shutdown",
            "quit",
            "vxlan local 10.0.0.1",
            "vxlan vni 10000 vlan 10",
            "evpn enable",
            "evpn peer 10.0.0.2",
        ],
        "DSW1",
    );
    run(
        &mut eng,
        &mut s2,
        d2,
        &[
            "system-view",
            "vlan batch 10 100",
            "interface g0/0/1",
            "port link-type access",
            "port default vlan 100",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface vlanif 100",
            "ip address 10.0.0.2 24",
            "undo shutdown",
            "quit",
            "vxlan local 10.0.0.2",
            "vxlan vni 10000 vlan 10",
            "evpn enable",
            "evpn peer 10.0.0.1",
        ],
        "DSW2",
    );
    let show = exec_and_run(&mut eng, &mut s1, d1, "display evpn");
    assert!(show.contains("10.0.0.2"), "{}", show);
    assert!(show.contains("10000"), "{}", show);
    assert_eq!(
        ping(&mut eng, h1, "192.168.10.20"),
        5,
        "EVPN 学习远端 VTEP 后应跨 DC 通"
    );
}

#[test]
fn vxlan_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "vxlan local 10.0.0.1"),
        (Dialect::Ios, Mode::Config, "vxlan peer 10.0.0.2"),
        (Dialect::Ios, Mode::Config, "vxlan vni 10000 vlan 10"),
        (Dialect::Ios, Mode::Exec, "show vxlan"),
        (Dialect::Vrp, Mode::Config, "vxlan local 10.0.0.1"),
        (Dialect::Vrp, Mode::Config, "vxlan vni 10000 vlan 10"),
        (Dialect::Vrp, Mode::Exec, "display vxlan"),
        (Dialect::Comware, Mode::Config, "vxlan peer 10.0.0.2"),
        (Dialect::Comware, Mode::Exec, "display vxlan"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
