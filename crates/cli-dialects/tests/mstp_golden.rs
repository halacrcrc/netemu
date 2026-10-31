//! MSTP：三方言配置与按实例展示。

use cli_dialects::{exec_and_run, prompt, trees};
use cli_engine::{parse, Mode, Session};
use sim_core::device::{DeviceKind, DeviceState, Dialect, StpMode};
use sim_core::engine::Engine;
use sim_core::Net;

fn two_switches(dialect: Dialect) -> (Engine, u32) {
    let mut net = Net::new();
    let sw1 = net.add_device("SW1".into(), DeviceKind::Switch, dialect);
    let sw2 = net.add_device("SW2".into(), DeviceKind::Switch, dialect);
    net.add_link(sw1, sw2).unwrap();
    net.add_link(sw1, sw2).unwrap();
    (Engine::new(net), sw1)
}

fn mode_of(eng: &Engine, dev: u32) -> StpMode {
    let d = eng.net.device(dev).unwrap();
    let DeviceState::Switch(sw) = &d.state else {
        panic!()
    };
    sw.stp.mode
}

#[test]
fn ios_mstp_config_and_show() {
    let (mut eng, sw1) = two_switches(Dialect::Ios);
    let mut s = Session::default();
    for line in ["enable", "configure terminal", "spanning-tree mode mst"] {
        exec_and_run(&mut eng, &mut s, sw1, line);
    }
    assert_eq!(mode_of(&eng, sw1), StpMode::Mstp);
    exec_and_run(&mut eng, &mut s, sw1, "spanning-tree mst 1 priority 4096");
    exec_and_run(&mut eng, &mut s, sw1, "spanning-tree mst configuration");
    assert_eq!(prompt(&eng, &s, sw1), "SW1(config-mst)#");
    let out = exec_and_run(&mut eng, &mut s, sw1, "instance 1 vlan 10");
    assert_eq!(out, "");
    exec_and_run(&mut eng, &mut s, sw1, "exit");
    exec_and_run(&mut eng, &mut s, sw1, "end");
    let show = exec_and_run(&mut eng, &mut s, sw1, "show spanning-tree");
    assert!(show.contains("MSTP configuration"), "{}", show);
    assert!(show.contains("Instance 1 (VLAN 10)"), "{}", show);
    assert!(show.contains("Root 4096."), "{}", show);
}

#[test]
fn vrp_mstp_config_and_display() {
    let (mut eng, sw1) = two_switches(Dialect::Vrp);
    let mut s = Session::default();
    for line in ["system-view", "stp mode mstp"] {
        exec_and_run(&mut eng, &mut s, sw1, line);
    }
    assert_eq!(mode_of(&eng, sw1), StpMode::Mstp);
    exec_and_run(&mut eng, &mut s, sw1, "stp instance 1 priority 4096");
    exec_and_run(&mut eng, &mut s, sw1, "stp region-configuration");
    assert_eq!(prompt(&eng, &s, sw1), "[SW1-mst-region]");
    exec_and_run(&mut eng, &mut s, sw1, "instance 1 vlan 10 20");
    exec_and_run(&mut eng, &mut s, sw1, "region-name campus");
    let out = exec_and_run(&mut eng, &mut s, sw1, "active region-configuration");
    assert_eq!(out, "");
    exec_and_run(&mut eng, &mut s, sw1, "quit");
    let show = exec_and_run(&mut eng, &mut s, sw1, "display stp brief");
    assert!(show.contains("Mode: MSTP"), "{}", show);
    assert!(show.contains("Instance 1 (VLAN 10,20)"), "{}", show);
    assert!(show.contains("Root 4096."), "{}", show);
}

#[test]
fn mstp_commands_parse_in_all_dialects() {
    let cases: Vec<(Dialect, Mode, &str)> = vec![
        (Dialect::Ios, Mode::Config, "spanning-tree mode mst"),
        (
            Dialect::Ios,
            Mode::Config,
            "spanning-tree mst 1 priority 4096",
        ),
        (
            Dialect::Ios,
            Mode::Config,
            "spanning-tree mst configuration",
        ),
        (Dialect::Ios, Mode::Mst, "instance 1 vlan 10,20"),
        (Dialect::Ios, Mode::Mst, "name campus"),
        (Dialect::Ios, Mode::Mst, "revision 1"),
        (Dialect::Vrp, Mode::Config, "stp mode mstp"),
        (Dialect::Vrp, Mode::Config, "stp instance 1 priority 4096"),
        (Dialect::Vrp, Mode::Config, "stp region-configuration"),
        (Dialect::Vrp, Mode::Mst, "instance 1 vlan 10 20"),
        (Dialect::Vrp, Mode::Mst, "revision-level 1"),
        (Dialect::Vrp, Mode::Mst, "active region-configuration"),
        (Dialect::Comware, Mode::Config, "stp mode mstp"),
        (Dialect::Comware, Mode::Mst, "instance 2 vlan 20"),
    ];
    for (d, m, line) in cases {
        let ok = matches!(parse(trees(d).root_for(m), line), Ok(Some(_)));
        assert!(ok, "{:?} {:?} '{}' 应可解析", d, m, line);
    }
}
