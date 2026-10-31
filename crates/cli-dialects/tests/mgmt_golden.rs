//! M2 收尾：SSH/Telnet 管理面命令与远程登录会话的黄金测试。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn lab(r1_dialect: Dialect) -> (Engine, u32, u32) {
    let mut net = Net::new();
    let pc = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let r1 = net.add_device("R1".into(), DeviceKind::Router, r1_dialect);
    net.add_link(pc, r1).unwrap();
    net.set_host(
        pc,
        Some(ipv4_from_str("192.168.1.10").unwrap()),
        24,
        Some(ipv4_from_str("192.168.1.1").unwrap()),
    );
    (Engine::new(net), pc, r1)
}

fn run(eng: &mut Engine, sess: &mut Session, dev: u32, lines: &[&str]) {
    for l in lines {
        let out = exec_and_run(eng, sess, dev, l);
        assert!(
            !out.contains("% ") && !out.contains("Error:"),
            "命令 {:?} 意外报错: {}",
            l,
            out
        );
    }
}

fn login_flow(
    eng: &mut Engine,
    sess: &mut Session,
    pc: u32,
    proto: &str,
    user: &str,
    pass: &str,
) -> String {
    let out = exec_and_run(eng, sess, pc, &format!("{} 192.168.1.1", proto));
    assert!(
        out.contains("Trying 192.168.1.1") || out.contains("Connecting to 192.168.1.1"),
        "应显示连接过程: {}",
        out
    );
    assert_eq!(prompt(eng, sess, pc), "Username: ");
    let _ = exec_and_run(eng, sess, pc, user);
    assert_eq!(prompt(eng, sess, pc), "Password: ");
    exec_and_run(eng, sess, pc, pass)
}

const IOS_CONFIG: &[&str] = &[
    "enable",
    "configure terminal",
    "interface g0/0",
    "ip address 192.168.1.1 255.255.255.0",
    "no shutdown",
    "exit",
    "username admin privilege 15 secret Cisco@123",
    "line vty 0 4",
    "login local",
    "transport input ssh",
    "exit",
];

const VRP_CONFIG: &[&str] = &[
    "system-view",
    "interface g0/0/0",
    "ip address 192.168.1.1 24",
    "undo shutdown",
    "quit",
    "aaa",
    "local-user admin password cipher Cisco@123",
    "local-user admin privilege level 15",
    "local-user admin service-type ssh",
    "quit",
    "user-interface vty 0 4",
    "authentication-mode aaa",
    "protocol inbound ssh",
    "quit",
];

const COMWARE_CONFIG: &[&str] = &[
    "system-view",
    "interface g1/0/1",
    "ip address 192.168.1.1 24",
    "undo shutdown",
    "quit",
    "local-user admin class manage",
    "password simple Cisco@123",
    "service-type ssh",
    "authorization-attribute user-role network-admin",
    "quit",
    "line vty 0 4",
    "authentication-mode scheme",
    "protocol inbound ssh",
    "quit",
];

#[test]
fn ios_ssh_remote_session_golden() {
    let (mut eng, pc, r1) = lab(Dialect::Ios);
    let mut rs = Session::default();
    run(&mut eng, &mut rs, r1, IOS_CONFIG);

    let mut ps = Session::default();
    // 错误口令
    let out = login_flow(&mut eng, &mut ps, pc, "ssh", "admin", "bad");
    assert!(out.contains("Permission denied"), "{}", out);
    assert_eq!(prompt(&eng, &ps, pc), "PC1>", "登录失败应回到本机提示符");

    // 正确口令 → 权限 15 直接进特权模式
    let out = login_flow(&mut eng, &mut ps, pc, "ssh", "admin", "Cisco@123");
    assert_eq!(out, "");
    assert_eq!(prompt(&eng, &ps, pc), "R1#");

    // 远程执行 show：输出来自目标设备
    let out = exec_and_run(&mut eng, &mut ps, pc, "show ip interface brief");
    assert!(
        out.contains("GigabitEthernet0/0") && out.contains("192.168.1.1"),
        "{}",
        out
    );

    // 远程改配置：落到目标设备
    run(
        &mut eng,
        &mut ps,
        pc,
        &["configure terminal", "hostname EDGE1", "end"],
    );
    assert_eq!(prompt(&eng, &ps, pc), "EDGE1#");
    assert_eq!(eng.net.device(r1).unwrap().name, "EDGE1");

    // 断开
    let out = exec_and_run(&mut eng, &mut ps, pc, "exit");
    assert!(out.contains("Connection closed"), "{}", out);
    assert_eq!(prompt(&eng, &ps, pc), "PC1>");

    // Telnet 被 VTY 传输策略拒绝（无需输入口令）
    let out = exec_and_run(&mut eng, &mut ps, pc, "telnet 192.168.1.1");
    assert!(out.contains("Connection refused"), "{}", out);
    assert_eq!(prompt(&eng, &ps, pc), "PC1>");
    assert!(ps.login.is_none(), "被拒绝时不应进入登录流程");
}

#[test]
fn vrp_ssh_remote_session_golden() {
    let (mut eng, pc, r1) = lab(Dialect::Vrp);
    let mut rs = Session::default();
    run(&mut eng, &mut rs, r1, VRP_CONFIG);
    // AAA 视图提示符
    let _ = exec_and_run(&mut eng, &mut rs, r1, "system-view");
    let _ = exec_and_run(&mut eng, &mut rs, r1, "aaa");
    assert_eq!(prompt(&eng, &rs, r1), "[R1-aaa]");

    let mut ps = Session::default();
    let _ = login_flow(&mut eng, &mut ps, pc, "ssh", "admin", "Cisco@123");
    assert_eq!(prompt(&eng, &ps, pc), "<R1>");
    let out = exec_and_run(&mut eng, &mut ps, pc, "display ip interface brief");
    assert!(out.contains("192.168.1.1"), "{}", out);
    let _ = exec_and_run(&mut eng, &mut ps, pc, "exit");
    assert_eq!(prompt(&eng, &ps, pc), "PC1>");

    let out = exec_and_run(&mut eng, &mut ps, pc, "telnet 192.168.1.1");
    assert!(out.contains("refused"), "{}", out);
}

#[test]
fn comware_ssh_remote_session_golden() {
    let (mut eng, pc, r1) = lab(Dialect::Comware);
    let mut rs = Session::default();
    run(&mut eng, &mut rs, r1, COMWARE_CONFIG);

    // 用户视图提示符
    let mut ps = Session::default();
    let _ = login_flow(&mut eng, &mut ps, pc, "ssh", "admin", "Cisco@123");
    assert_eq!(prompt(&eng, &ps, pc), "<R1>");
    let out = exec_and_run(&mut eng, &mut ps, pc, "display ip interface brief");
    assert!(out.contains("192.168.1.1"), "{}", out);
    let _ = exec_and_run(&mut eng, &mut ps, pc, "exit");
    assert_eq!(prompt(&eng, &ps, pc), "PC1>");

    let out = exec_and_run(&mut eng, &mut ps, pc, "telnet 192.168.1.1");
    assert!(out.contains("refused"), "{}", out);
}

#[test]
fn mgmt_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (
            Dialect::Ios,
            Mode::Config,
            "username admin privilege 15 secret Cisco@123",
        ),
        (Dialect::Ios, Mode::Config, "line vty 0 4"),
        (Dialect::Ios, Mode::Line, "login local"),
        (Dialect::Ios, Mode::Line, "transport input ssh"),
        (Dialect::Ios, Mode::Line, "transport input ssh telnet"),
        (Dialect::Ios, Mode::Exec, "ssh 192.168.1.1"),
        (Dialect::Ios, Mode::Priv, "telnet 192.168.1.1"),
        (Dialect::Vrp, Mode::Config, "aaa"),
        (
            Dialect::Vrp,
            Mode::Config,
            "local-user admin password cipher Cisco@123",
        ),
        (
            Dialect::Vrp,
            Mode::Aaa,
            "local-user admin privilege level 15",
        ),
        (
            Dialect::Vrp,
            Mode::Aaa,
            "local-user admin service-type telnet ssh",
        ),
        (Dialect::Vrp, Mode::Line, "authentication-mode aaa"),
        (Dialect::Vrp, Mode::Line, "protocol inbound ssh"),
        (Dialect::Vrp, Mode::Exec, "ssh 192.168.1.1"),
        (
            Dialect::Comware,
            Mode::Config,
            "local-user admin class manage",
        ),
        (Dialect::Comware, Mode::Config, "line vty 0 4"),
        (Dialect::Comware, Mode::Aaa, "password simple Cisco@123"),
        (Dialect::Comware, Mode::Aaa, "service-type ssh"),
        (
            Dialect::Comware,
            Mode::Aaa,
            "authorization-attribute user-role network-admin",
        ),
        (Dialect::Comware, Mode::Line, "authentication-mode scheme"),
        (Dialect::Comware, Mode::Line, "protocol inbound ssh"),
        (Dialect::Comware, Mode::Exec, "telnet 192.168.1.1"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
