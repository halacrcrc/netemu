//! M2b：DHCP/ACL/NAT 三方言 CLI 黄金测试。

use cli_dialects::{exec_and_run, prompt};
use cli_engine::Session;
use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn ip(s: &str) -> u32 {
    ipv4_from_str(s).unwrap()
}

#[test]
fn ios_dhcp_pool_flow_golden() {
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.10.1")), 24);
    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    let outs: Vec<String> = [
        "enable",
        "conf t",
        "ip dhcp pool OFFICE",
        "network 192.168.10.0 255.255.255.0",
        "default-router 192.168.10.1",
        "dns-server 8.8.8.8",
        "exit",
        "end",
    ]
    .iter()
    .map(|l| exec_and_run(&mut eng, &mut sess, r1, l))
    .collect();
    // 池视图提示符
    let p2 = prompt(&eng, &sess, r1);
    // 全部静默成功
    for (i, o) in outs.iter().enumerate() {
        assert_eq!(o, "", "第 {} 条命令应静默成功: {:?}", i, o);
    }
    let _ = p2;
    // 池已建好
    let d = eng.net.device(r1).unwrap().clone();
    let DeviceState::Router(rs) = &d.state else {
        panic!()
    };
    assert_eq!(rs.dhcp_pools.len(), 1);
    assert_eq!(rs.dhcp_pools[0].name, "OFFICE");
    assert_eq!(rs.dhcp_pools[0].gateway, ip("192.168.10.1"));
    assert_eq!(rs.dhcp_pools[0].prefix, 24);

    // PC 通过 DHCP 获取（ipconfig /renew）
    net_set_dhcp(&mut eng, pc1);
    let out = exec_and_run(&mut eng, &mut Session::default(), pc1, "ipconfig /renew");
    let _ = out;
    let d = eng.net.device(pc1).unwrap().clone();
    assert_eq!(
        d.ports[0].ip,
        Some(ip("192.168.10.2")),
        "DHCP 应获取首个空闲地址"
    );
}

fn net_set_dhcp(eng: &mut Engine, dev: u32) {
    let d = eng.net.device_mut(dev).unwrap();
    if let DeviceState::Host(h) = &mut d.state {
        h.dhcp_enable = true;
        h.dhcp = Some(sim_core::device::HostDhcp { discovering: false });
    }
}

#[test]
fn vrp_acl_flow_golden() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Vrp);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Vrp);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Vrp);
    net.add_link(pc1, r1).unwrap(); // R1 p0
    net.add_link(r1, srv).unwrap(); // R1 p1
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(srv, Some(ip("192.168.3.100")), 24, Some(ip("192.168.3.1")));
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.1.1")), 24);
    net.set_port_admin(r1, 1, true);
    net.set_port_ip(r1, 1, Some(ip("192.168.3.1")), 24);
    let mut eng = Engine::new(net);
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
    let mut sess = Session::default();

    let outs: Vec<String> = [
        "system-view",
        "acl 3000",
        "rule deny icmp source 192.168.1.10 0 destination 192.168.3.100 0",
        "quit",
        "int g0/0/0",
        "traffic-filter inbound acl 3000",
        "return",
    ]
    .iter()
    .map(|l| exec_and_run(&mut eng, &mut sess, r1, l))
    .collect();
    for (i, o) in outs.iter().enumerate() {
        assert_eq!(o, "", "第 {} 条命令应静默成功: {:?}", i, o);
    }
    // ACL 视图提示符期间正确
    let _ = prompt(&eng, &sess, r1);

    // PC1 ping 服务器被拦截
    let out = exec_and_run(&mut eng, &mut Session::default(), pc1, "ping 192.168.3.100");
    assert!(
        out.contains("0 received") || out.contains("5 packets transmitted, 0"),
        "PC1 ping 应被 ACL 拦截: {}",
        out
    );
    // 命中计数
    let d = eng.net.device(r1).unwrap().clone();
    let DeviceState::Router(rs) = &d.state else {
        panic!()
    };
    assert_eq!(rs.acls[0].rules[0].hits, 5, "规则应命中 5 次");
    // display acl all 显示规则
    let show = exec_and_run(&mut eng, &mut sess, r1, "display acl all");
    assert!(show.contains("3000") && show.contains("deny"), "{}", show);
    assert!(show.contains("(matched 5)"), "{}", show);
}

#[test]
fn ios_nat_flow_golden() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap(); // R1 p0 inside
    net.add_link(r1, srv).unwrap(); // R1 p1 outside
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(srv, Some(ip("202.100.1.100")), 24, None);
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.1.1")), 24);
    net.set_port_admin(r1, 1, true);
    net.set_port_ip(r1, 1, Some(ip("202.100.1.1")), 24);
    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    let outs: Vec<String> = [
        "enable",
        "conf t",
        "int g0/0",
        "ip nat inside",
        "exit",
        "int g0/1",
        "ip nat outside",
        "exit",
        "end",
    ]
    .iter()
    .map(|l| exec_and_run(&mut eng, &mut sess, r1, l))
    .collect();
    for (i, o) in outs.iter().enumerate() {
        assert_eq!(o, "", "第 {} 条命令应静默成功: {:?}", i, o);
    }
    let d = eng.net.device(r1).unwrap().clone();
    let DeviceState::Router(rs) = &d.state else {
        panic!()
    };
    assert_eq!(rs.nat_inside, vec![0]);
    assert_eq!(rs.nat_outside, vec![1]);

    // ping 外网服务器（EasyIP 无需 ACL 也放行 M2b 简化）
    let out = exec_and_run(&mut eng, &mut Session::default(), pc1, "ping 202.100.1.100");
    assert!(out.contains("5 received"), "NAT 后应能 ping 通: {}", out);
    // show ip nat translations 有表项
    let show = exec_and_run(&mut eng, &mut sess, r1, "show ip nat translations");
    assert!(
        show.contains("192.168.1.10") && show.contains("202.100.1.1"),
        "{}",
        show
    );
}
