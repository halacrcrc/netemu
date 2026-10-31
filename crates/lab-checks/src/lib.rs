//! 实验校验引擎：对模拟状态运行声明式检查项，返回逐项判分结果。
//!
//! 检查项由实验文件（M1 起为 TOML）声明；这里提供通用执行器。

use sim_core::device::{DevId, MgmtProto, RouteSrc, VtyTransport};
use sim_core::engine::{Engine, PING_HORIZON_NS};
use sim_packets::{ipv4_from_str, ipv4_str, Ipv4Addr};

#[derive(Clone, Debug)]
pub enum CheckKind {
    /// 从某设备发起 ping，期望全部 5 个探针通（expect=false 时期望全不通）。
    Ping {
        from: DevId,
        to: Ipv4Addr,
        expect: bool,
    },
    /// 检查路由表存在特定网段（可限定来源）。
    RouteExists {
        dev: DevId,
        dst: Ipv4Addr,
        len: u8,
        src: Option<RouteSrc>,
    },
    /// 检查接口已配置指定 IP。
    PortIp {
        dev: DevId,
        port: usize,
        ip: Ipv4Addr,
    },
    /// 检查端口处于 up（管理开启且有链路且链路 up）。
    PortUp { dev: DevId, port: usize },
    /// 检查交换机上 VLAN 已创建。
    VlanExists { dev: DevId, vlan: u16 },
    /// 检查端口为 Access 且接入指定 VLAN。
    AccessVlan { dev: DevId, port: usize, vlan: u16 },
    /// 检查端口为 Trunk 且放行指定 VLAN。
    TrunkAllows { dev: DevId, port: usize, vlan: u16 },
    /// 检查 SVI 已配置指定 IP。
    SviIp { dev: DevId, vlan: u16, ip: Ipv4Addr },
    /// 检查该交换机是 STP 根桥。
    RootBridge { dev: DevId },
    /// 检查端口 STP 角色。
    PortRole {
        dev: DevId,
        port: usize,
        role: sim_core::device::StpRole,
    },
    /// 检查端口 STP 状态。
    PortState {
        dev: DevId,
        port: usize,
        state: sim_core::device::StpSt,
    },
    /// 检查端口已加入指定聚合组。
    LagMember { dev: DevId, group: u16, port: usize },
    /// 检查聚合组已选中的成员数（LACP 协商完成）。
    LagSelected {
        dev: DevId,
        group: u16,
        count: usize,
    },
    /// 检查设备上存在本地用户。
    MgmtUser { dev: DevId, name: String },
    /// 检查 VTY 允许的接入协议。
    VtyTransport { dev: DevId, transport: VtyTransport },
    /// 检查一次远程登录尝试的结果是否符合预期。
    LoginAttempt {
        from: DevId,
        to: Ipv4Addr,
        proto: MgmtProto,
        user: String,
        pass: String,
        expect: bool,
    },
}

#[derive(Clone, Debug)]
pub struct Check {
    pub label: String,
    pub kind: CheckKind,
}

#[derive(Clone, Debug)]
pub struct CheckResult {
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

pub fn run_checks(eng: &mut Engine, checks: &[Check]) -> Vec<CheckResult> {
    checks.iter().map(|c| run_one(eng, c)).collect()
}

fn run_one(eng: &mut Engine, c: &Check) -> CheckResult {
    let (passed, detail) = match &c.kind {
        CheckKind::Ping { from, to, expect } => {
            let ok = ping_ok(eng, *from, *to);
            (ok == *expect, {
                if ok {
                    "ping 连通".into()
                } else {
                    "ping 不通".into()
                }
            })
        }
        CheckKind::RouteExists { dev, dst, len, src } => {
            let Some(d) = eng.net.device(*dev) else {
                return CheckResult {
                    label: c.label.clone(),
                    passed: false,
                    detail: "设备不存在".into(),
                };
            };
            let found = {
                let mut found = false;
                let connected = d.connected_routes();
                for r in connected.iter() {
                    if r.dst == (*dst & sim_packets::mask_from_prefix(*len))
                        && r.len == *len
                        && src.is_none_or(|s| s == RouteSrc::Connected)
                    {
                        found = true;
                    }
                }
                if let sim_core::device::DeviceState::Router(rs) = &d.state {
                    for r in &rs.static_routes {
                        if r.dst == (*dst & sim_packets::mask_from_prefix(*len))
                            && r.len == *len
                            && src.is_none_or(|s| s == RouteSrc::Static)
                        {
                            found = true;
                        }
                    }
                }
                found
            };
            (
                found,
                if found {
                    format!("路由 {}/{} 存在", ipv4_str(*dst), len)
                } else {
                    format!("路由 {}/{} 不存在", ipv4_str(*dst), len)
                },
            )
        }
        CheckKind::PortIp { dev, port, ip } => {
            let actual = eng
                .net
                .device(*dev)
                .and_then(|d| d.port(*port))
                .and_then(|p| p.ip);
            (
                actual == Some(*ip),
                match actual {
                    Some(a) => format!("接口 IP = {}", ipv4_str(a)),
                    None => "接口未配置 IP".into(),
                },
            )
        }
        CheckKind::PortUp { dev, port } => {
            let up = eng
                .net
                .device(*dev)
                .and_then(|d| d.port(*port))
                .map(|p| p.admin_up && p.link.map(|l| eng.net.link_up(l)).unwrap_or(false));
            (
                up == Some(true),
                match up {
                    Some(true) => "端口 up".into(),
                    Some(false) => "端口 down".into(),
                    None => "端口不存在".into(),
                },
            )
        }
        CheckKind::VlanExists { dev, vlan } => {
            let has = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => Some(sw.vlans.contains(vlan)),
                _ => None,
            });
            (
                has == Some(true),
                match has {
                    Some(true) => format!("VLAN{} 已创建", vlan),
                    Some(false) => format!("VLAN{} 未创建", vlan),
                    None => "设备不是交换机".into(),
                },
            )
        }
        CheckKind::AccessVlan { dev, port, vlan } => {
            let got = eng.net.device(*dev).and_then(|d| d.port(*port)).map(|p| {
                (
                    p.mode == sim_core::device::PortMode::Access && p.pvid == *vlan,
                    p.pvid,
                )
            });
            match got {
                Some((ok, pvid)) => (
                    ok,
                    if ok {
                        format!("端口为 Access 且属于 VLAN{}", pvid)
                    } else {
                        format!("端口 Access VLAN 为 {}（期望 {}）", pvid, vlan)
                    },
                ),
                None => (false, "端口不存在".into()),
            }
        }
        CheckKind::TrunkAllows { dev, port, vlan } => {
            let got = eng.net.device(*dev).and_then(|d| d.port(*port)).map(|p| {
                p.mode == sim_core::device::PortMode::Trunk
                    && p.trunk_allowed.as_ref().is_none_or(|v| v.contains(vlan))
            });
            match got {
                Some(ok) => (
                    ok,
                    if ok {
                        format!("Trunk 已放行 VLAN{}", vlan)
                    } else {
                        format!("Trunk 未放行 VLAN{}", vlan)
                    },
                ),
                None => (false, "端口不存在".into()),
            }
        }
        CheckKind::RootBridge { dev } => {
            let is_root = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => {
                    Some(sw.stp.root.mac == sim_core::port_mac(*dev, 0) && sw.stp.root_cost == 0)
                }
                _ => None,
            });
            (
                is_root == Some(true),
                match is_root {
                    Some(true) => "本机是根桥".into(),
                    Some(false) => "本机不是根桥".into(),
                    None => "设备不是交换机".into(),
                },
            )
        }
        CheckKind::PortRole { dev, port, role } => {
            let got = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => Some(sw.stp.role_of(*port)),
                _ => None,
            });
            match got {
                Some(actual) => (actual == *role, format!("端口 STP 角色 = {:?}", actual)),
                None => (false, "端口不存在".into()),
            }
        }
        CheckKind::PortState { dev, port, state } => {
            let got = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => Some(sw.stp.state_of(*port)),
                _ => None,
            });
            match got {
                Some(actual) => (actual == *state, format!("端口 STP 状态 = {:?}", actual)),
                None => (false, "端口不存在".into()),
            }
        }
        CheckKind::LagMember { dev, group, port } => {
            let actual = eng
                .net
                .device(*dev)
                .and_then(|d| d.port(*port))
                .map(|p| p.lag);
            (
                actual == Some(Some(*group)),
                match actual {
                    Some(Some(g)) => format!("端口属于聚合组 {}", g),
                    Some(None) => "端口未加入聚合组".into(),
                    None => "端口不存在".into(),
                },
            )
        }
        CheckKind::LagSelected { dev, group, count } => {
            let got = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => {
                    sw.lags.get(group).map(|g| g.selected.len())
                }
                _ => None,
            });
            match got {
                Some(n) => (n >= *count, format!("聚合组已选中 {} 个成员", n)),
                None => (false, "聚合组不存在".into()),
            }
        }
        CheckKind::MgmtUser { dev, name } => {
            let has = eng
                .net
                .device(*dev)
                .map(|d| d.mgmt.find_user(name).is_some());
            (
                has == Some(true),
                match has {
                    Some(true) => format!("本地用户 {} 存在", name),
                    Some(false) => format!("本地用户 {} 不存在", name),
                    None => "设备不存在".into(),
                },
            )
        }
        CheckKind::VtyTransport { dev, transport } => {
            let got = eng.net.device(*dev).map(|d| d.mgmt.transport);
            let ok = got == Some(*transport);
            let label = match transport {
                VtyTransport::All => "all",
                VtyTransport::Ssh => "ssh",
                VtyTransport::Telnet => "telnet",
                VtyTransport::None => "none",
            };
            (
                ok,
                if ok {
                    format!("VTY 仅允许 {} 接入", label)
                } else {
                    format!("VTY 接入协议不符合预期（期望 {}）", label)
                },
            )
        }
        CheckKind::LoginAttempt {
            from,
            to,
            proto,
            user,
            pass,
            expect,
        } => {
            let r = eng.try_login(*from, *to, *proto, user, pass);
            let ok = r.is_ok();
            let name = match proto {
                MgmtProto::Telnet => "Telnet",
                MgmtProto::Ssh => "SSH",
            };
            (
                ok == *expect,
                if ok {
                    format!("{} 登录成功", name)
                } else {
                    format!("{} 登录被拒绝（{:?}）", name, r.err())
                },
            )
        }
        CheckKind::SviIp { dev, vlan, ip } => {
            let got = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => sw.svi.get(vlan).map(|s| s.ip),
                _ => None,
            });
            match got {
                Some(actual) => (
                    actual == *ip,
                    format!(
                        "Vlanif{} = {}（期望 {}）",
                        vlan,
                        ipv4_str(actual),
                        ipv4_str(*ip)
                    ),
                ),
                None => (false, format!("VLAN{} 未配置 SVI", vlan)),
            }
        }
    };
    CheckResult {
        label: c.label.clone(),
        passed,
        detail,
    }
}

/// 发起一次 ping 并驱动引擎至静默，返回是否 5/5 全通。
fn ping_ok(eng: &mut Engine, from: DevId, to: Ipv4Addr) -> bool {
    let Some(sid) = eng.start_ping(from, to) else {
        return false;
    };
    eng.run_to_quiescence(PING_HORIZON_NS);
    let sessions = eng.take_unreported_pings(from);
    let Some(s) = sessions.into_iter().find(|s| s.id == sid) else {
        return false;
    };
    s.received() == s.total
}

/// 解析 "192.168.2.0/24" 形式的网段。
pub fn parse_net(s: &str) -> Option<(Ipv4Addr, u8)> {
    let (addr, len) = s.split_once('/')?;
    Some((ipv4_from_str(addr)?, len.parse().ok()?))
}
