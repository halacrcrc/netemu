//! 堆叠/IRF 与 M-LAG：共享系统 MAC、fabric 链路与跨设备 LACP。

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
fn stack_domain_and_forwarding() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap(); // SW1 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p1 / SW2 p0 stack link
    net.add_link(sw2, pc2).unwrap(); // SW2 p1
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);
    let mut eng = Engine::new(net);
    let mut s1 = Session::default();
    let mut s2 = Session::default();
    run(
        &mut eng,
        &mut s1,
        sw1,
        &[
            "system-view",
            "stack domain 10",
            "stack member 1",
            "stack priority 200",
            "vlan batch 10",
            "interface g0/0/1",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type trunk",
            "port trunk allow-pass vlan 10",
            "undo shutdown",
            "quit",
        ],
        "SW1",
    );
    run(
        &mut eng,
        &mut s2,
        sw2,
        &[
            "system-view",
            "stack domain 10",
            "stack member 2",
            "stack priority 100",
            "vlan batch 10",
            "interface g0/0/1",
            "port link-type trunk",
            "port trunk allow-pass vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
        ],
        "SW2",
    );
    assert_eq!(ping(&mut eng, pc1, "192.168.10.12"), 5, "堆叠成员间应互通");
    let show = exec_and_run(&mut eng, &mut s1, sw1, "display stack");
    assert!(show.contains("Stack domain: 10"), "{}", show);
    assert!(show.contains("Master: SW1"), "{}", show);
}

#[test]
fn mlag_cross_device_lacp_and_forwarding() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let d1 = net.add_device("DSW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let d2 = net.add_device("DSW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap(); // SW1 p0
    net.add_link(sw1, d1).unwrap(); // SW1 p1 / DSW1 p0
    net.add_link(sw1, d2).unwrap(); // SW1 p2 / DSW2 p0
    net.add_link(d1, d2).unwrap(); // DSW1 p1 / DSW2 p1 peer-link
    net.add_link(d2, pc2).unwrap(); // DSW2 p2
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);
    let mut eng = Engine::new(net);
    let mut s_sw = Session::default();
    let mut s1 = Session::default();
    let mut s2 = Session::default();
    run(
        &mut eng,
        &mut s_sw,
        sw1,
        &[
            "system-view",
            "vlan batch 10",
            "stp disable",
            "interface g0/0/1",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type trunk",
            "port trunk allow-pass vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/3",
            "port link-type trunk",
            "port trunk allow-pass vlan 10",
            "undo shutdown",
            "quit",
            "interface eth-trunk 1",
            "mode lacp-static",
            "quit",
            "interface g0/0/2",
            "eth-trunk 1",
            "quit",
            "interface g0/0/3",
            "eth-trunk 1",
            "quit",
        ],
        "SW1",
    );
    for (dev, sess, label) in [(d1, &mut s1, "DSW1"), (d2, &mut s2, "DSW2")] {
        run(
            &mut eng,
            sess,
            dev,
            &[
                "system-view",
                "vlan batch 10",
                "mlag domain 1",
                "interface g0/0/1",
                "port link-type trunk",
                "port trunk allow-pass vlan 10",
                "undo shutdown",
                "quit",
                "interface g0/0/2",
                "port link-type trunk",
                "port trunk allow-pass vlan 10",
                "undo shutdown",
                "mlag peer-link",
                "quit",
                "interface eth-trunk 1",
                "mode lacp-static",
                "quit",
                "interface g0/0/1",
                "eth-trunk 1",
                "quit",
            ],
            label,
        );
    }
    // DSW2 的 PC2 端口
    run(
        &mut eng,
        &mut s2,
        d2,
        &[
            "interface g0/0/3",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
        ],
        "DSW2",
    );
    for _ in 0..4 {
        let h = eng.drain_horizon();
        eng.run_to_quiescence(h);
    }
    let selected = match &eng.net.device(sw1).unwrap().state {
        DeviceState::Switch(sw) => sw.lags.get(&1).map(|g| g.selected.len()).unwrap_or(0),
        _ => 0,
    };
    assert_eq!(selected, 2, "SW1 跨设备 LACP 应选中两个成员");
    assert_eq!(ping(&mut eng, pc1, "192.168.10.12"), 5, "M-LAG 双归应互通");
    let show = exec_and_run(&mut eng, &mut s1, d1, "display mlag");
    assert!(show.contains("M-LAG domain: 1"), "{}", show);
}

#[test]
fn stack_mlag_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "stack domain 10"),
        (Dialect::Ios, Mode::Config, "stack member 1"),
        (Dialect::Ios, Mode::Config, "stack priority 200"),
        (Dialect::Ios, Mode::Config, "mlag domain 1"),
        (Dialect::Ios, Mode::If, "mlag peer-link"),
        (Dialect::Ios, Mode::Exec, "show stack"),
        (Dialect::Ios, Mode::Exec, "show mlag"),
        (Dialect::Vrp, Mode::Config, "stack domain 10"),
        (Dialect::Vrp, Mode::Config, "mlag domain 1"),
        (Dialect::Vrp, Mode::If, "mlag peer-link"),
        (Dialect::Vrp, Mode::Exec, "display stack"),
        (Dialect::Vrp, Mode::Exec, "display mlag"),
        (Dialect::Comware, Mode::Config, "stack member 2"),
        (Dialect::Comware, Mode::Exec, "display mlag"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
