//! 大型园区综合实验：VLAN/LACP/SVI/OSPF/VRRP/NAT 端到端。

use cli_dialects::exec_and_run;
use cli_engine::Session;
use sim_core::device::{DeviceKind, DeviceState, Dialect, VrrpState};
use sim_core::engine::{Engine, PING_HORIZON_NS};
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn ip(s: &str) -> u32 {
    ipv4_from_str(s).unwrap()
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
fn campus_capstone_end_to_end() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let pc3 = net.add_device("PC3".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let dsw = net.add_device("DSW".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    let pc4 = net.add_device("PC4".into(), DeviceKind::Host, Dialect::Ios);
    let core1 = net.add_device("CORE1".into(), DeviceKind::Router, Dialect::Ios);
    let core2 = net.add_device("CORE2".into(), DeviceKind::Router, Dialect::Ios);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap(); // SW1 p0
    net.add_link(pc3, sw1).unwrap(); // SW1 p1
    net.add_link(sw1, sw2).unwrap(); // SW1 p2 / SW2 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p3 / SW2 p1
    net.add_link(sw1, dsw).unwrap(); // SW1 p4 / DSW p0
    net.add_link(sw2, pc2).unwrap(); // SW2 p2
    net.add_link(sw2, pc4).unwrap(); // SW2 p3
    net.add_link(dsw, core1).unwrap(); // DSW p1 / CORE1 p0
    net.add_link(dsw, core2).unwrap(); // DSW p2 / CORE2 p0
    net.add_link(core1, core2).unwrap(); // CORE1 p1 / CORE2 p1
    net.add_link(core1, srv).unwrap(); // CORE1 p2
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, Some(ip("192.168.10.1")));
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, Some(ip("192.168.10.1")));
    net.set_host(pc3, Some(ip("192.168.20.21")), 24, Some(ip("192.168.20.1")));
    net.set_host(pc4, Some(ip("192.168.20.22")), 24, Some(ip("192.168.20.1")));
    net.set_host(srv, Some(ip("203.0.113.10")), 24, None);

    let mut eng = Engine::new(net);
    let mut s_sw1 = Session::default();
    let mut s_sw2 = Session::default();
    let mut s_dsw = Session::default();
    let mut s_c1 = Session::default();
    let mut s_c2 = Session::default();

    run(
        &mut eng,
        &mut s_sw1,
        sw1,
        &[
            "system-view",
            "vlan batch 10 20",
            "interface g0/0/1",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type access",
            "port default vlan 20",
            "undo shutdown",
            "quit",
            "interface g0/0/3",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/4",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/5",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/3",
            "eth-trunk 1",
            "quit",
            "interface g0/0/4",
            "eth-trunk 1",
            "quit",
        ],
        "SW1",
    );
    run(
        &mut eng,
        &mut s_sw2,
        sw2,
        &[
            "system-view",
            "vlan batch 10 20",
            "interface g0/0/1",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/3",
            "port link-type access",
            "port default vlan 10",
            "undo shutdown",
            "quit",
            "interface g0/0/4",
            "port link-type access",
            "port default vlan 20",
            "undo shutdown",
            "quit",
            "interface g0/0/1",
            "eth-trunk 1",
            "quit",
            "interface g0/0/2",
            "eth-trunk 1",
            "quit",
        ],
        "SW2",
    );
    run(
        &mut eng,
        &mut s_dsw,
        dsw,
        &[
            "system-view",
            "vlan batch 10 20 99",
            "interface g0/0/1",
            "port link-type trunk",
            "port trunk allow-pass vlan 10 20",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "port link-type access",
            "port default vlan 99",
            "undo shutdown",
            "quit",
            "interface g0/0/3",
            "port link-type access",
            "port default vlan 99",
            "undo shutdown",
            "quit",
            "interface vlanif 10",
            "ip address 192.168.10.1 24",
            "undo shutdown",
            "quit",
            "interface vlanif 20",
            "ip address 192.168.20.1 24",
            "undo shutdown",
            "quit",
            "interface vlanif 99",
            "ip address 10.0.0.2 24",
            "undo shutdown",
            "quit",
            "ip route-static 0.0.0.0 0 10.0.0.1",
        ],
        "DSW",
    );
    run(
        &mut eng,
        &mut s_c1,
        core1,
        &[
            "enable",
            "configure terminal",
            "interface g0/0",
            "ip address 10.0.0.10 255.255.255.0",
            "no shutdown",
            "vrrp 1 ip 10.0.0.1",
            "vrrp 1 priority 200",
            "vrrp 1 preempt",
            "ip nat inside",
            "exit",
            "interface g0/1",
            "ip address 10.1.0.1 255.255.255.252",
            "no shutdown",
            "exit",
            "interface g0/2",
            "ip address 203.0.113.1 255.255.255.0",
            "no shutdown",
            "ip nat outside",
            "exit",
            "ip route 192.168.0.0 255.255.0.0 10.0.0.2",
            "router ospf 1",
            "network 10.1.0.0 0.0.0.3 area 0",
            "end",
        ],
        "CORE1",
    );
    run(
        &mut eng,
        &mut s_c2,
        core2,
        &[
            "enable",
            "configure terminal",
            "interface g0/0",
            "ip address 10.0.0.11 255.255.255.0",
            "no shutdown",
            "vrrp 1 ip 10.0.0.1",
            "vrrp 1 priority 100",
            "exit",
            "interface g0/1",
            "ip address 10.1.0.2 255.255.255.252",
            "no shutdown",
            "exit",
            "router ospf 1",
            "network 10.1.0.0 0.0.0.3 area 0",
            "end",
        ],
        "CORE2",
    );
    for _ in 0..4 {
        let h = eng.drain_horizon();
        eng.run_to_quiescence(h);
    }

    assert_eq!(
        host_ping(&mut eng, pc1, "192.168.10.12"),
        5,
        "跨交换机同 VLAN"
    );
    assert_eq!(
        host_ping(&mut eng, pc1, "192.168.20.21"),
        5,
        "跨 VLAN 经 SVI"
    );
    assert_eq!(
        host_ping(&mut eng, pc3, "192.168.20.22"),
        5,
        "另一楼同 VLAN"
    );
    assert_eq!(
        host_ping(&mut eng, pc1, "203.0.113.10"),
        5,
        "园区经 NAT 上外网"
    );

    if let DeviceState::Router(rs) = &eng.net.device(core1).unwrap().state {
        assert_eq!(
            rs.vrrp[0].state,
            VrrpState::Master,
            "CORE1 应为 VRRP Master"
        );
    }
    if let DeviceState::Router(rs) = &eng.net.device(core2).unwrap().state {
        assert_eq!(rs.vrrp[0].state, VrrpState::Backup, "CORE2 应为 Backup");
    }
    assert!(
        !eng.ospf_neighbors_of(core1).is_empty(),
        "CORE1 应有 OSPF 邻居"
    );
    if let DeviceState::Switch(sw) = &eng.net.device(sw1).unwrap().state {
        let g = sw.lags.get(&1).expect("SW1 应有聚合组 1");
        assert_eq!(g.selected.len(), 2, "SW1 聚合组应选中 2 个成员");
    }
}
