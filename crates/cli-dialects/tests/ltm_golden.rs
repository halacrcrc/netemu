//! F5 LTM：虚拟服务器、池成员、TCP 负载均衡与回程地址转换。

use cli_dialects::{exec_and_run, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
use sim_packets::ipv4_from_str;

fn ip(s: &str) -> u32 {
    ipv4_from_str(s).unwrap()
}

fn run(eng: &mut Engine, s: &mut Session, dev: u32, lines: &[&str]) {
    for line in lines {
        let out = exec_and_run(eng, s, dev, line);
        assert!(
            !out.contains("Error:") && !out.contains("% "),
            "{}: {}",
            line,
            out
        );
    }
}

#[test]
fn ltm_tcp_load_balances_and_translates_return() {
    let mut net = Net::new();
    let pc = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let lb = net.add_device("LB1".into(), DeviceKind::LoadBalancer, Dialect::Vrp);
    let s1 = net.add_device("SRV1".into(), DeviceKind::Host, Dialect::Ios);
    let s2 = net.add_device("SRV2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc, lb).unwrap(); // LB p0
    net.add_link(lb, s1).unwrap(); // LB p1
    net.add_link(lb, s2).unwrap(); // LB p2
    net.set_host(pc, Some(ip("10.0.0.10")), 24, Some(ip("10.0.0.1")));
    net.set_host(s1, Some(ip("10.0.1.10")), 24, Some(ip("10.0.1.1")));
    net.set_host(s2, Some(ip("10.0.2.11")), 24, Some(ip("10.0.2.1")));
    let mut eng = Engine::new(net);
    let mut s_lb = Session::default();
    let mut s_pc = Session::default();
    run(
        &mut eng,
        &mut s_lb,
        lb,
        &[
            "system-view",
            "interface g0/0/0",
            "ip address 10.0.0.1 24",
            "undo shutdown",
            "quit",
            "interface g0/0/1",
            "ip address 10.0.1.1 24",
            "undo shutdown",
            "quit",
            "interface g0/0/2",
            "ip address 10.0.2.1 24",
            "undo shutdown",
            "quit",
            "ltm pool WEB member 10.0.1.10 80",
            "ltm pool WEB member 10.0.2.11 80",
            "ltm virtual VS vip 10.0.0.100 80 pool WEB",
        ],
    );
    let show = exec_and_run(&mut eng, &mut s_lb, lb, "display ltm");
    assert!(show.contains("10.0.0.100:80"), "{}", show);
    assert!(show.contains("10.0.1.10:80"), "{}", show);

    let a = exec_and_run(&mut eng, &mut s_pc, pc, "tcp 10.0.0.100 80");
    assert!(a.contains("连接成功"), "{}", a);
    let b = exec_and_run(&mut eng, &mut s_pc, pc, "tcp 10.0.0.100 80");
    assert!(b.contains("连接成功"), "{}", b);
    let show = exec_and_run(&mut eng, &mut s_lb, lb, "display ltm");
    assert!(show.contains("10.0.1.10:80"), "{}", show);
    assert!(show.contains("10.0.2.11:80"), "{}", show);
    if let Some(d) = eng.net.device(lb) {
        assert_eq!(d.ltm.sessions.len(), 2, "两次连接应产生两条会话");
    }
}

#[test]
fn ltm_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (
            Dialect::Ios,
            Mode::Config,
            "ltm pool WEB member 10.0.1.10 80",
        ),
        (
            Dialect::Ios,
            Mode::Config,
            "ltm virtual VS vip 10.0.0.100 80 pool WEB",
        ),
        (Dialect::Ios, Mode::Exec, "show ltm"),
        (
            Dialect::Vrp,
            Mode::Config,
            "ltm pool WEB member 10.0.1.10 80",
        ),
        (Dialect::Vrp, Mode::Exec, "display ltm"),
        (
            Dialect::Comware,
            Mode::Config,
            "ltm virtual VS vip 10.0.0.100 80 pool WEB",
        ),
        (Dialect::Comware, Mode::Exec, "display ltm"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
