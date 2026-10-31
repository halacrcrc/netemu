//! M2a：三方言 STP 命令的黄金输出测试。

use cli_dialects::{exec_and_run, prompt};
use cli_engine::Session;
use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::Engine;
use sim_core::Net;
/// 双交换机平行链路（环路）。
fn two_switch_lab() -> (Engine, u32, u32, Session, Session) {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, Dialect::Ios);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, Dialect::Vrp);
    net.add_link(sw1, sw2).unwrap(); // SW1 p0 — SW2 p0
    net.add_link(sw1, sw2).unwrap(); // SW1 p1 — SW2 p1
    let eng = Engine::new(net);
    (eng, sw1, sw2, Session::default(), Session::default())
}

#[test]
fn ios_show_spanning_tree_golden() {
    let (mut eng, sw1, _sw2, mut s1, _s2) = two_switch_lab();
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
    let out = exec_and_run(&mut eng, &mut s1, sw1, "show spanning-tree");
    // SW1 是根（MAC 最小）
    assert!(
        out.contains("This bridge is the root"),
        "SW1 应为根:\n{}",
        out
    );
    assert!(out.contains("Address     0200.0000.0000"), "{}", out);
    assert!(out.contains("Desg"), "{}", out);
    // 全部端口转发（根桥无阻塞口）
    assert!(out.contains("FWD"), "{}", out);

    // 非根桥视角：切到 SW2（VRP 设备用 VRP 命令，此处用 IOS 语法对照在 vrp 测试）
    let _ = _sw2;
}

#[test]
fn vrp_stp_priority_and_display_golden() {
    let (mut eng, sw1, sw2, mut s1, mut s2) = two_switch_lab();
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);

    // SW2 优先级 4096 → 根桥变更
    let outs: Vec<String> = ["system-view", "stp priority 4096", "quit"]
        .iter()
        .map(|l| exec_and_run(&mut eng, &mut s2, sw2, l))
        .collect();
    assert_eq!(outs[1], "", "stp priority 应静默成功");
    // 根桥变更触发重新收敛（真实 STP 端口需重走 listening/learning）
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
    let brief = exec_and_run(&mut eng, &mut s2, sw2, "display stp brief");
    // SW2 自己成了根桥：所有端口 DESI/FORWARDING（根桥无根端口）
    assert!(
        brief.contains("DESI") && brief.contains("FORWARDING"),
        "SW2 为根桥应全部指定端口转发:\n{}",
        brief
    );
    assert!(
        !brief.contains("ROOT"),
        "根桥视角不应出现 ROOT 角色:\n{}",
        brief
    );
    // 根桥已是 SW2（优先级 4096）——SW1 视角确认
    let out1 = exec_and_run(&mut eng, &mut s1, sw1, "show spanning-tree");
    assert!(
        out1.contains("Priority    4096"),
        "SW1 看到的根优先级应为 4096:\n{}",
        out1
    );
    assert!(!out1.contains("This bridge is the root"), "SW1 不再是根");

    // SW1 状态与角色：平行链路一端被阻塞
    let d = eng.net.device(sw1).unwrap().clone();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    let blocked = (0..2)
        .filter(|&p| sw.stp.role_of(p) == sim_core::device::StpRole::Alternate)
        .count();
    assert_eq!(blocked, 1, "平行链路应阻塞一端");

    // stp disable 后风暴护栏（不做长跑，仅确认命令生效）
    let ok = exec_and_run(&mut eng, &mut s2, sw2, "system-view");
    let _ = ok;
    let out = exec_and_run(&mut eng, &mut s2, sw2, "stp disable");
    assert_eq!(out, "", "stp disable 应静默成功");
    let enabled = match &eng.net.device(sw2).unwrap().state {
        DeviceState::Switch(sw) => sw.stp.enabled,
        _ => true,
    };
    assert!(!enabled);
    // prompt：仍在系统视图
    let p = prompt(&eng, &s2, sw2);
    assert_eq!(p, "[SW2]", "提示符");
}

#[test]
fn comware_stp_cost_golden() {
    let mut net = Net::new();
    let a = net.add_device("SWA".into(), DeviceKind::Switch, Dialect::Comware);
    let b = net.add_device("SWB".into(), DeviceKind::Switch, Dialect::Comware);
    net.add_link(a, b).unwrap();
    net.add_link(a, b).unwrap();
    // SWB 优先级 4096 → SWB 为根，SWA 才有根端口
    let d = net.device_mut(b).unwrap();
    if let DeviceState::Switch(sw) = &mut d.state {
        sw.stp.priority = 4096;
    }
    let mut eng = Engine::new(net);
    eng.stp_topology_changed();
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
    let mut sess = Session::default();
    let outs: Vec<String> = ["system-view", "int g1/0/1", "stp cost 100", "quit"]
        .iter()
        .map(|l| exec_and_run(&mut eng, &mut sess, a, l))
        .collect();
    assert_eq!(outs[2], "", "stp cost 应静默成功");
    // 成本变化改变根端口 → 重新收敛
    let h = eng.drain_horizon();
    eng.run_to_quiescence(h);
    let brief = exec_and_run(&mut eng, &mut sess, a, "display stp brief");
    assert!(brief.contains("FORWARDING"), "收敛后应全部转发:\n{}", brief);
    // 成本抬高后：g1/0/2 变根端口（走低成本路径）
    let d = eng.net.device(a).unwrap().clone();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    assert_eq!(sw.stp.root_port, Some(1), "低成本口应成为根端口");
}

#[test]
fn zz_probe2() {
    use sim_core::device::Dialect;
    // 完整 exec 序列复现
    use cli_dialects::exec_and_run;
    use cli_engine::Session;
    use sim_core::engine::Engine;
    use sim_core::{DeviceKind, Net};
    let mut net = Net::new();
    let r1 = net.add_device("R1".into(), DeviceKind::Router, Dialect::Comware);
    let mut eng = Engine::new(net);
    let mut sess = Session::default();
    for l in [
        "system-view",
        "acl 3000",
        "rule deny icmp source 192.168.1.10 0 destination 192.168.3.100 0",
    ] {
        let out = exec_and_run(&mut eng, &mut sess, r1, l);
        println!("PROBE3 {:?} → {:?} mode={:?}", l, out, sess.mode);
    }
}

#[test]
fn zz_dump_acl_tree() {
    use cli_dialects::trees;
    use cli_engine::Mode;
    use sim_core::device::Dialect;
    fn dump(nodes: &[cli_engine::CNode], depth: usize, out: &mut Vec<String>) {
        for n in nodes {
            let tok = match &n.tok {
                cli_engine::Tok::Lit(w) => format!("L:{}", w),
                cli_engine::Tok::Param { kind, name } => format!("P:{:?}:{}", kind, name),
            };
            out.push(format!(
                "{}{} run={} pkids={}",
                "  ".repeat(depth),
                tok,
                n.run.is_some(),
                n.kids
                    .iter()
                    .any(|k| matches!(k.tok, cli_engine::Tok::Param { .. }))
            ));
            dump(&n.kids, depth + 1, out);
        }
    }
    let t = trees(Dialect::Comware);
    let mut out = vec![];
    dump(t.root_for(Mode::Acl), 0, &mut out);
    for l in out {
        println!("TREE {}", l);
    }
}
