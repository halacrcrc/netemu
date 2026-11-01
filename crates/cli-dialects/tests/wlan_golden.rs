//! WLAN：AC/AP、CAPWAP 注册、WPA2 关联与无线 VLAN 转发。

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
fn wlan_capwap_wpa2_and_forwarding() {
    let mut net = Net::new();
    let ac = net.add_device("AC1".into(), DeviceKind::WlanController, Dialect::Vrp);
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let ap = net.add_device("AP1".into(), DeviceKind::AccessPoint, Dialect::Vrp);
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(ac, sw).unwrap(); // SW p0 (VLAN1)
    net.add_link(sw, ap).unwrap(); // SW p1 trunk / AP p0
    net.add_link(ap, pc1).unwrap(); // AP p1
    net.add_link(ap, pc2).unwrap(); // AP p2
    net.add_link(sw, srv).unwrap(); // SW p2 access VLAN10
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);
    net.set_host(srv, Some(ip("192.168.10.10")), 24, None);
    let mut eng = Engine::new(net);
    let mut s_ac = Session::default();
    let mut s_sw = Session::default();
    let mut s_ap = Session::default();
    let mut s_pc1 = Session::default();
    let mut s_pc2 = Session::default();

    run(
        &mut eng,
        &mut s_ac,
        ac,
        &[
            "system-view",
            "interface g0/0/0",
            "ip address 10.0.0.2 24",
            "undo shutdown",
            "quit",
            "wlan ssid CORP",
            "wlan security wpa2 psk Admin@123",
            "wlan vlan 10",
        ],
        "AC1",
    );
    run(
        &mut eng,
        &mut s_sw,
        sw,
        &[
            "system-view",
            "vlan batch 10",
            "interface g0/0/2",
            "port link-type trunk",
            "port trunk allow-pass vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/3",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
        ],
        "SW1",
    );
    run(
        &mut eng,
        &mut s_ap,
        ap,
        &[
            "system-view",
            "interface g0/0/1",
            "port link-type trunk",
            "port trunk allow-pass vlan 10",
            "undo shutdown",
            "quit",
            "interface vlanif 1",
            "ip address 10.0.0.10 24",
            "quit",
            "capwap ac 10.0.0.2",
        ],
        "AP1",
    );
    let ap_show = exec_and_run(&mut eng, &mut s_ap, ap, "display wlan");
    assert!(ap_show.contains("CORP"), "{}", ap_show);
    assert!(ap_show.contains("Joined"), "{}", ap_show);
    let ac_show = exec_and_run(&mut eng, &mut s_ac, ac, "display wlan");
    assert!(ac_show.contains("AP1"), "{}", ac_show);
    assert!(ac_show.contains("wpa2-psk"), "{}", ac_show);

    let bad = exec_and_run(&mut eng, &mut s_pc2, pc2, "wifi connect CORP wrong");
    assert!(bad.contains("失败"), "{}", bad);
    let ok = exec_and_run(&mut eng, &mut s_pc1, pc1, "wifi connect CORP Admin@123");
    assert!(ok.contains("已连接"), "{}", ok);
    let status = exec_and_run(&mut eng, &mut s_pc1, pc1, "wifi status");
    assert!(status.contains("CORP"), "{}", status);

    assert_eq!(
        ping(&mut eng, pc1, "192.168.10.10"),
        5,
        "关联终端应能访问有线服务器"
    );
    assert_eq!(
        ping(&mut eng, pc2, "192.168.10.10"),
        0,
        "未关联终端应被 AP 丢弃"
    );
}

#[test]
fn wlan_multi_ssid_wpa3_and_roaming() {
    let mut net = Net::new();
    let ac = net.add_device("AC1".into(), DeviceKind::WlanController, Dialect::Vrp);
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let ap1 = net.add_device("AP1".into(), DeviceKind::AccessPoint, Dialect::Vrp);
    let ap2 = net.add_device("AP2".into(), DeviceKind::AccessPoint, Dialect::Vrp);
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    let srv10 = net.add_device("SRV10".into(), DeviceKind::Host, Dialect::Ios);
    let srv20 = net.add_device("SRV20".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(ac, sw).unwrap(); // SW p0
    net.add_link(ap1, sw).unwrap(); // SW p1 / AP1 p0
    net.add_link(ap2, sw).unwrap(); // SW p2 / AP2 p0
    net.add_link(sw, srv10).unwrap(); // SW p3
    net.add_link(sw, srv20).unwrap(); // SW p4
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.20.22")), 24, None);
    net.set_host(srv10, Some(ip("192.168.10.10")), 24, None);
    net.set_host(srv20, Some(ip("192.168.20.10")), 24, None);
    let mut eng = Engine::new(net);
    let mut s_ac = Session::default();
    let mut s_sw = Session::default();
    let mut s_ap1 = Session::default();
    let mut s_ap2 = Session::default();
    let mut s_pc1 = Session::default();
    let mut s_pc2 = Session::default();

    run(
        &mut eng,
        &mut s_ac,
        ac,
        &[
            "system-view",
            "interface g0/0/0",
            "ip address 10.0.0.2 24",
            "undo shutdown",
            "quit",
            "wlan ssid CORP",
            "wlan security wpa2 psk Admin@123",
            "wlan vlan 10",
            "wlan ssid GUEST",
            "wlan security wpa3 sae Guest@123",
            "wlan vlan 20",
        ],
        "AC1",
    );
    run(
        &mut eng,
        &mut s_sw,
        sw,
        &[
            "system-view",
            "vlan batch 10 20",
            "interface g0/0/2",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/3",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/4",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/5",
            "port link-type access",
            "port default vlan 20",
            "undo shutdown",
            "quit",
        ],
        "SW1",
    );
    run(
        &mut eng,
        &mut s_ap1,
        ap1,
        &[
            "system-view",
            "interface g0/0/1",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface vlanif 1",
            "ip address 10.0.0.10 24",
            "quit",
            "wlan priority 200",
            "capwap ac 10.0.0.2",
        ],
        "AP1",
    );
    run(
        &mut eng,
        &mut s_ap2,
        ap2,
        &[
            "system-view",
            "interface g0/0/1",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface vlanif 1",
            "ip address 10.0.0.11 24",
            "quit",
            "wlan priority 100",
            "capwap ac 10.0.0.2",
        ],
        "AP2",
    );
    let ac_show = exec_and_run(&mut eng, &mut s_ac, ac, "display wlan");
    assert!(
        ac_show.contains("CORP") && ac_show.contains("GUEST"),
        "{}",
        ac_show
    );
    assert!(ac_show.contains("wpa3-sae"), "{}", ac_show);

    let bad = exec_and_run(&mut eng, &mut s_pc2, pc2, "wifi connect GUEST wrong");
    assert!(bad.contains("失败"), "{}", bad);
    let g = exec_and_run(&mut eng, &mut s_pc2, pc2, "wifi connect GUEST Guest@123");
    assert!(g.contains("已连接"), "{}", g);
    let c = exec_and_run(&mut eng, &mut s_pc1, pc1, "wifi connect CORP Admin@123");
    assert!(c.contains("AP AP1"), "{}", c);
    assert_eq!(
        ping(&mut eng, pc1, "192.168.10.10"),
        5,
        "CORP 终端访问 VLAN10 服务器"
    );
    assert_eq!(
        ping(&mut eng, pc2, "192.168.20.10"),
        5,
        "GUEST 终端访问 VLAN20 服务器"
    );
    assert_eq!(
        ping(&mut eng, pc1, "192.168.20.10"),
        0,
        "不同 SSID/VLAN 默认隔离"
    );

    let roam = exec_and_run(&mut eng, &mut s_pc1, pc1, "wifi roam");
    assert!(roam.contains("AP2"), "{}", roam);
    assert_eq!(ping(&mut eng, pc1, "192.168.10.10"), 5, "漫游后业务不中断");
    assert_eq!(
        ping(&mut eng, pc2, "192.168.20.10"),
        5,
        "漫游不影响其他终端"
    );
}

#[test]
fn wlan_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "wlan ssid CORP"),
        (
            Dialect::Ios,
            Mode::Config,
            "wlan security wpa2 psk Admin@123",
        ),
        (
            Dialect::Ios,
            Mode::Config,
            "wlan security wpa3 sae Guest@123",
        ),
        (Dialect::Ios, Mode::Config, "wlan priority 200"),
        (Dialect::Ios, Mode::Config, "wlan vlan 10"),
        (Dialect::Ios, Mode::Config, "capwap ac 10.0.0.2"),
        (Dialect::Ios, Mode::Exec, "show wlan"),
        (Dialect::Vrp, Mode::Config, "wlan ssid CORP"),
        (
            Dialect::Vrp,
            Mode::Config,
            "wlan security wpa2 psk Admin@123",
        ),
        (
            Dialect::Vrp,
            Mode::Config,
            "wlan security wpa3 sae Guest@123",
        ),
        (Dialect::Vrp, Mode::Config, "wlan priority 100"),
        (Dialect::Vrp, Mode::Config, "capwap ac 10.0.0.2"),
        (Dialect::Vrp, Mode::Exec, "display wlan"),
        (Dialect::Comware, Mode::Config, "wlan vlan 10"),
        (Dialect::Comware, Mode::Exec, "display wlan"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
