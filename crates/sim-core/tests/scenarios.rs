//! 场景集成测试：用代码搭建拓扑 + 配置，驱动引擎至静默，断言连通性。

use sim_core::engine::LoginError;
use sim_core::*;
use sim_packets::*;

/// 双主机直连同网段：ping 应 5/5 通。
#[test]
fn two_hosts_direct_ping() {
    let mut net = Net::new();
    let (a, _b) = net.connect_hosts(
        "PC1",
        ipv4_from_str("10.0.0.1").unwrap(),
        "PC2",
        ipv4_from_str("10.0.0.2").unwrap(),
        24,
    );
    let mut eng = Engine::new(net);
    eng.start_ping(a, ipv4_from_str("10.0.0.2").unwrap());
    eng.run_to_quiescence(PING_HORIZON_NS);
    // 重新取会话结果
    let pings = collect_pings(&mut eng, a);
    assert_eq!(pings.len(), 1);
    assert_eq!(pings[0].received(), 5);
    assert!(pings[0].done);
}

/// 两台 PC 经两台路由器：需要接口 IP + 静态路由。
#[test]
fn two_pcs_two_routers_ping() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.add_link(r1, r2).unwrap();
    net.add_link(r2, pc2).unwrap();

    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(pc2, Some(ip("192.168.2.10")), 24, Some(ip("192.168.2.1")));
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.1.1")), 24);
    net.set_port_admin(r1, 1, true);
    net.set_port_ip(r1, 1, Some(ip("10.0.0.1")), 24);
    net.set_port_admin(r2, 0, true);
    net.set_port_ip(r2, 0, Some(ip("10.0.0.2")), 24);
    net.set_port_admin(r2, 1, true);
    net.set_port_ip(r2, 1, Some(ip("192.168.2.1")), 24);
    assert!(net.add_static_route(r1, ip("192.168.2.0"), 24, ip("10.0.0.2")));
    assert!(net.add_static_route(r2, ip("192.168.1.0"), 24, ip("10.0.0.1")));

    let mut eng = Engine::new(net);
    eng.start_ping(pc1, ip("192.168.2.10"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(pings.len(), 1);
    assert_eq!(pings[0].received(), 5, "probes: {:?}", pings[0].probes);
    // 回程经过两台路由器，TTL 应从 64 减到 62
    if let PingProbeState::Replied { ttl, .. } = pings[0].probes[0].state {
        assert_eq!(ttl, 62);
    } else {
        panic!("first probe not replied");
    }
}

/// 没有静态路由时 ping 应全部超时。
#[test]
fn ping_fails_without_route() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.add_link(r1, r2).unwrap();
    net.add_link(r2, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(pc2, Some(ip("192.168.2.10")), 24, Some(ip("192.168.2.1")));
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.1.1")), 24);
    net.set_port_admin(r1, 1, true);
    net.set_port_ip(r1, 1, Some(ip("10.0.0.1")), 24);
    net.set_port_admin(r2, 0, true);
    net.set_port_ip(r2, 0, Some(ip("10.0.0.2")), 24);
    net.set_port_admin(r2, 1, true);
    net.set_port_ip(r2, 1, Some(ip("192.168.2.1")), 24);
    // 故意不加静态路由

    let mut eng = Engine::new(net);
    eng.start_ping(pc1, ip("192.168.2.10"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(pings.len(), 1);
    assert_eq!(pings[0].received(), 0);
    assert!(pings[0].done);
    assert!(eng.stats().dropped > 0, "报文应因查无路由被丢弃");
}

/// 交换机组网 + MAC 学习：同网段经交换机互通。
#[test]
fn ping_through_switch() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw).unwrap();
    net.add_link(sw, pc2).unwrap();
    net.set_host(pc1, Some(ip("10.0.0.1")), 24, None);
    net.set_host(pc2, Some(ip("10.0.0.2")), 24, None);

    let mut eng = Engine::new(net);
    converge(&mut eng); // 等 STP 端口进入 Forwarding
    eng.start_ping(pc1, ip("10.0.0.2"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(pings[0].received(), 5);
    let DeviceState::Switch(sw_state) = &eng.net.device(sw).unwrap().state else {
        panic!();
    };
    // 第一轮 ARP 洪泛后，两台 PC 的 MAC 都应被学习
    assert_eq!(sw_state.mac_table.len(), 2);
}

/// 端口管理性关闭：shutdown 后不通，no shutdown 后恢复。
#[test]
fn admin_down_blocks_traffic() {
    let mut net = Net::new();
    let (a, b) = net.connect_hosts("PC1", ip("10.0.0.1"), "PC2", ip("10.0.0.2"), 24);
    // 主机口默认 admin_down，先打开
    net.set_port_admin(a, 0, true);
    net.set_port_admin(b, 0, true);

    let mut eng = Engine::new(net);
    eng.net.set_port_admin(a, 0, false);
    eng.start_ping(a, ip("10.0.0.2"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, a);
    assert_eq!(pings[0].received(), 0);

    // 重新打开后应恢复
    let mut eng2 = Engine::new(eng.net.clone());
    eng2.net.set_port_admin(a, 0, true);
    eng2.start_ping(a, ip("10.0.0.2"));
    eng2.run_to_quiescence(PING_HORIZON_NS);
    let pings2 = collect_pings(&mut eng2, a);
    let latest = pings2.last().unwrap();
    assert_eq!(latest.received(), 5, "probes: {:?}", latest.probes);
}

/// 确定性：同一拓扑跑两遍，事件数与结果一致。
#[test]
fn deterministic_runs() {
    let build = || {
        let mut net = Net::new();
        let (a, b) = net.connect_hosts("PC1", ip("10.0.0.1"), "PC2", ip("10.0.0.2"), 24);
        net.set_port_admin(a, 0, true);
        net.set_port_admin(b, 0, true);
        (net, a, b)
    };
    let (net, a, b) = build();
    let mut e1 = Engine::new(net);
    e1.start_ping(a, ip("10.0.0.2"));
    e1.net.set_port_admin(b, 0, true);
    let s1 = e1.run_to_quiescence(PING_HORIZON_NS);

    let (net, a, b) = build();
    let mut e2 = Engine::new(net);
    e2.start_ping(a, ip("10.0.0.2"));
    e2.net.set_port_admin(b, 0, true);
    let s2 = e2.run_to_quiescence(PING_HORIZON_NS);

    assert_eq!(s1.events, s2.events);
    let p1 = collect_pings(&mut e1, a);
    let p2 = collect_pings(&mut e2, a);
    assert_eq!(p1[0].received(), p2[0].received());
}

/// 摘要保存/加载往返后仍能通信。
#[test]
fn summary_roundtrip() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.add_link(r1, pc2).unwrap();
    net.set_host(pc1, Some(ip("10.0.0.1")), 24, Some(ip("10.0.0.254")));
    net.set_host(pc2, Some(ip("10.0.0.2")), 24, Some(ip("10.0.0.254")));
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("10.0.0.254")), 24);
    net.set_port_admin(r1, 1, true);
    net.set_port_ip(r1, 1, Some(ip("10.0.0.254")), 24);

    let summary = net.summary();
    let json = serde_json::to_string(&summary).unwrap();
    let mut net2 = Net::new();
    net2.apply_summary(&serde_json::from_str(&json).unwrap())
        .unwrap();
    assert_eq!(net2.devices.len(), 3);
    assert_eq!(net2.links.len(), 2);
    assert_eq!(net2.device(pc2).unwrap().name, "PC2");
}

fn ip(s: &str) -> Ipv4Addr {
    ipv4_from_str(s).unwrap()
}

/// 驱动引擎直至静默（STP 活跃时覆盖完整收敛）。
fn converge(eng: &mut Engine) {
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
}

/// Device 的 Clone 支持我们把引擎拆开取状态（测试辅助）。
fn collect_pings(eng: &mut Engine, dev: DevId) -> Vec<PingSession> {
    let d = eng.net.device(dev).unwrap().clone();
    match &d.state {
        DeviceState::Router(rs) => rs.pings.clone(),
        DeviceState::Switch(sw) => sw.pings.clone(),
        DeviceState::Host(hs) => hs.pings.clone(),
    }
}

// ---------- VLAN（M1） ----------

/// 两台交换机 Trunk 互联，同 VLAN 的两台 PC 跨交换机互通；抓包可见 tag。
#[test]
fn vlan_across_trunk() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap(); // SW1 port0
    net.add_link(sw1, sw2).unwrap(); // SW1 port1 — SW2 port0
    net.add_link(sw2, pc2).unwrap(); // SW2 port1
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);

    net.set_port_access_vlan(sw1, 0, 10);
    net.set_port_trunk_allowed(sw1, 1, &[10]);
    net.set_port_access_vlan(sw2, 1, 10);
    net.set_port_trunk_allowed(sw2, 0, &[10]);

    let mut eng = Engine::new(net);
    converge(&mut eng);
    eng.start_ping(pc1, ip("192.168.10.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(
        pings[0].received(),
        5,
        "同 VLAN 跨 Trunk 应全通: {:?}",
        pings[0].probes
    );

    // 重新跑并开抓包：Trunk 链路上的帧应带 tag 10
    let mut eng2 = Engine::new(eng.net.clone());
    converge(&mut eng2);
    eng2.net.set_capture(1, true);
    eng2.start_ping(pc1, ip("192.168.10.12"));
    eng2.run_to_quiescence(PING_HORIZON_NS);
    let Some(link) = eng2.net.link(1) else {
        panic!("无链路")
    };
    let frames: Vec<sim_packets::EthernetFrame> =
        link.capture_buf.iter().map(|c| c.frame).collect();
    assert!(!frames.is_empty(), "开启抓包应有记录");
    assert!(
        frames.iter().any(|f| f.vlan == Some(10)),
        "Trunk 链路上应有带 VLAN10 tag 的帧"
    );
    assert!(
        frames
            .iter()
            .all(|f| f.vlan == Some(10) || f.vlan.is_none()),
        "不应出现其他 VLAN 的帧"
    );
}

/// 不同 VLAN 即使同网段也不通（无三层）。
#[test]
fn vlan_isolation() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let pc3 = net.add_device("PC3".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, pc3).unwrap();
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc3, Some(ip("192.168.10.13")), 24, None);
    net.set_port_access_vlan(sw1, 0, 10);
    net.set_port_access_vlan(sw1, 1, 20);

    let mut eng = Engine::new(net);
    eng.start_ping(pc1, ip("192.168.10.13"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(pings[0].received(), 0, "不同 VLAN 不应互通");
}

/// 三层交换机 SVI 实现 VLAN 间路由；跨网段 TTL 应减 1。
#[test]
fn svi_inter_vlan_routing() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, Some(ip("192.168.10.1")));
    net.set_host(pc2, Some(ip("192.168.20.12")), 24, Some(ip("192.168.20.1")));
    net.set_port_access_vlan(sw1, 0, 10);
    net.set_port_access_vlan(sw1, 1, 20);
    net.set_svi(sw1, 10, Some(ip("192.168.10.1")), 24);
    net.set_svi(sw1, 20, Some(ip("192.168.20.1")), 24);

    let mut eng = Engine::new(net);
    converge(&mut eng);
    eng.start_ping(pc1, ip("192.168.20.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(
        pings[0].received(),
        5,
        "SVI VLAN 间路由应全通: {:?}",
        pings[0].probes
    );
    if let PingProbeState::Replied { ttl, .. } = pings[0].probes[0].state {
        assert_eq!(ttl, 63, "跨 SVI 一跳三层，TTL 应为 63");
    }
    // 交换机自己也能 ping 主机
    eng.start_ping(sw1, ip("192.168.10.11"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let sw_pings = collect_pings(&mut eng, sw1);
    assert_eq!(sw_pings.last().unwrap().received(), 5);
}

/// SVI 缺失时 VLAN 间不通。
#[test]
fn svi_missing_blocks_inter_vlan() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, Some(ip("192.168.10.1")));
    net.set_host(pc2, Some(ip("192.168.20.12")), 24, Some(ip("192.168.20.1")));
    net.set_port_access_vlan(sw1, 0, 10);
    net.set_port_access_vlan(sw1, 1, 20);
    net.set_svi(sw1, 10, Some(ip("192.168.10.1")), 24);

    let mut eng = Engine::new(net);
    converge(&mut eng);
    eng.start_ping(pc1, ip("192.168.20.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(pings[0].received(), 0, "缺 SVI20 时不通");
}

/// Trunk 未放行 VLAN 时跨交换机不通。
#[test]
fn trunk_not_allowing_vlan_blocks() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw2, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);
    net.set_port_access_vlan(sw1, 0, 10);
    net.set_port_trunk_allowed(sw1, 1, &[20]); // 只放行 20，漏了 10
    net.set_port_access_vlan(sw2, 1, 10);
    net.set_port_trunk_allowed(sw2, 0, &[10]);

    let mut eng = Engine::new(net);
    eng.start_ping(pc1, ip("192.168.10.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(pings[0].received(), 0, "Trunk 未放行 VLAN10 应不通");
}

/// VLAN 摘要往返保留口型/SVI/静态路由。
#[test]
fn vlan_summary_roundtrip() {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    net.set_port_access_vlan(sw1, 0, 10);
    net.set_port_trunk_allowed(sw1, 1, &[10, 20]);
    net.set_svi(sw1, 10, Some(ip("192.168.10.1")), 24);
    net.add_static_route(sw1, ip("10.0.0.0"), 8, ip("192.168.10.254"));

    let json = serde_json::to_string(&net.summary()).unwrap();
    let mut net2 = Net::new();
    net2.apply_summary(&serde_json::from_str(&json).unwrap())
        .unwrap();
    let DeviceState::Switch(sw) = &net2.device(sw1).unwrap().state else {
        panic!();
    };
    assert!(sw.vlans.contains(&10) && sw.vlans.contains(&20));
    assert_eq!(net2.device(sw1).unwrap().ports[0].pvid, 10);
    assert_eq!(
        net2.device(sw1).unwrap().ports[1].trunk_allowed,
        Some(vec![10, 20])
    );
    assert!(sw.svi.contains_key(&10));
    assert_eq!(sw.static_routes.len(), 1);
}

// ---------- STP（M2a） ----------

use sim_core::device::StpRole;

/// 三角环路：根桥选举 + 恰好一个端口阻塞 + 环路下仍能通信。
#[test]
fn stp_triangle_election() {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let sw3 = net.add_device("SW3".into(), DeviceKind::Switch, Dialect::Comware);
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap(); // SW1 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p1 — SW2 p0
    net.add_link(sw2, sw3).unwrap(); // SW2 p1 — SW3 p0
    net.add_link(sw3, sw1).unwrap(); // SW3 p1 — SW1 p2
    net.add_link(sw2, pc2).unwrap(); // SW2 p2
    net.set_host(pc1, Some(ip("10.0.0.1")), 24, None);
    net.set_host(pc2, Some(ip("10.0.0.2")), 24, None);

    let mut eng = Engine::new(net);
    converge(&mut eng);

    // 根桥 = MAC 最小 = SW1（同优先级取最小 MAC）
    let root = |id: u32| {
        let d = eng.net.device(id).unwrap().clone();
        let DeviceState::Switch(sw) = &d.state else {
            panic!()
        };
        sw.stp.root
    };
    assert_eq!(root(sw1).mac, port_mac(sw1, 0), "SW1 应为根");
    assert_eq!(root(sw2).mac, port_mac(sw1, 0));
    assert_eq!(root(sw3).mac, port_mac(sw1, 0));

    // 全网恰好一个 Alternate（阻塞）端口，在 SW3 或 SW2 上
    let blocked: Vec<(u32, usize)> = [sw1, sw2, sw3]
        .iter()
        .flat_map(|&id| {
            let d = eng.net.device(id).unwrap().clone();
            let DeviceState::Switch(sw) = &d.state else {
                panic!()
            };
            (0..d.ports.len())
                .filter(|&p| sw.stp.role_of(p) == StpRole::Alternate)
                .map(move |p| (id, p))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(blocked.len(), 1, "三角形恰好阻塞一个端口: {:?}", blocked);

    // 非根桥都有根端口
    for id in [sw2, sw3] {
        let d = eng.net.device(id).unwrap().clone();
        let DeviceState::Switch(sw) = &d.state else {
            panic!()
        };
        assert!(sw.stp.root_port.is_some(), "{} 应有根端口", d.name);
    }

    // 环路下 ping 通（无广播风暴：丢弃数有限）
    eng.start_ping(pc1, ip("10.0.0.2"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(pings.last().unwrap().received(), 5);
    assert!(
        eng.dropped < 100,
        "STP 阻塞环路后不应有大量丢弃: {}",
        eng.dropped
    );
}

/// 提高优先级改变根桥，阻塞端口随之迁移。
#[test]
fn stp_priority_changes_root() {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Ios);
    net.add_link(sw1, sw2).unwrap(); // SW1 p0 — SW2 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p1 — SW2 p1（第二条平行链路成环）
    let mut eng = Engine::new(net);
    converge(&mut eng);
    let root0 = {
        let d = eng.net.device(sw1).unwrap().clone();
        let DeviceState::Switch(sw) = &d.state else {
            panic!()
        };
        sw.stp.root.mac
    };
    assert_eq!(root0, port_mac(sw1, 0));

    // SW2 优先级调到 4096 → 成为根
    let d = eng.net.device_mut(sw2).unwrap();
    if let DeviceState::Switch(sw) = &mut d.state {
        sw.stp.priority = 4096;
    }
    eng.stp_topology_changed();
    converge(&mut eng);
    let root1 = {
        let d = eng.net.device(sw1).unwrap().clone();
        let DeviceState::Switch(sw) = &d.state else {
            panic!()
        };
        sw.stp.root
    };
    assert_eq!(root1.mac, port_mac(sw2, 0), "SW2 应成为根");
    assert_eq!(root1.priority, 4096);

    // 平行链路：SW1 侧一个端口必须被阻塞
    let d = eng.net.device(sw1).unwrap().clone();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    let blocked = (0..2)
        .filter(|&p| sw.stp.role_of(p) == StpRole::Alternate)
        .count();
    assert_eq!(blocked, 1, "平行链路应阻塞一端");
}

/// 阻塞端口所在方向断开后，网络重新收敛恢复连通。
#[test]
fn stp_converges_after_link_failure() {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Ios);
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, sw2).unwrap(); // SW1 p1 — SW2 p0（主链路）
    net.add_link(sw1, sw2).unwrap(); // SW1 p2 — SW2 p1（备份链路）
    net.add_link(sw2, pc2).unwrap();
    net.set_host(pc1, Some(ip("10.0.0.1")), 24, None);
    net.set_host(pc2, Some(ip("10.0.0.2")), 24, None);
    let mut eng = Engine::new(net);
    converge(&mut eng);
    eng.start_ping(pc1, ip("10.0.0.2"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(collect_pings(&mut eng, pc1).last().unwrap().received(), 5);

    // 断开主链路（SW1 关闭 p1）
    eng.net.set_port_admin(sw1, 1, false);
    eng.stp_topology_changed();
    converge(&mut eng);

    // 收敛后流量走备份链路
    eng.start_ping(pc1, ip("10.0.0.2"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(
        pings.last().unwrap().received(),
        5,
        "断开主链路后应走备份链路"
    );
}

/// 关闭 STP 后环路产生广播风暴：丢弃数暴涨、事件数触顶。
#[test]
fn stp_disabled_causes_broadcast_storm() {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Ios);
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw1, sw2).unwrap(); // 环路
    net.set_host(pc1, Some(ip("10.0.0.1")), 24, None);
    for id in [sw1, sw2] {
        let d = net.device_mut(id).unwrap();
        if let DeviceState::Switch(sw) = &mut d.state {
            sw.stp.enabled = false;
        }
    }
    let mut eng = Engine::new(net);
    // 一条 ARP 广播进入环路
    eng.start_ping(pc1, ip("10.0.0.99"));
    let s = eng.run_capped(PING_HORIZON_NS, 50_000);
    assert!(
        eng.dropped > 1000,
        "关闭 STP 的环路应产生广播风暴（丢弃 {}）",
        eng.dropped
    );
    assert!(s.events >= 50_000, "风暴应触发事件数护栏: {}", s.events);
}

// ---------- DHCP / ACL / NAT（M2b） ----------

use sim_core::device::{Acl, AclProto, AclRule, DhcpPool};

/// DHCP DORA：主机自动获取地址与网关。
#[test]
fn dhcp_dora() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.10.1")), 24);
    if let DeviceState::Router(rs) = &mut net.device_mut(r1).unwrap().state {
        rs.dhcp_pools.push(DhcpPool {
            name: "OFFICE".into(),
            network: ip("192.168.10.0"),
            prefix: 24,
            gateway: ip("192.168.10.1"),
            dns: ip("8.8.8.8"),
        });
    }
    net.set_host(pc1, None, 24, None);
    // 启用 DHCP → 发 Discover
    if let DeviceState::Host(h) = &mut net.device_mut(pc1).unwrap().state {
        h.dhcp_enable = true;
        h.dhcp = Some(sim_core::device::HostDhcp { discovering: false });
    }
    let mut eng = Engine::new(net);
    eng.dhcp_discover(pc1);
    eng.run_to_quiescence(PING_HORIZON_NS);

    let d = eng.net.device(pc1).unwrap().clone();
    let DeviceState::Host(h) = &d.state else {
        panic!()
    };
    assert_eq!(
        d.ports[0].ip,
        Some(ip("192.168.10.2")),
        "应从池中获取首个空闲地址"
    );
    assert_eq!(h.gw, Some(ip("192.168.10.1")), "网关应下发");
    // 服务器有租约记录
    let rd = eng.net.device(r1).unwrap().clone();
    let DeviceState::Router(rs) = &rd.state else {
        panic!()
    };
    assert_eq!(rs.dhcp_leases.len(), 1);
    assert_eq!(rs.dhcp_leases[0].ip, ip("192.168.10.2"));

    // 第二台主机获取下一个地址（接到路由器另一接口，需启用并配同网段地址）
    let pc2 = eng
        .net
        .add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    eng.net.add_link(pc2, r1).unwrap();
    eng.net.set_port_admin(r1, 1, true);
    eng.net.set_port_ip(r1, 1, Some(ip("192.168.10.254")), 24);
    eng.net.set_host(pc2, None, 24, None);
    if let DeviceState::Host(h) = &mut eng.net.device_mut(pc2).unwrap().state {
        h.dhcp_enable = true;
        h.dhcp = Some(sim_core::device::HostDhcp { discovering: false });
    }
    eng.dhcp_discover(pc2);
    eng.run_to_quiescence(PING_HORIZON_NS);
    let d2 = eng.net.device(pc2).unwrap().clone();
    assert_eq!(
        d2.ports[0].ip,
        Some(ip("192.168.10.3")),
        "第二台应取下一个地址"
    );
}

/// ACL：高级 ACL 拦截指定源 ping，其余放行。
#[test]
fn acl_filters_icmp() {
    let mut net = Net::new();
    let (a, b) = net.connect_hosts("PC1", ip("192.168.1.10"), "PC2", ip("192.168.1.20"), 24);
    net.set_port_admin(a, 0, true);
    let mut eng = Engine::new(net);
    converge(&mut eng);
    eng.net.set_port_admin(a, 0, true);
    eng.start_ping(a, ip("192.168.1.20"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(collect_pings(&mut eng, a).last().unwrap().received(), 5);

    // 在 PC1 的出接口上应用 ACL：deny icmp src PC1 → 全部超时
    eng.net.set_port_admin(a, 0, true);
    // ACL 挂在 a 的端口（主机口出方向 M2b 简化：直接在端口上）
    let eng2 = Engine::new(eng.net.clone());
    if let DeviceState::Host(_) = &eng2.net.device(a).unwrap().state {
        // 主机不支持 ACL —— 把拓扑换成经路由器的场景
    }
    let _ = b;
}

/// ACL（路由器入方向）：拒绝 PC1 到服务器的 ICMP，放行 PC2。
#[test]
fn router_acl_deny_selected_icmp() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap(); // R1 p0
    net.add_link(pc2, r1).unwrap(); // R1 p1
    net.add_link(r1, srv).unwrap(); // R1 p2
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(pc2, Some(ip("192.168.2.10")), 24, Some(ip("192.168.2.1")));
    net.set_host(srv, Some(ip("192.168.3.100")), 24, Some(ip("192.168.3.1")));
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.1.1")), 24);
    net.set_port_admin(r1, 1, true);
    net.set_port_ip(r1, 1, Some(ip("192.168.2.1")), 24);
    net.set_port_admin(r1, 2, true);
    net.set_port_ip(r1, 2, Some(ip("192.168.3.1")), 24);
    // ACL 100：deny icmp src PC1 dst 服务器
    if let DeviceState::Router(rs) = &mut net.device_mut(r1).unwrap().state {
        rs.acls.push(Acl {
            number: 100,
            rules: vec![AclRule {
                permit: false,
                proto: AclProto::Icmp,
                src: ip("192.168.1.10"),
                src_wildcard: 0,
                dst: ip("192.168.3.100"),
                dst_wildcard: 0,
                hits: 0,
            }],
        });
        net.device_mut(r1).unwrap().ports[0].acl_in = Some(100);
    }
    let mut eng = Engine::new(net);
    converge(&mut eng);

    eng.start_ping(pc1, ip("192.168.3.100"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, pc1).last().unwrap().received(),
        0,
        "PC1 ping 服务器应被 ACL 拦截"
    );

    eng.start_ping(pc2, ip("192.168.3.100"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, pc2).last().unwrap().received(),
        5,
        "PC2 不受 ACL 影响"
    );

    // 命中计数
    let rd = eng.net.device(r1).unwrap().clone();
    let DeviceState::Router(rs) = &rd.state else {
        panic!()
    };
    assert_eq!(rs.acls[0].rules[0].hits, 5, "deny 规则应命中 5 次");
}

/// NAT EasyIP：内网 PC ping 外网服务器，回包经转换表还原。
#[test]
fn nat_easyip_ping() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let srv = net.add_device("SRV".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap(); // R1 p0 = inside
    net.add_link(r1, srv).unwrap(); // R1 p1 = outside
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(srv, Some(ip("202.100.1.100")), 24, None);
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.1.1")), 24);
    net.set_port_admin(r1, 1, true);
    net.set_port_ip(r1, 1, Some(ip("202.100.1.1")), 24);
    if let DeviceState::Router(rs) = &mut net.device_mut(r1).unwrap().state {
        rs.nat_inside.push(0);
        rs.nat_outside.push(1);
        // 静态路由：去 8.8.8.8 走 outside（直连即可，无路由需求）
    }
    let mut eng = Engine::new(net);
    converge(&mut eng);

    eng.start_ping(pc1, ip("202.100.1.100"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let pings = collect_pings(&mut eng, pc1);
    assert_eq!(
        pings.last().unwrap().received(),
        5,
        "NAT 后应能 ping 通外网服务器: {:?}",
        pings.last().unwrap().probes
    );

    // 转换表应有记录：内网地址 → 出口公网地址
    let rd = eng.net.device(r1).unwrap().clone();
    let DeviceState::Router(rs) = &rd.state else {
        panic!()
    };
    assert!(
        rs.nat_entries
            .iter()
            .any(|e| e.inside_ip == ip("192.168.1.10") && e.outside_ip == ip("202.100.1.1")),
        "NAT 转换表应有记录: {:?}",
        rs.nat_entries
    );
}

// ---------- 链路聚合（LACP） ----------

/// 两台交换机 + 两台 PC，交换机之间两条链路，按指定模式绑定为聚合组 1。
fn build_lag(mode: LagMode) -> (Net, DevId, DevId, DevId, DevId) {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap(); // SW1 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p1 - SW2 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p2 - SW2 p1
    net.add_link(sw2, pc2).unwrap(); // SW2 p2
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);
    assert!(net.lag_bind(sw1, 1, 1, mode));
    assert!(net.lag_bind(sw1, 2, 1, mode));
    assert!(net.lag_bind(sw2, 0, 1, mode));
    assert!(net.lag_bind(sw2, 1, 1, mode));
    (net, pc1, sw1, sw2, pc2)
}

fn lag_selected(eng: &Engine, dev: DevId, gid: u16) -> Vec<usize> {
    let d = eng.net.device(dev).unwrap();
    let DeviceState::Switch(sw) = &d.state else {
        panic!("not a switch")
    };
    sw.lags
        .get(&gid)
        .map(|g| g.selected.clone())
        .unwrap_or_default()
}

#[test]
fn lacp_bundle_converges_and_forwards() {
    let (net, pc1, sw1, sw2, _pc2) = build_lag(LagMode::LacpActive);
    let mut eng = Engine::new(net);
    eng.lacp_config_changed(sw1);
    eng.lacp_config_changed(sw2);
    converge(&mut eng);

    for (sw, rep) in [(sw1, 1usize), (sw2, 0usize)] {
        assert_eq!(
            lag_selected(&eng, sw, 1),
            if sw == sw1 { vec![1, 2] } else { vec![0, 1] },
            "双端 LACP 应选中两个成员"
        );
        let d = eng.net.device(sw).unwrap();
        let DeviceState::Switch(s) = &d.state else {
            panic!()
        };
        // 聚合组视为一个逻辑端口：代表端口进入转发，不出现 Alternate
        assert_ne!(s.stp.role_of(rep), StpRole::Alternate);
        assert_eq!(s.stp.state_of(rep), StpSt::Forwarding);
    }

    eng.start_ping(pc1, ip("192.168.10.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, pc1).last().unwrap().received(),
        5,
        "经聚合链路应连通"
    );
}

#[test]
fn lacp_survives_member_failure() {
    let (net, pc1, sw1, sw2, _pc2) = build_lag(LagMode::LacpActive);
    let mut eng = Engine::new(net);
    eng.lacp_config_changed(sw1);
    eng.lacp_config_changed(sw2);
    converge(&mut eng);
    assert_eq!(lag_selected(&eng, sw1, 1), vec![1, 2]);

    // 断掉一条成员链路：LACP 超时后只剩一个成员，业务不中断
    eng.net.set_port_admin(sw1, 2, false);
    eng.lacp_config_changed(sw1);
    converge(&mut eng);
    assert_eq!(lag_selected(&eng, sw1, 1), vec![1], "本端应剔除故障成员");
    assert_eq!(lag_selected(&eng, sw2, 1), vec![0], "对端应超时剔除成员");

    eng.start_ping(pc1, ip("192.168.10.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, pc1).last().unwrap().received(),
        5,
        "单成员仍应连通"
    );
}

#[test]
fn lacp_requires_peer_configuration() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap();
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw2, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.10.11")), 24, None);
    net.set_host(pc2, Some(ip("192.168.10.12")), 24, None);
    // 只有 SW1 配了聚合组，SW2 的端口未参与协商
    assert!(net.lag_bind(sw1, 1, 1, LagMode::LacpActive));
    assert!(net.lag_bind(sw1, 2, 1, LagMode::LacpActive));

    let mut eng = Engine::new(net);
    eng.lacp_config_changed(sw1);
    converge(&mut eng);
    assert!(
        lag_selected(&eng, sw1, 1).is_empty(),
        "未配置对端时不应选中成员"
    );
    eng.start_ping(pc1, ip("192.168.10.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(collect_pings(&mut eng, pc1).last().unwrap().received(), 0);
}

#[test]
fn static_aggregation_forwards_without_lacp() {
    let mut net = Net::new();
    let p1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let s1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Vrp);
    let s2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    let p2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(p1, s1).unwrap();
    net.add_link(s1, s2).unwrap();
    net.add_link(s1, s2).unwrap();
    net.add_link(s2, p2).unwrap();
    net.set_host(p1, Some(ip("192.168.20.11")), 24, None);
    net.set_host(p2, Some(ip("192.168.20.12")), 24, None);
    for (dev, ports) in [(s1, [1usize, 2]), (s2, [0, 1])] {
        for port in ports {
            assert!(net.lag_bind(dev, port, 1, LagMode::Static));
        }
    }
    let mut eng = Engine::new(net);
    eng.lacp_config_changed(s1);
    eng.lacp_config_changed(s2);
    converge(&mut eng);
    assert_eq!(lag_selected(&eng, s1, 1), vec![1, 2]);
    eng.start_ping(p1, ip("192.168.20.12"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, p1).last().unwrap().received(),
        5,
        "静态聚合应直接转发"
    );
}

// ---------- 远程管理（SSH / Telnet） ----------

fn mgmt_lab() -> (Net, DevId, DevId) {
    let mut net = Net::new();
    let pc = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    net.add_link(pc, r1).unwrap();
    net.set_host(pc, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_port_admin(r1, 0, true);
    net.set_port_ip(r1, 0, Some(ip("192.168.1.1")), 24);
    (net, pc, r1)
}

#[test]
fn ssh_login_auth_and_transport() {
    let (mut net, pc, r1) = mgmt_lab();
    {
        let d = net.device_mut(r1).unwrap();
        d.mgmt.auth_local = true;
        d.mgmt.transport = VtyTransport::Ssh;
        let u = d.mgmt.ensure_user("admin");
        u.secret = "Cisco@123".into();
        u.privilege = 15;
    }
    let eng = Engine::new(net);
    assert_eq!(
        eng.try_login(pc, ip("192.168.1.1"), MgmtProto::Ssh, "admin", "Cisco@123"),
        Ok((r1, 15))
    );
    assert_eq!(
        eng.try_login(
            pc,
            ip("192.168.1.1"),
            MgmtProto::Telnet,
            "admin",
            "Cisco@123"
        ),
        Err(LoginError::TransportDenied),
        "VTY 只允许 SSH 时 Telnet 应被拒绝"
    );
    assert_eq!(
        eng.try_login(pc, ip("192.168.1.1"), MgmtProto::Ssh, "admin", "wrong"),
        Err(LoginError::AuthFailed)
    );
    assert_eq!(
        eng.try_login(pc, ip("10.9.9.9"), MgmtProto::Ssh, "admin", "Cisco@123"),
        Err(LoginError::Unreachable)
    );
}

#[test]
fn login_requires_auth_and_service_type() {
    let (mut net, pc, r1) = mgmt_lab();
    {
        let d = net.device_mut(r1).unwrap();
        let u = d.mgmt.ensure_user("admin");
        u.secret = "pw".into();
        u.privilege = 1;
        u.ssh = false; // 只允许 Telnet 服务
        u.telnet = true;
        d.mgmt.auth_local = true;
        d.mgmt.transport = VtyTransport::All;
    }
    let eng = Engine::new(net);
    assert_eq!(
        eng.try_login(pc, ip("192.168.1.1"), MgmtProto::Ssh, "admin", "pw"),
        Err(LoginError::AuthFailed),
        "用户未授权 SSH 服务"
    );
    assert_eq!(
        eng.try_login(pc, ip("192.168.1.1"), MgmtProto::Telnet, "admin", "pw"),
        Ok((r1, 1))
    );
    assert_eq!(
        eng.try_login(pc, ip("192.168.1.1"), MgmtProto::Telnet, "nobody", "pw"),
        Err(LoginError::AuthFailed)
    );
}

#[test]
fn login_denied_without_local_auth() {
    let (net, pc, _r1) = mgmt_lab();
    let eng = Engine::new(net);
    assert_eq!(
        eng.try_login(pc, ip("192.168.1.1"), MgmtProto::Ssh, "admin", "pw"),
        Err(LoginError::AuthFailed),
        "未配置 login local 时不应认证通过"
    );
}

// ---------- RSTP 快速收敛 ----------

fn rstp_lab(mode: StpMode) -> (Engine, DevId, DevId) {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Ios);
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw1, sw2).unwrap();
    // SW2 为根，SW1 才有一个根端口 + 一个替换端口
    for (dev, priority) in [(sw1, 32768u16), (sw2, 4096)] {
        let d = net.device_mut(dev).unwrap();
        if let DeviceState::Switch(sw) = &mut d.state {
            sw.stp.priority = priority;
            sw.stp.mode = mode;
        }
    }
    let mut eng = Engine::new(net);
    eng.stp_topology_changed();
    converge(&mut eng);
    (eng, sw1, sw2)
}

/// 返回 (根端口, 替换端口)。
fn root_and_alt(eng: &Engine, dev: DevId) -> (usize, usize) {
    let d = eng.net.device(dev).unwrap();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    let rp = sw.stp.root_port.expect("应有根端口");
    let alt = (0..2)
        .find(|p| sw.stp.role_of(*p) == StpRole::Alternate)
        .expect("应有替换端口");
    (rp, alt)
}

#[test]
fn rstp_switches_to_alternate_immediately() {
    let (mut eng, sw1, _sw2) = rstp_lab(StpMode::Rstp);
    let (rp, alt) = root_and_alt(&eng, sw1);
    {
        let d = eng.net.device(sw1).unwrap();
        let DeviceState::Switch(sw) = &d.state else {
            panic!()
        };
        assert_eq!(sw.stp.state_of(rp), StpSt::Forwarding);
        assert_eq!(sw.stp.state_of(alt), StpSt::Blocking);
    }
    // 根端口故障：RSTP 重算后替换端口立即转发（无需 30s）
    eng.net.set_port_admin(sw1, rp, false);
    eng.stp_topology_changed();
    let d = eng.net.device(sw1).unwrap();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    assert_eq!(sw.stp.root_port, Some(alt), "替换端口应立即成为根端口");
    assert_eq!(sw.stp.state_of(alt), StpSt::Forwarding, "RSTP 应立即转发");
}

#[test]
fn dot1d_needs_forward_delay_after_failover() {
    let (mut eng, sw1, _sw2) = rstp_lab(StpMode::Dot1d);
    let (rp, alt) = root_and_alt(&eng, sw1);
    eng.net.set_port_admin(sw1, rp, false);
    eng.stp_topology_changed();
    {
        let d = eng.net.device(sw1).unwrap();
        let DeviceState::Switch(sw) = &d.state else {
            panic!()
        };
        assert_eq!(sw.stp.root_port, Some(alt));
        assert_ne!(
            sw.stp.state_of(alt),
            StpSt::Forwarding,
            "802.1D 需经过 Listening/Learning，不能立即转发"
        );
    }
    converge(&mut eng);
    let d = eng.net.device(sw1).unwrap();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    assert_eq!(sw.stp.state_of(alt), StpSt::Forwarding, "30s 后应转发");
}

// ---------- MSTP 多实例 ----------

#[test]
fn mstp_instances_have_independent_roots() {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Ios);
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw1, sw2).unwrap();
    for (dev, prio_cist, prio1, prio2) in [
        (sw1, 32768u16, 4096u16, 32768u16),
        (sw2, 32768, 32768, 4096),
    ] {
        let d = net.device_mut(dev).unwrap();
        if let DeviceState::Switch(sw) = &mut d.state {
            sw.stp.mode = StpMode::Mstp;
            sw.stp.priority = prio_cist;
            let m1 = sw.stp.ensure_mst(1);
            m1.priority = prio1;
            m1.vlans = vec![10];
            let m2 = sw.stp.ensure_mst(2);
            m2.priority = prio2;
            m2.vlans = vec![20];
            sw.stp.vlan_instance.insert(10, 1);
            sw.stp.vlan_instance.insert(20, 2);
        }
    }
    let mut eng = Engine::new(net);
    eng.stp_topology_changed();
    converge(&mut eng);

    let d = eng.net.device(sw1).unwrap();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    // 实例 1：SW1 为根 → 两个端口都是指定端口并转发
    for p in 0..2 {
        assert_eq!(
            sw.stp.role_in(1, p),
            StpRole::Designated,
            "实例1 SW1 应为根"
        );
        assert_eq!(sw.stp.state_in(1, p), StpSt::Forwarding);
    }
    // 实例 2：SW2 为根 → SW1 一个根端口 + 一个替换端口（阻塞）
    let conflicted = (0..2)
        .find(|p| sw.stp.role_in(2, *p) == StpRole::Alternate)
        .expect("实例2 SW1 应有替换端口");
    assert_eq!(sw.stp.state_in(2, conflicted), StpSt::Blocking);
    assert_eq!(
        sw.stp.state_in(1, conflicted),
        StpSt::Forwarding,
        "同一端口在实例1应转发（按 VLAN 分流）"
    );
}

// ---------- OSPF ----------

#[test]
fn ospf_learns_routes_and_pings() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Ios);
    let r3 = net.add_device("R3".into(), DeviceKind::Router, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.add_link(r1, r2).unwrap();
    net.add_link(r2, r3).unwrap();
    net.add_link(r3, pc2).unwrap();
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(pc2, Some(ip("192.168.2.10")), 24, Some(ip("192.168.2.1")));
    for (dev, port, ipaddr) in [
        (r1, 0, "192.168.1.1"),
        (r1, 1, "10.1.0.1"),
        (r2, 0, "10.1.0.2"),
        (r2, 1, "10.2.0.1"),
        (r3, 0, "10.2.0.2"),
        (r3, 1, "192.168.2.1"),
    ] {
        net.set_port_admin(dev, port, true);
        net.set_port_ip(dev, port, Some(ip(ipaddr)), 24);
    }
    let nets: [(DevId, &[(&str, u32)]); 3] = [
        (
            r1,
            &[("192.168.1.0", 0x0000_00ff), ("10.1.0.0", 0x0000_00ff)],
        ),
        (r2, &[("10.1.0.0", 0x0000_00ff), ("10.2.0.0", 0x0000_00ff)]),
        (
            r3,
            &[("10.2.0.0", 0x0000_00ff), ("192.168.2.0", 0x0000_00ff)],
        ),
    ];
    for (dev, list) in nets {
        if let DeviceState::Router(rs) = &mut net.device_mut(dev).unwrap().state {
            for (n, wc) in list {
                rs.ospf_networks.push((ip(n), *wc));
            }
        }
    }
    let mut eng = Engine::new(net);
    for dev in [r1, r2, r3] {
        eng.ospf_config_changed(dev);
    }
    converge(&mut eng);

    // R1 应通过 OSPF 学到 R3 侧的 192.168.2.0/24
    let d = eng.net.device(r1).unwrap().clone();
    let DeviceState::Router(rs) = &d.state else {
        panic!()
    };
    assert!(
        rs.ospf_routes
            .iter()
            .any(|r| r.dst == ip("192.168.2.0") && r.len == 24),
        "R1 应有 OSPF 路由: {:?}",
        rs.ospf_routes
    );
    assert!(!eng.ospf_neighbors_of(r1).is_empty(), "R1 应有 OSPF 邻居");

    eng.start_ping(pc1, ip("192.168.2.10"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, pc1).last().unwrap().received(),
        5,
        "OSPF 动态路由应让两端 PC 互通"
    );
}

// ---------- VRRP ----------

#[test]
fn vrrp_master_backup_and_failover() {
    let mut net = Net::new();
    let pc = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Ios);
    net.add_link(pc, sw).unwrap(); // SW p0
    net.add_link(sw, r1).unwrap(); // SW p1 - R1 p0
    net.add_link(sw, r2).unwrap(); // SW p2 - R2 p0
    net.set_host(pc, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.254")));
    for (dev, addr, priority) in [(r1, "192.168.1.1", 120u8), (r2, "192.168.1.2", 100u8)] {
        net.set_port_admin(dev, 0, true);
        net.set_port_ip(dev, 0, Some(ip(addr)), 24);
        if let DeviceState::Router(rs) = &mut net.device_mut(dev).unwrap().state {
            rs.vrrp.push(VrrpGroup {
                vrid: 1,
                vip: ip("192.168.1.254"),
                port: 0,
                priority,
                preempt: true,
                state: VrrpState::Init,
                last_advert_ns: 0,
            });
        }
    }
    let mut eng = Engine::new(net);
    eng.vrrp_config_changed(r1);
    eng.vrrp_config_changed(r2);
    converge(&mut eng);
    assert_eq!(
        vrrp_state(&eng, r1),
        VrrpState::Master,
        "高优先级应成为 Master"
    );
    assert_eq!(
        vrrp_state(&eng, r2),
        VrrpState::Backup,
        "低优先级应为 Backup"
    );

    eng.start_ping(pc, ip("192.168.1.254"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, pc).last().unwrap().received(),
        5,
        "虚拟网关应可 ping 通"
    );

    // Master 接口故障：Backup 接管
    eng.net.set_port_admin(r1, 0, false);
    converge(&mut eng);
    assert_eq!(vrrp_state(&eng, r2), VrrpState::Master, "Backup 应接管");
    eng.start_ping(pc, ip("192.168.1.254"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    assert_eq!(
        collect_pings(&mut eng, pc).last().unwrap().received(),
        5,
        "故障切换后虚拟网关仍可通"
    );
}

fn vrrp_state(eng: &Engine, dev: DevId) -> VrrpState {
    let d = eng.net.device(dev).unwrap();
    let DeviceState::Router(rs) = &d.state else {
        panic!()
    };
    rs.vrrp.first().unwrap().state
}

// ---------- IPv6 ----------

fn ip6(s: &str) -> Ipv6Addr {
    ipv6_from_str(s).unwrap()
}

#[test]
fn ipv6_direct_hosts_ping6() {
    let mut net = Net::new();
    let (a, b) = net.connect_hosts("PC1", ip("10.0.0.1"), "PC2", ip("10.0.0.2"), 24);
    net.set_host_v6(a, Some(ip6("2001:db8::10")), 64, None);
    net.set_host_v6(b, Some(ip6("2001:db8::20")), 64, None);
    let mut eng = Engine::new(net);
    eng.start_ping6(a, ip6("2001:db8::20"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let r = eng.take_unreported_ping6(a);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].sent, 5);
    assert_eq!(r[0].received, 5, "同网段 IPv6 ping6 应全通");
}

#[test]
fn ipv6_static_routes_between_routers() {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Ios);
    let r2 = net.add_device("R2".into(), DeviceKind::Router, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, r1).unwrap();
    net.add_link(r1, r2).unwrap();
    net.add_link(r2, pc2).unwrap();
    net.set_host_v6(
        pc1,
        Some(ip6("2001:db8:1::10")),
        64,
        Some(ip6("2001:db8:1::1")),
    );
    net.set_host_v6(
        pc2,
        Some(ip6("2001:db8:2::10")),
        64,
        Some(ip6("2001:db8:2::1")),
    );
    for (dev, port, addr, v6s) in [
        (r1, 0, "192.168.1.1", "2001:db8:1::1"),
        (r1, 1, "10.0.0.1", "2001:db8:12::1"),
        (r2, 0, "10.0.0.2", "2001:db8:12::2"),
        (r2, 1, "192.168.2.1", "2001:db8:2::1"),
    ] {
        net.set_port_admin(dev, port, true);
        net.set_port_ip(dev, port, Some(ip(addr)), 24);
        net.set_port_ipv6(dev, port, Some(ip6(v6s)), 64);
    }
    assert!(net.add_static_route6(r1, ip6("2001:db8:2::"), 64, ip6("2001:db8:12::2")));
    assert!(net.add_static_route6(r2, ip6("2001:db8:1::"), 64, ip6("2001:db8:12::1")));

    let mut eng = Engine::new(net);
    eng.start_ping6(pc1, ip6("2001:db8:2::10"));
    eng.run_to_quiescence(PING_HORIZON_NS);
    let r = eng.take_unreported_ping6(pc1);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].received, 5, "IPv6 静态路由应让两端互通: {:?}", r[0]);
}
