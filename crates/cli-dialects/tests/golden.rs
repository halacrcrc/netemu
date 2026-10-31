//! CLI 黄金输出测试：锁定三方言的命令行为与输出格式。
//! 这些快照就是"保真度回归"——改动渲染/解析时必须显式更新。

use cli_dialects::{complete_line, exec_and_run, exec_line, prompt};
use cli_engine::Session;
use sim_core::device::{DeviceKind, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn ip(s: &str) -> u32 {
    ipv4_from_str(s).unwrap()
}

/// IOS 路由器 + 两台直连主机（g0/0—PC1，g0/1—PC2）。
fn ios_lab() -> (Engine, u32, Session) {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let id = net.add_device("Router".into(), DeviceKind::Router, Dialect::Ios);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, id).unwrap(); // 路由器 g0/0
    net.add_link(id, pc2).unwrap(); // 路由器 g0/1
    net.set_host(pc1, Some(ip("192.168.1.10")), 24, Some(ip("192.168.1.1")));
    net.set_host(pc2, Some(ip("10.0.0.10")), 24, Some(ip("10.0.0.1")));
    (Engine::new(net), id, Session::default())
}

fn run(eng: &mut Engine, sess: &mut Session, dev: u32, lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .map(|l| exec_and_run(eng, sess, dev, l))
        .collect()
}

// ---------- IOS ----------

#[test]
fn ios_config_flow_and_show() {
    let (mut eng, dev, mut sess) = ios_lab();
    let outs = run(
        &mut eng,
        &mut sess,
        dev,
        &[
            "enable",
            "conf t",
            "hostname R1",
            "int g0/0",
            "ip address 192.168.1.1 255.255.255.0",
            "no shut",
            "exit",
            "int g0/1",
            "ip address 10.0.0.1 255.255.255.0",
            "no shutdown",
            "end",
            "ip route 192.168.2.0 255.255.255.0 10.0.0.2", // 特权模式没有这条，应报错
        ],
    );
    // 缩写生效、配置命令静默成功
    assert_eq!(outs[2], "", "hostname 应静默成功");
    assert_eq!(prompt(&eng, &sess, dev), "R1#");
    // 特权模式下 ip route 不可用（未进入配置模式）
    assert!(
        outs[11].contains("Invalid input detected at '^' marker."),
        "ios 特权模式 ip route 应报错: {:?}",
        outs[11]
    );

    // 回到配置模式加静态路由，然后看路由表
    let outs2 = run(
        &mut eng,
        &mut sess,
        dev,
        &[
            "configure terminal",
            "ip route 192.168.2.0 255.255.255.0 10.0.0.2",
            "end",
            "show ip route",
        ],
    );
    assert_eq!(outs2[1], "");
    assert_eq!(
        outs2[3],
        "Codes: C - connected, S - static\n\n\
         Gateway of last resort is not set\n\n\
         C    10.0.0.0/24 is directly connected, GigabitEthernet0/1\n\
         C    192.168.1.0/24 is directly connected, GigabitEthernet0/0\n\
         S    192.168.2.0/24 [1/0] via 10.0.0.2"
    );

    // 接口摘要（黄金快照；IOS 的 IP 列不带掩码长度）
    let brief = exec_and_run(&mut eng, &mut sess, dev, "show ip interface brief");
    assert_eq!(
        brief,
        "Interface                 IP-Address     OK? Method Status                Protocol\n\
         GigabitEthernet0/0        192.168.1.1    yes manual up                    up\n\
         GigabitEthernet0/1        10.0.0.1       yes manual up                    up\n\
         GigabitEthernet0/2        unassigned     yes unset  administratively down down\n\
         GigabitEthernet0/3        unassigned     yes unset  administratively down down"
    );
}

#[test]
fn ios_error_styles() {
    let (mut eng, dev, mut sess) = ios_lab();
    run(&mut eng, &mut sess, dev, &["enable", "conf t"]);
    // 非连续掩码 → 参数无法匹配，插入符指向该 token 起点（列 17）
    let bad = exec_and_run(
        &mut eng,
        &mut sess,
        dev,
        "ip route 1.2.3.0 255.0.255.0 10.0.0.9",
    );
    assert_eq!(
        bad,
        "                 ^\n% Invalid input detected at '^' marker."
    );
    let inc = exec_and_run(&mut eng, &mut sess, dev, "hostname");
    assert_eq!(inc, "% Incomplete command.");
    let bad_mask = run(
        &mut eng,
        &mut sess,
        dev,
        &["int g0/0", "ip address 10.0.0.1 255.0.255.0"],
    );
    assert!(
        bad_mask[1].contains("Invalid input"),
        "非连续掩码应报错: {:?}",
        bad_mask[1]
    );
}

#[test]
fn ios_help_and_completion() {
    let (mut eng, dev, mut sess) = ios_lab();
    run(&mut eng, &mut sess, dev, &["enable", "conf t"]);
    // 上下文帮助：参数占位符
    let h = exec_and_run(&mut eng, &mut sess, dev, "ip route ?");
    assert!(h.contains("A.B.C.D"), "{}", h);
    // route 还缺参数，不应出现 <cr>
    assert!(!h.contains("<cr>"), "{}", h);
    // 独立 `?`（用户视图）
    run(&mut eng, &mut sess, dev, &["end"]);
    let h2 = exec_and_run(&mut eng, &mut sess, dev, "?");
    assert!(h2.contains("show"), "{}", h2);
    assert!(
        h2.contains("<cr>"),
        "用户视图可直接执行命令，应有 <cr>: {}",
        h2
    );
    // Tab 补全：唯一前缀（特权模式）
    let (line, opts) = complete_line(&eng, &sess, dev, "sh");
    assert_eq!(line, "show ");
    assert_eq!(opts, vec!["show"]);
    // 扩展到唯一匹配
    let (line, _) = complete_line(&eng, &sess, dev, "sh ip in");
    assert_eq!(line, "sh ip interface ");
    // 回到配置模式：多候选 i → interface / ip
    run(&mut eng, &mut sess, dev, &["conf t"]);
    let (_, opts) = complete_line(&eng, &sess, dev, "i");
    assert_eq!(opts, vec!["interface", "ip"]);
}

#[test]
fn ios_ping_through_routers() {
    // 全 CLI 配置双路由器并从 PC1 ping PC2
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
    let mut eng = Engine::new(net);
    let mut s1 = Session::default();
    run(
        &mut eng,
        &mut s1,
        r1,
        &[
            "enable",
            "conf t",
            "int g0/0",
            "ip address 192.168.1.1 255.255.255.0",
            "no shut",
            "exit",
            "int g0/1",
            "ip address 10.0.0.1 255.255.255.0",
            "no shut",
            "exit",
            "ip route 192.168.2.0 255.255.255.0 10.0.0.2",
            "end",
        ],
    );
    let mut s2 = Session::default();
    run(
        &mut eng,
        &mut s2,
        r2,
        &[
            "enable",
            "conf t",
            "int g0/0",
            "ip address 10.0.0.2 255.255.255.0",
            "no shut",
            "exit",
            "int g0/1",
            "ip address 192.168.2.1 255.255.255.0",
            "no shut",
            "exit",
            "ip route 192.168.1.0 255.255.255.0 10.0.0.1",
            "end",
        ],
    );
    // PC1 用迷你 shell ping
    let out = exec_and_run(&mut eng, &mut Session::default(), pc1, "ping 192.168.2.10");
    assert!(
        out.contains("5 packets transmitted, 5 received, 0% packet loss"),
        "PC1 ping PC2 应全通: {}",
        out
    );
    // R1 上 ping PC2（IOS 风格输出）
    let out = exec_and_run(&mut eng, &mut s1, r1, "ping 192.168.2.10");
    assert!(
        out.contains("Success rate is 100 percent (5/5)"),
        "R1 ping 应全通: {}",
        out
    );
    assert!(out.contains("!!!!!"), "IOS 风格应输出感叹号: {}", out);
}

// ---------- VRP ----------

#[test]
fn vrp_config_flow_and_show() {
    let mut net = Net::new();
    let dev = net.add_device("Huawei".into(), DeviceKind::Router, Dialect::Vrp);
    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    assert_eq!(prompt(&eng, &sess, dev), "<Huawei>");
    let outs = run(
        &mut eng,
        &mut sess,
        dev,
        &[
            "system-view",
            "sysname HW1",
            "interface GigabitEthernet0/0/0",
            "ip address 10.0.0.1 24",
            "undo shutdown",
            "quit",
            "interface g0/0/1",
            "ip address 192.168.1.1 24",
            "undo shutdown",
            "quit",
            "ip route-static 192.168.2.0 24 10.0.0.2",
            "return",
        ],
    );
    assert_eq!(prompt(&eng, &sess, dev), "<HW1>");
    let brief = exec_and_run(&mut eng, &mut sess, dev, "display ip interface brief");
    assert_eq!(
        brief,
        "Interface                        IP Address/Mask     Physical  Protocol\n\
         GigabitEthernet0/0/0             10.0.0.1/24         down      down\n\
         GigabitEthernet0/0/1             192.168.1.1/24      down      down\n\
         GigabitEthernet0/0/2             unassigned          *down     down\n\
         GigabitEthernet0/0/3             unassigned          *down     down"
    );
    let table = exec_and_run(&mut eng, &mut sess, dev, "display ip routing-table");
    assert_eq!(
        table,
        "Routing Table: Public\n\
         Destinations : 3        Routes : 3\n\n\
         Destination/Mask    Proto   Pre  Cost   Flags  NextHop         Interface\n\n\
         10.0.0.0/24         Direct  0    0      D      10.0.0.1        GigabitEthernet0/0/0\n\
         192.168.1.0/24      Direct  0    0      D      192.168.1.1     GigabitEthernet0/0/1\n\
         192.168.2.0/24      Static  60   0      RD     10.0.0.2        GigabitEthernet0/0/0"
    );
    let _ = outs;
}

#[test]
fn vrp_error_style() {
    let mut net = Net::new();
    let dev = net.add_device("Huawei".into(), DeviceKind::Router, Dialect::Vrp);
    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    run(&mut eng, &mut sess, dev, &["system-view"]);
    let bad = exec_and_run(&mut eng, &mut sess, dev, "sysnme HW1");
    assert_eq!(bad, "Error: Unrecognized command found at '^' position.");
    let inc = exec_and_run(&mut eng, &mut sess, dev, "sysname");
    assert_eq!(inc, "Error: Incomplete command found at '^' position.");
    let too_many = exec_and_run(&mut eng, &mut sess, dev, "sysname R1 extra");
    assert_eq!(
        too_many,
        "Error: Too many parameters found at '^' position."
    );
}

#[test]
fn vrp_ping_output() {
    let mut net2 = Net::new();
    let r = net2.add_device("HW1".into(), DeviceKind::Router, Dialect::Vrp);
    let pc = net2.add_device("PC2".into(), DeviceKind::Host, Dialect::Vrp);
    net2.add_link(r, pc).unwrap();
    net2.set_host(pc, Some(ip("10.0.0.2")), 24, None);
    let mut eng2 = Engine::new(net2);
    let mut sess = Session::default();
    run(
        &mut eng2,
        &mut sess,
        r,
        &[
            "system-view",
            "int g0/0/0",
            "ip address 10.0.0.1 24",
            "undo shutdown",
            "return",
        ],
    );
    let out = exec_and_run(&mut eng2, &mut sess, r, "ping 10.0.0.2");
    assert!(out.contains("PING 10.0.0.2"), "{}", out);
    assert!(out.contains("5 packet(s) transmitted"), "{}", out);
    assert!(out.contains("0.00% packet loss"), "{}", out);
}

// ---------- Comware ----------

#[test]
fn comware_config_flow_and_show() {
    let mut net = Net::new();
    let dev = net.add_device("H3C".into(), DeviceKind::Switch, Dialect::Comware);
    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    assert_eq!(prompt(&eng, &sess, dev), "<H3C>");
    run(
        &mut eng,
        &mut sess,
        dev,
        &[
            "system-view",
            "sysname SW1",
            "interface g1/0/1",
            "shutdown",
            "quit",
            "return",
        ],
    );
    assert_eq!(prompt(&eng, &sess, dev), "<SW1>");
    // 交换机端口是 L2：ip address 应被拒绝
    run(&mut eng, &mut sess, dev, &["system-view", "int g1/0/2"]);
    let refused = exec_and_run(&mut eng, &mut sess, dev, "ip address 10.0.0.1 24");
    assert!(refused.contains("not supported on Layer 2"), "{}", refused);
    let brief = exec_and_run(&mut eng, &mut sess, dev, "display ip interface brief");
    assert_eq!(
        brief,
        "Interface                Physical Protocol IP Address      Description\n\
         GE1/0/1                  *down    down     unassigned      --\n\
         GE1/0/2                  down     down     unassigned      --\n\
         GE1/0/3                  down     down     unassigned      --\n\
         GE1/0/4                  down     down     unassigned      --\n\
         GE1/0/5                  down     down     unassigned      --\n\
         GE1/0/6                  down     down     unassigned      --\n\
         GE1/0/7                  down     down     unassigned      --\n\
         GE1/0/8                  down     down     unassigned      --"
    );
    // 三层交换机路由表为空（未配 SVI）
    run(&mut eng, &mut sess, dev, &["quit", "return"]);
    let no_route = exec_and_run(&mut eng, &mut sess, dev, "display ip routing-table");
    assert!(
        no_route.contains("Destinations : 0"),
        "无 SVI 时路由表应为空: {}",
        no_route
    );
}

// ---------- PC shell ----------

#[test]
fn pc_shell_ipconfig_and_arp() {
    let mut net = Net::new();
    let (a, b) = net.connect_hosts("PC1", ip("10.0.0.1"), "PC2", ip("10.0.0.2"), 24);
    net.set_host(a, Some(ip("10.0.0.1")), 24, None);
    let mut eng = Engine::new(net);
    let out = exec_and_run(&mut eng, &mut Session::default(), a, "ipconfig");
    assert!(out.contains("10.0.0.1/24"), "{}", out);
    exec_and_run(&mut eng, &mut Session::default(), a, "ping 10.0.0.2");
    let arp = exec_and_run(&mut eng, &mut Session::default(), a, "arp");
    assert!(
        arp.contains("10.0.0.2"),
        "ping 后 ARP 缓存应有 PC2: {}",
        arp
    );
    let unknown = exec_line(&mut eng, &mut Session::default(), a, "blah");
    assert!(unknown.contains("未知命令"), "{}", unknown);
    let _ = b;
}
