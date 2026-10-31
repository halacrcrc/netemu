//! M1：三方言 VLAN/Trunk/SVI 命令的黄金输出测试。

use cli_dialects::{exec_and_run, prompt};
use cli_engine::Session;
use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn ip(s: &str) -> u32 {
    ipv4_from_str(s).unwrap()
}

// ---------- IOS 交换机 ----------

#[test]
fn ios_vlan_flow() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw).unwrap(); // g0/0
    net.add_link(sw, pc2).unwrap(); // g0/1
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);

    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    let lines = [
        "enable",
        "conf t",
        "vlan 10",
        "int g0/0",
        "switchport mode access",
        "switchport access vlan 10",
        "exit",
        "int vlan 10",
        "ip address 192.168.10.1 255.255.255.0",
        "end",
        "show vlan brief",
    ];
    let outs: Vec<String> = lines
        .iter()
        .map(|l| exec_and_run(&mut eng, &mut sess, sw, l))
        .collect();

    // 全部命令执行完（已 end 回特权模式）；SVI 配置静默成功
    assert_eq!(prompt(&eng, &sess, sw), "SW1#");
    assert_eq!(outs[8], "", "SVI ip address 应静默成功");

    // show vlan brief 黄金快照
    assert_eq!(
        outs[10],
        "VLAN Name                             Status    Ports\n\
         ---- -------------------------------- --------- -----------------------------\n\
         1    default                          active    Gi0/1(U) Gi0/2(U) Gi0/3(U) Gi0/4(U) Gi0/5(U) Gi0/6(U) Gi0/7(U)\n\
         10   VLAN0010                         active    Vlan10(U) Gi0/0(U)"
    );

    // VLAN 间无第二 VLAN：SVI10 应能 ping 同网段 PC
    let out = exec_and_run(&mut eng, &mut sess, sw, "ping 192.168.10.11");
    assert!(out.contains("Success rate is 100 percent (5/5)"), "{}", out);

    // 状态确实生效
    let DeviceState::Switch(sw_state) = &eng.net.device(sw).unwrap().state else {
        panic!();
    };
    assert!(sw_state.vlans.contains(&10));
    assert_eq!(eng.net.device(sw).unwrap().ports[0].pvid, 10);
}

// ---------- VRP ----------

#[test]
fn vrp_vlan_flow() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Vrp);
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Vrp);
    net.add_link(pc1, sw).unwrap(); // GE0/0/1
    net.add_link(sw, pc2).unwrap(); // GE0/0/2
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, Some(ip("192.168.10.1")));
    net.set_host(pc2, Some(ip("192.168.20.12")), 24, Some(ip("192.168.20.1")));

    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    let lines = [
        "system-view",
        "vlan batch 10 20",
        "int g0/0/1",
        "port link-type access",
        "port default vlan 10",
        "quit",
        "int g0/0/2",
        "port link-type access",
        "port default vlan 20",
        "quit",
        "int vlanif 10",
        "ip address 192.168.10.1 24",
        "quit",
        "int vlanif 20",
        "ip address 192.168.20.1 24",
        "quit",
        "display vlan",
    ];
    let outs: Vec<String> = lines
        .iter()
        .map(|l| exec_and_run(&mut eng, &mut sess, sw, l))
        .collect();

    // 全部命令执行完（已 quit 回用户视图）
    run_and_assert_prompt(&eng, &sess, sw, "[SW1]");

    // display vlan 黄金快照（SVI 也算成员，标记 U，SVI 列前）
    assert_eq!(
        outs[16],
        "  VLAN ID Name                    Ports\n  \
         --------------------------------------------------------------------\n  \
         1       default                 GE0/0/3(U) GE0/0/4(U) GE0/0/5(U) GE0/0/6(U) GE0/0/7(U) GE0/0/8(U)\n  \
         10      VLAN0010                Vlanif10(U) GE0/0/1(U)\n  \
         20      VLAN0020                Vlanif20(U) GE0/0/2(U)"
    );

    // 跨 VLAN ping（经 SVI 路由）
    let out = exec_and_run(&mut eng, &mut sess, sw, "ping 192.168.20.12");
    assert!(
        out.contains("0.00% packet loss"),
        "SVI 间路由 ping 应全通: {}",
        out
    );

    // PC1 → PC2 跨 VLAN
    let out = exec_and_run(&mut eng, &mut Session::default(), pc1, "ping 192.168.20.12");
    assert!(
        out.contains("5 packets transmitted, 5 received"),
        "PC1 ping PC2 跨 VLAN 应通: {}",
        out
    );
}

fn run_and_assert_prompt(eng: &Engine, sess: &Session, dev: u32, expect: &str) {
    assert_eq!(prompt(eng, sess, dev), expect);
}

// ---------- Comware ----------

#[test]
fn comware_vlan_flow() {
    let mut net = Net::new();
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Comware);
    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    let lines = [
        "system-view",
        "vlan 10",
        "int g1/0/1",
        "port link-type trunk",
        "port trunk permit vlan 10 20",
        "quit",
        "display vlan",
    ];
    let outs: Vec<String> = lines
        .iter()
        .map(|l| exec_and_run(&mut eng, &mut sess, sw, l))
        .collect();

    // trunk 默认仅 VLAN1，permit 后放行 10/20（保留 1）
    let p = &eng.net.device(sw).unwrap().ports[0];
    assert_eq!(
        p.trunk_allowed,
        Some(vec![1, 10, 20]),
        "Comware permit 应保留默认 VLAN1"
    );

    // display vlan 快照（permit 的 VLAN 自动登记进 VLAN 表）
    assert_eq!(
        outs[6],
        "  VLAN ID Name                    Ports\n  \
         --------------------------------------------------------------------\n  \
         1       default                 GE1/0/1(U) GE1/0/2(U) GE1/0/3(U) GE1/0/4(U) GE1/0/5(U) GE1/0/6(U) GE1/0/7(U) GE1/0/8(U)\n  \
         10      VLAN0010                GE1/0/1(T)\n  \
         20      VLAN0020                GE1/0/1(T)"
    );

    // mac 表（空）
    let mac = exec_and_run(&mut eng, &mut sess, sw, "display mac-address");
    assert!(mac.contains("MAC Address Table"), "{}", mac);
}

#[test]
fn ios_trunk_between_switches_golden() {
    // IOS：trunk 默认放行全部 → 两台交换机间同 VLAN 直通
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw2, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);

    let mut eng = Engine::new(net);
    let mut s1 = Session::default();
    for l in [
        "enable",
        "conf t",
        "vlan 10",
        "int g0/0",
        "switchport access vlan 10",
        "exit",
        "int g0/1",
        "switchport mode trunk",
        "end",
    ] {
        exec_and_run(&mut eng, &mut s1, sw1, l);
    }
    let mut s2 = Session::default();
    for l in [
        "enable",
        "conf t",
        "vlan 10",
        "int g0/0",
        "switchport mode trunk",
        "exit",
        "int g0/1",
        "switchport access vlan 10",
        "end",
    ] {
        exec_and_run(&mut eng, &mut s2, sw2, l);
    }
    // trunk_allowed = None（全部放行）
    assert_eq!(eng.net.device(sw1).unwrap().ports[1].trunk_allowed, None);
    let out = exec_and_run(&mut eng, &mut Session::default(), pc1, "ping 192.168.10.12");
    assert!(
        out.contains("5 packets transmitted, 5 received"),
        "IOS trunk 默认全放行: {}",
        out
    );
}
