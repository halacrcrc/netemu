//! RSTP：三方言模式命令与 show 输出测试。

use cli_dialects::{exec_and_run, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, DeviceState, Dialect, StpMode};
use sim_core::engine::Engine;
use sim_core::Net;

fn two_switches(dialect: Dialect) -> (Engine, u32, Session) {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, dialect);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, dialect);
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw1, sw2).unwrap();
    (Engine::new(net), sw1, Session::default())
}

fn mode_of(eng: &Engine, dev: u32) -> StpMode {
    let d = eng.net.device(dev).unwrap();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    sw.stp.mode
}

#[test]
fn ios_rapid_pvst_command_and_show() {
    let (mut eng, sw1, mut s) = two_switches(Dialect::Ios);
    exec_and_run(&mut eng, &mut s, sw1, "enable");
    exec_and_run(&mut eng, &mut s, sw1, "configure terminal");
    let out = exec_and_run(&mut eng, &mut s, sw1, "spanning-tree mode rapid-pvst");
    assert_eq!(out, "");
    assert_eq!(mode_of(&eng, sw1), StpMode::Rstp);
    exec_and_run(&mut eng, &mut s, sw1, "end");
    let show = exec_and_run(&mut eng, &mut s, sw1, "show spanning-tree");
    assert!(
        show.contains("Spanning tree enabled protocol rapid-pvst"),
        "{}",
        show
    );
    assert!(show.contains("FWD"), "RSTP 端口应直接转发:\n{}", show);

    // 切回 802.1D
    exec_and_run(&mut eng, &mut s, sw1, "configure terminal");
    exec_and_run(&mut eng, &mut s, sw1, "spanning-tree mode pvst");
    assert_eq!(mode_of(&eng, sw1), StpMode::Dot1d);
}

#[test]
fn vrp_rstp_mode_and_display() {
    let (mut eng, sw1, mut s) = two_switches(Dialect::Vrp);
    exec_and_run(&mut eng, &mut s, sw1, "system-view");
    let out = exec_and_run(&mut eng, &mut s, sw1, "stp mode rstp");
    assert_eq!(out, "");
    assert_eq!(mode_of(&eng, sw1), StpMode::Rstp);
    let show = exec_and_run(&mut eng, &mut s, sw1, "display stp brief");
    assert!(show.contains("Mode: RSTP"), "{}", show);
    assert!(show.contains("FORWARDING"), "{}", show);

    exec_and_run(&mut eng, &mut s, sw1, "stp mode stp");
    assert_eq!(mode_of(&eng, sw1), StpMode::Dot1d);
    let show = exec_and_run(&mut eng, &mut s, sw1, "display stp brief");
    assert!(show.contains("Mode: STP"), "{}", show);
}

#[test]
fn rstp_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "spanning-tree mode rapid-pvst"),
        (Dialect::Ios, Mode::Config, "spanning-tree mode pvst"),
        (Dialect::Vrp, Mode::Config, "stp mode rstp"),
        (Dialect::Vrp, Mode::Config, "stp mode stp"),
        (Dialect::Comware, Mode::Config, "stp mode rstp"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
