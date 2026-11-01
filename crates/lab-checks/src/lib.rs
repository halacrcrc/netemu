//! 实验校验引擎：对模拟状态运行声明式检查项，返回逐项判分结果。
//!
//! 检查项由实验文件（M1 起为 TOML）声明；这里提供通用执行器。

use sim_core::device::{DevId, MgmtProto, MuxRole, RouteSrc, VtyTransport};
use sim_core::engine::{Engine, PING_HORIZON_NS};
use sim_packets::{ipv4_from_str, ipv4_str, Ipv4Addr, Ipv6Addr};

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
    /// 检查设备上存在指定名称的静态 LSP。
    MplsLsp { dev: DevId, name: String },
    /// 检查 VRRP 组处于 Master。
    VrrpMaster { dev: DevId, vrid: u8 },
    /// 检查 OSPF 邻居数量不少于 min。
    OspfNeighbors { dev: DevId, min: usize },
    /// 检查端口所属安全区域。
    SecurityZone {
        dev: DevId,
        port: usize,
        zone: String,
    },
    /// 检查区域间策略。
    SecurityPolicy {
        dev: DevId,
        name: String,
        src_zone: String,
        dst_zone: String,
        permit: bool,
    },
    /// 检查 IPsec 隧道配置。
    IpsecTunnel { dev: DevId, name: String },
    /// 检查主机已建立 SSL VPN 会话。
    VpnConnected { dev: DevId },
    /// 检查 AP 已加入 AC。
    WlanApJoined { ac: DevId, ap: DevId },
    /// 检查 AC 的 SSID。
    WlanSsid { ac: DevId, ssid: String },
    /// 检查主机已关联 Wi-Fi。
    WifiConnected { dev: DevId },
    /// 检查主机当前关联的 AP。
    WifiOnAp { dev: DevId, ap: DevId },
    /// 检查 VXLAN VNI 与 VLAN 映射。
    VxlanVni { dev: DevId, vlan: u16, vni: u32 },
    /// 检查堆叠 domain/member。
    StackMember { dev: DevId, domain: u16, member: u8 },
    /// 检查 M-LAG domain。
    MlagDomain { dev: DevId, domain: u16 },
    /// 检查 EVPN 学到的远端 VTEP/VNI。
    EvpnRoute {
        dev: DevId,
        vtep: Ipv4Addr,
        vni: u32,
    },
    /// 检查 LTM 虚拟服务器。
    LtmVirtual {
        dev: DevId,
        name: String,
        vip: Ipv4Addr,
        port: u16,
    },
    /// 发起 TCP 探测并检查结果。
    TcpProbe {
        from: DevId,
        to: Ipv4Addr,
        port: u16,
        expect: bool,
    },
    /// 检查端口隔离组。
    PortIsolate { dev: DevId, port: usize, group: u16 },
    /// 检查 MUX VLAN 端口角色。
    MuxRole {
        dev: DevId,
        port: usize,
        role: MuxRole,
    },
    /// 检查 OSPFv3 邻居数量。
    Ospfv3Neighbors { dev: DevId, min: usize },
    /// 检查 SRv6 静态路由。
    Srv6Route { dev: DevId, dst: Ipv6Addr, len: u8 },
    /// 发起 IPv6 ping 并检查结果。
    Ping6 {
        from: DevId,
        to: Ipv6Addr,
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
    // 让 STP 完成 Listening/Learning（约 30s）后再判分，避免端口仍在 Learning 时误判不通。
    eng.run_to_quiescence(45_000_000_000);
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
                    for r in rs
                        .ospf_routes
                        .iter()
                        .chain(rs.bgp_routes.iter())
                        .chain(rs.isis_routes.iter())
                    {
                        if r.dst == (*dst & sim_packets::mask_from_prefix(*len))
                            && r.len == *len
                            && src.is_none_or(|s| s == r.src)
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
        CheckKind::MplsLsp { dev, name } => {
            let found = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Router(rs) => {
                    rs.mpls_lsps.iter().find(|l| l.name == *name).cloned()
                }
                _ => None,
            });
            (
                found.is_some(),
                match found {
                    Some(l) => format!(
                        "LSP {} 已配置（in {} / out {}）",
                        name,
                        l.in_label
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".into()),
                        l.out_label
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".into())
                    ),
                    None => format!("LSP {} 未配置", name),
                },
            )
        }
        CheckKind::VrrpMaster { dev, vrid } => {
            let state = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Router(rs) => {
                    rs.vrrp.iter().find(|g| g.vrid == *vrid).map(|g| g.state)
                }
                _ => None,
            });
            (
                state == Some(sim_core::device::VrrpState::Master),
                match state {
                    Some(sim_core::device::VrrpState::Master) => {
                        format!("VRRP 组 {} 为 Master", vrid)
                    }
                    Some(sim_core::device::VrrpState::Backup) => {
                        format!("VRRP 组 {} 为 Backup", vrid)
                    }
                    Some(sim_core::device::VrrpState::Init) => {
                        format!("VRRP 组 {} 未收敛（Init）", vrid)
                    }
                    None => format!("VRRP 组 {} 未配置", vrid),
                },
            )
        }
        CheckKind::OspfNeighbors { dev, min } => {
            let n = eng.ospf_neighbors_of(*dev).len();
            (n >= *min, format!("OSPF 邻居 {} 个（要求 ≥{}）", n, min))
        }
        CheckKind::SecurityZone { dev, port, zone } => {
            let actual = eng
                .net
                .device(*dev)
                .and_then(|d| d.port(*port))
                .and_then(|p| p.zone.clone());
            (
                actual.as_deref() == Some(zone.as_str()),
                match actual {
                    Some(z) => format!("接口区域 = {}", z),
                    None => "接口未划分安全区域".into(),
                },
            )
        }
        CheckKind::SecurityPolicy {
            dev,
            name,
            src_zone,
            dst_zone,
            permit,
        } => {
            let found = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Router(rs) => rs
                    .security_policies
                    .iter()
                    .find(|p| p.name == *name)
                    .cloned(),
                _ => None,
            });
            match found {
                Some(p) => (
                    p.src_zone == *src_zone && p.dst_zone == *dst_zone && p.permit == *permit,
                    format!(
                        "策略 {}：{} → {} {}",
                        p.name,
                        p.src_zone,
                        p.dst_zone,
                        if p.permit { "permit" } else { "deny" }
                    ),
                ),
                None => (false, format!("策略 {} 未配置", name)),
            }
        }
        CheckKind::IpsecTunnel { dev, name } => {
            let found = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Router(rs) => {
                    rs.ipsec_tunnels.iter().find(|t| t.name == *name).cloned()
                }
                _ => None,
            });
            (
                found.is_some(),
                found
                    .map(|t| format!("IPsec 隧道 {} 已配置", t.name))
                    .unwrap_or_else(|| format!("IPsec 隧道 {} 未配置", name)),
            )
        }
        CheckKind::VpnConnected { dev } => {
            let connected = eng.net.device(*dev).map(|d| match &d.state {
                sim_core::device::DeviceState::Host(hs) => hs.vpn.is_some(),
                _ => false,
            });
            (
                connected == Some(true),
                match connected {
                    Some(true) => "SSL VPN 已连接".into(),
                    _ => "SSL VPN 未连接".into(),
                },
            )
        }
        CheckKind::WlanApJoined { ac, ap } => {
            let ac_ip = eng
                .net
                .device(*ac)
                .and_then(|d| d.ports.iter().find_map(|p| p.ip));
            let joined = eng.net.device(*ap).map(|d| match (&d.kind, ac_ip) {
                (sim_core::device::DeviceKind::AccessPoint, Some(ip)) => {
                    d.wlan.joined && d.wlan.ac_ip == Some(ip)
                }
                _ => false,
            });
            (
                joined == Some(true),
                match joined {
                    Some(true) => "AP 已加入 AC".into(),
                    _ => "AP 未加入 AC".into(),
                },
            )
        }
        CheckKind::WlanSsid { ac, ssid } => {
            let actual = eng.net.device(*ac).and_then(|d| {
                d.wlan
                    .profiles
                    .iter()
                    .find(|p| p.ssid == *ssid)
                    .map(|p| p.ssid.clone())
            });
            (
                actual.as_deref() == Some(ssid.as_str()),
                match actual {
                    Some(s) => format!("SSID = {}", s),
                    None => "未配置 SSID".into(),
                },
            )
        }
        CheckKind::WifiConnected { dev } => {
            let connected = eng.net.device(*dev).map(|d| match &d.state {
                sim_core::device::DeviceState::Host(hs) => hs.wifi.is_some(),
                _ => false,
            });
            (
                connected == Some(true),
                match connected {
                    Some(true) => "Wi-Fi 已关联".into(),
                    _ => "Wi-Fi 未关联".into(),
                },
            )
        }
        CheckKind::WifiOnAp { dev, ap } => {
            let current = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Host(hs) => hs.wifi.as_ref().map(|w| w.ap),
                _ => None,
            });
            (
                current == Some(*ap),
                match current {
                    Some(a) => format!("当前关联 AP id={}", a),
                    None => "未关联任何 AP".into(),
                },
            )
        }
        CheckKind::VxlanVni { dev, vlan, vni } => {
            let found = eng
                .net
                .device(*dev)
                .map(|d| match &d.state {
                    sim_core::device::DeviceState::Switch(sw) => sw
                        .vxlan
                        .mappings
                        .iter()
                        .any(|(v, vl)| *v == *vni && *vl == *vlan),
                    _ => false,
                })
                .unwrap_or(false);
            (
                found,
                if found {
                    format!("VNI {} <-> VLAN {}", vni, vlan)
                } else {
                    format!("VNI {} <-> VLAN {} 未配置", vni, vlan)
                },
            )
        }
        CheckKind::StackMember {
            dev,
            domain,
            member,
        } => {
            let found = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => sw
                    .stack
                    .as_ref()
                    .map(|s| s.domain == *domain && s.member == *member)
                    .filter(|ok| *ok),
                _ => None,
            });
            (
                found.is_some(),
                if found.is_some() {
                    format!("Stack domain {} member {}", domain, member)
                } else {
                    format!("Stack domain {} member {} 未配置", domain, member)
                },
            )
        }
        CheckKind::MlagDomain { dev, domain } => {
            let found = eng.net.device(*dev).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Switch(sw) => sw
                    .mlag
                    .as_ref()
                    .map(|m| m.domain == *domain)
                    .filter(|ok| *ok),
                _ => None,
            });
            (
                found.is_some(),
                if found.is_some() {
                    format!("M-LAG domain {}", domain)
                } else {
                    format!("M-LAG domain {} 未配置", domain)
                },
            )
        }
        CheckKind::EvpnRoute { dev, vtep, vni } => {
            let found = eng
                .evpn_routes_of(*dev)
                .into_iter()
                .any(|r| r.vtep == *vtep && r.vni == *vni);
            (
                found,
                if found {
                    format!("EVPN 路由 {}/{} 已学习", ipv4_str(*vtep), vni)
                } else {
                    format!("EVPN 路由 {}/{} 未学习", ipv4_str(*vtep), vni)
                },
            )
        }
        CheckKind::LtmVirtual {
            dev,
            name,
            vip,
            port,
        } => {
            let found = eng.net.device(*dev).is_some_and(|d| {
                d.ltm
                    .virtuals
                    .iter()
                    .any(|v| v.name == *name && v.vip == *vip && v.port == *port)
            });
            (
                found,
                if found {
                    format!("虚拟服务器 {} {}:{}", name, ipv4_str(*vip), port)
                } else {
                    format!("虚拟服务器 {} 未配置", name)
                },
            )
        }
        CheckKind::TcpProbe {
            from,
            to,
            port,
            expect,
        } => {
            eng.start_tcp_probe(*from, *to, *port);
            eng.run_to_quiescence(PING_HORIZON_NS);
            let opened = eng.net.device(*from).and_then(|d| match &d.state {
                sim_core::device::DeviceState::Host(hs) => {
                    hs.tcp_probes.last().and_then(|p| p.opened)
                }
                _ => None,
            });
            let ok = opened == Some(true);
            (
                ok == *expect,
                if ok {
                    format!("TCP {}:{} 连接成功", ipv4_str(*to), port)
                } else {
                    format!("TCP {}:{} 未建立", ipv4_str(*to), port)
                },
            )
        }
        CheckKind::PortIsolate { dev, port, group } => {
            let got = eng
                .net
                .device(*dev)
                .and_then(|d| d.port(*port))
                .and_then(|p| p.isolate_group);
            (
                got == Some(*group),
                match got {
                    Some(g) => format!("隔离组 {}", g),
                    None => "未配置端口隔离".into(),
                },
            )
        }
        CheckKind::MuxRole { dev, port, role } => {
            let got = eng
                .net
                .device(*dev)
                .and_then(|d| d.port(*port))
                .and_then(|p| p.mux_role);
            (got == Some(*role), format!("MUX 角色 {:?}", got))
        }
        CheckKind::Ospfv3Neighbors { dev, min } => {
            let n = eng.ospfv3_neighbors_of(*dev).len();
            (n >= *min, format!("OSPFv3 邻居 {}", n))
        }
        CheckKind::Srv6Route { dev, dst, len } => {
            let found = eng.net.device(*dev).is_some_and(|d| {
                matches!(&d.state, sim_core::device::DeviceState::Router(rs)
                    if rs.srv6.routes.iter().any(|r| r.dst == *dst && r.len == *len))
            });
            (
                found,
                if found {
                    format!("SRv6 路由 {}/{} 存在", sim_packets::ipv6_str(dst), len)
                } else {
                    format!("SRv6 路由 {}/{} 缺失", sim_packets::ipv6_str(dst), len)
                },
            )
        }
        CheckKind::Ping6 { from, to, expect } => {
            let ok = ping6_ok(eng, *from, *to);
            (
                ok == *expect,
                if ok {
                    format!("ping6 {} 连通", sim_packets::ipv6_str(to))
                } else {
                    format!("ping6 {} 不通", sim_packets::ipv6_str(to))
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
fn ping6_ok(eng: &mut Engine, from: DevId, to: Ipv6Addr) -> bool {
    let Some(sid) = eng.start_ping6(from, to) else {
        return false;
    };
    eng.run_to_quiescence(PING_HORIZON_NS);
    let sessions = eng.take_unreported_ping6(from);
    let Some(s) = sessions.into_iter().find(|s| s.id == sid) else {
        return false;
    };
    s.received == sim_core::device::Ping6Session::TOTAL
}

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
