//! M3b：三方言链路聚合命令与 show 输出的黄金测试。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;

/// 两台交换机 + 两台 PC，交换机之间两条链路。
fn lag_lab(d1: Dialect, d2: Dialect) -> (Engine, u32, u32, u32, u32) {
    let mut net = Net::new();
    let pc1 = net.add_device("PC1".into(), DeviceKind::Host, Dialect::Ios);
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, d1);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, d2);
    let pc2 = net.add_device("PC2".into(), DeviceKind::Host, Dialect::Ios);
    net.add_link(pc1, sw1).unwrap(); // SW1 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p1 - SW2 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p2 - SW2 p1
    net.add_link(sw2, pc2).unwrap(); // SW2 p2
    net.set_host(
        pc1,
        Some(sim_packets::ipv4_from_str("192.168.10.11").unwrap()),
        24,
        None,
    );
    net.set_host(
        pc2,
        Some(sim_packets::ipv4_from_str("192.168.10.12").unwrap()),
        24,
        None,
    );
    let eng = Engine::new(net);
    (eng, pc1, sw1, sw2, pc2)
}

fn run(eng: &mut Engine, sess: &mut Session, dev: u32, lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .map(|l| exec_and_run(eng, sess, dev, l))
        .collect()
}

fn selected(eng: &Engine, dev: u32, gid: u16) -> Vec<usize> {
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
fn ios_lacp_summary_golden() {
    let (mut eng, pc1, sw1, sw2, _pc2) = lag_lab(Dialect::Ios, Dialect::Ios);
    let mut s1 = Session::default();
    let mut s2 = Session::default();

    run(
        &mut eng,
        &mut s1,
        sw1,
        &[
            "enable",
            "configure terminal",
            "interface g0/1",
            "channel-group 1 mode active",
            "exit",
            "interface g0/2",
            "channel-group 1 mode active",
            "end",
        ],
    );
    run(
        &mut eng,
        &mut s2,
        sw2,
        &[
            "enable",
            "configure terminal",
            "interface g0/0",
            "channel-group 1 mode active",
            "exit",
            "interface g0/1",
            "channel-group 1 mode active",
            "end",
        ],
    );
    assert_eq!(selected(&eng, sw1, 1), vec![1, 2]);
    assert_eq!(selected(&eng, sw2, 1), vec![0, 1]);
    let summary = exec_and_run(&mut eng, &mut s1, sw1, "show etherchannel summary");
    assert!(
        summary.contains("Number of channel-groups in use: 1"),
        "{}",
        summary
    );
    assert!(summary.contains("Po1(SU)"), "{}", summary);
    assert!(summary.contains("LACP"), "{}", summary);
    assert!(
        summary.contains("Gi0/1(P)") && summary.contains("Gi0/2(P)"),
        "{}",
        summary
    );

    // STP 只看到一个逻辑端口（聚合口）
    let stp = exec_and_run(&mut eng, &mut s1, sw1, "show spanning-tree");
    assert_eq!(
        stp.matches("Port-channel1").count(),
        1,
        "STP 应只显示一个聚合口:\n{}",
        stp
    );
    assert!(stp.contains("FWD"), "{}", stp);

    // 提示符进入聚合口视图
    let mut s3 = Session::default();
    run(&mut eng, &mut s3, sw1, &["enable", "configure terminal"]);
    let _ = exec_and_run(&mut eng, &mut s3, sw1, "interface Port-channel1");
    assert_eq!(prompt(&eng, &s3, sw1), "SW1(config-if)#");

    // 业务连通
    let mut hp = Session::default();
    let ping = exec_and_run(&mut eng, &mut hp, pc1, "ping 192.168.10.12");
    assert!(ping.contains("5 received"), "聚合链路应连通:\n{}", ping);
}

#[test]
fn vrp_eth_trunk_display_golden() {
    let (mut eng, pc1, sw1, sw2, _pc2) = lag_lab(Dialect::Vrp, Dialect::Vrp);
    let mut s1 = Session::default();
    let mut s2 = Session::default();

    run(
        &mut eng,
        &mut s1,
        sw1,
        &[
            "system-view",
            "interface Eth-Trunk 1",
            "mode lacp-static",
            "quit",
            "interface g0/0/2",
            "eth-trunk 1",
            "quit",
            "interface g0/0/3",
            "eth-trunk 1",
            "quit",
        ],
    );
    run(
        &mut eng,
        &mut s2,
        sw2,
        &[
            "system-view",
            "interface Eth-Trunk 1",
            "quit",
            "interface g0/0/1",
            "eth-trunk 1",
            "quit",
            "interface g0/0/2",
            "eth-trunk 1",
            "quit",
        ],
    );
    assert_eq!(selected(&eng, sw1, 1), vec![1, 2]);
    assert_eq!(selected(&eng, sw2, 1), vec![0, 1]);

    let out = exec_and_run(&mut eng, &mut s1, sw1, "display eth-trunk 1");
    assert!(out.contains("WorkingMode: LACP"), "{}", out);
    assert!(out.contains("Operate status: up"), "{}", out);
    assert!(out.contains("Number Of Up Port In Trunk: 2"), "{}", out);
    assert!(out.contains("GigabitEthernet0/0/2"), "{}", out);
    assert!(out.contains("GigabitEthernet0/0/3"), "{}", out);

    // 提示符
    let mut s3 = Session::default();
    let _ = exec_and_run(&mut eng, &mut s3, sw1, "system-view");
    let _ = exec_and_run(&mut eng, &mut s3, sw1, "interface Eth-Trunk 1");
    assert_eq!(prompt(&eng, &s3, sw1), "[SW1-Eth-Trunk1]");

    let mut hp = Session::default();
    let ping = exec_and_run(&mut eng, &mut hp, pc1, "ping 192.168.10.12");
    assert!(ping.contains("5 received"), "聚合链路应连通:\n{}", ping);
}

#[test]
fn comware_bridge_aggregation_display_golden() {
    let (mut eng, pc1, sw1, sw2, _pc2) = lag_lab(Dialect::Comware, Dialect::Comware);
    let mut s1 = Session::default();
    let mut s2 = Session::default();

    run(
        &mut eng,
        &mut s1,
        sw1,
        &[
            "system-view",
            "interface Bridge-Aggregation 1",
            "link-aggregation mode dynamic",
            "quit",
            "interface g1/0/2",
            "port link-aggregation group 1",
            "quit",
            "interface g1/0/3",
            "port link-aggregation group 1",
            "quit",
        ],
    );
    run(
        &mut eng,
        &mut s2,
        sw2,
        &[
            "system-view",
            "interface Bridge-Aggregation 1",
            "quit",
            "interface g1/0/1",
            "port link-aggregation group 1",
            "quit",
            "interface g1/0/2",
            "port link-aggregation group 1",
            "quit",
        ],
    );
    assert_eq!(selected(&eng, sw1, 1), vec![1, 2]);
    assert_eq!(selected(&eng, sw2, 1), vec![0, 1]);

    let out = exec_and_run(&mut eng, &mut s1, sw1, "display link-aggregation summary");
    assert!(
        out.contains("Aggregation Interface: Bridge-Aggregation1"),
        "{}",
        out
    );
    assert!(out.contains("Aggregation Mode: Dynamic"), "{}", out);
    assert!(
        out.contains("GE1/0/2") && out.contains("GE1/0/3"),
        "{}",
        out
    );

    let mut hp = Session::default();
    let ping = exec_and_run(&mut eng, &mut hp, pc1, "ping 192.168.10.12");
    assert!(ping.contains("5 received"), "聚合链路应连通:\n{}", ping);
}

#[test]
fn lacp_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::If, "channel-group 1 mode active"),
        (Dialect::Ios, Mode::If, "channel-group 1 mode passive"),
        (Dialect::Ios, Mode::If, "channel-group 1 mode on"),
        (Dialect::Ios, Mode::If, "no channel-group"),
        (Dialect::Ios, Mode::Config, "interface Port-channel1"),
        (Dialect::Ios, Mode::Priv, "show etherchannel summary"),
        (Dialect::Vrp, Mode::Config, "interface Eth-Trunk 1"),
        (Dialect::Vrp, Mode::If, "eth-trunk 1"),
        (Dialect::Vrp, Mode::If, "mode lacp-static"),
        (Dialect::Vrp, Mode::If, "undo eth-trunk"),
        (Dialect::Vrp, Mode::Exec, "display eth-trunk 1"),
        (
            Dialect::Comware,
            Mode::Config,
            "interface Bridge-Aggregation 1",
        ),
        (Dialect::Comware, Mode::If, "port link-aggregation group 1"),
        (Dialect::Comware, Mode::If, "link-aggregation mode dynamic"),
        (
            Dialect::Comware,
            Mode::If,
            "undo port link-aggregation group",
        ),
        (
            Dialect::Comware,
            Mode::Exec,
            "display link-aggregation summary",
        ),
    ];
    for (d, m, line) in cases {
        let root = trees(d).root_for(m);
        let ok = matches!(parse(root, line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
