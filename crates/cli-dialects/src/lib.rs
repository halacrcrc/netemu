//! 三方言 CLI：IOS（思科）、VRP（华为）、Comware（新华三）。
//!
//! 共享的处理器在 `cmd` 模块；方言差异体现在：
//! 1. 命令树（命令词、参数形式：IOS 点分掩码 vs VRP/Comware 前缀长度）
//! 2. 接口名规则（见 ifname）
//! 3. 报错风格与 show 输出格式（见 show）
//! 4. 模式体系（IOS 多一层特权模式）

pub mod huawei;
pub mod ifname;
pub mod ios;
pub mod pc;
pub mod show;

use std::collections::HashMap;
use std::sync::OnceLock;

use cli_engine::{CliError, Ctx, HelpList, LoginFlow, LoginStep, Mode, RemoteSession, Session};
use sim_core::device::{DevId, DeviceKind, DeviceState, Dialect, MgmtProto, VtyTransport};
use sim_core::engine::Engine;

/// 供 pc.rs 复用的 IPv4 工具重导出。
pub use sim_packets::{ipv4_from_str, ipv4_str};

pub struct Trees {
    pub exec: Vec<cli_engine::CNode>,
    pub priv_: Vec<cli_engine::CNode>,
    pub config: Vec<cli_engine::CNode>,
    pub if_: Vec<cli_engine::CNode>,
    pub pool: Vec<cli_engine::CNode>,
    pub acl: Vec<cli_engine::CNode>,
    pub line: Vec<cli_engine::CNode>,
    pub aaa: Vec<cli_engine::CNode>,
    pub mst: Vec<cli_engine::CNode>,
    pub ospf: Vec<cli_engine::CNode>,
    pub ospf_area: Vec<cli_engine::CNode>,
    pub bgp: Vec<cli_engine::CNode>,
    pub isis: Vec<cli_engine::CNode>,
    pub route_policy: Vec<cli_engine::CNode>,
    pub zone: Vec<cli_engine::CNode>,
    pub security_policy: Vec<cli_engine::CNode>,
    pub security_rule: Vec<cli_engine::CNode>,
    pub ipsec: Vec<cli_engine::CNode>,
    pub ospfv3: Vec<cli_engine::CNode>,
}

impl Trees {
    pub fn root_for(&self, mode: Mode) -> &[cli_engine::CNode] {
        match mode {
            Mode::Exec => &self.exec,
            Mode::Priv => &self.priv_,
            Mode::Config => &self.config,
            Mode::If => &self.if_,
            Mode::Pool => &self.pool,
            Mode::Acl => &self.acl,
            Mode::Line => &self.line,
            Mode::Aaa => &self.aaa,
            Mode::Mst => &self.mst,
            Mode::Ospf => &self.ospf,
            Mode::OspfArea => &self.ospf_area,
            Mode::Bgp => &self.bgp,
            Mode::Isis => &self.isis,
            Mode::RoutePolicy => &self.route_policy,
            Mode::Zone => &self.zone,
            Mode::SecurityPolicy => &self.security_policy,
            Mode::SecurityRule => &self.security_rule,
            Mode::Ipsec => &self.ipsec,
            Mode::Ospfv3 => &self.ospfv3,
        }
    }
}

pub fn trees(d: Dialect) -> &'static Trees {
    static MAP: OnceLock<HashMap<Dialect, Trees>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert(Dialect::Ios, ios::build());
        m.insert(Dialect::Vrp, huawei::build());
        m.insert(Dialect::Comware, huawei::build());
        m
    })
    .get(&d)
    .unwrap()
}

// ---------- 共享命令处理器 ----------

/// 各方言通用的命令行为；个别方言差异通过参数控制。
pub mod cmd {
    use super::*;
    use crate::show;
    use cli_engine::IfCtx;
    use sim_core::device::{
        Acl, AclProto, AclRule, BgpPeerPolicy, IpsecTunnel, LtmMember, LtmPool, LtmVirtual,
        MlagConfig, RoutePolicy, RoutePolicyNode, SecurityPolicy, StackConfig, WlanProfile,
        WlanSecurity,
    };
    use sim_packets::mask_from_prefix;

    pub fn hostname(c: &mut Ctx) -> Result<String, CliError> {
        let name = c.word("name");
        if name.is_empty() {
            return Err(CliError::Custom("主机名不能为空".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.name = name;
        }
        Ok(String::new())
    }

    pub fn interface(c: &mut Ctx) -> Result<String, CliError> {
        let raw = c.word("if");
        let kind = c.device_kind();
        match ifname::resolve_if(c.dialect, kind, &raw) {
            Some(ifref) => {
                c.sess.mode = Mode::If;
                c.sess.cur_if = Some(match ifref {
                    ifname::IfRef::Physical(idx) => IfCtx::Physical(idx),
                    ifname::IfRef::Svi(v) => IfCtx::Svi(v),
                    ifname::IfRef::Lag(id) => IfCtx::Lag(id),
                });
                Ok(String::new())
            }
            None => Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid interface type and number".into(),
                _ => "Wrong parameter found at '^' position.".into(),
            })),
        }
    }

    /// `interface vlan 10`（IOS）/ `interface vlanif 20`（VRP）/ `interface vlan-interface 20`（Comware）
    /// 两段式 SVI 接口名。
    pub fn interface_svi(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Switch | DeviceKind::AccessPoint
        ) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid interface type and number".into(),
                _ => "Wrong parameter found at '^' position.".into(),
            }));
        }
        let v = c
            .number("vlan")
            .ok_or_else(|| CliError::Custom("Invalid VLAN ID".into()))?;
        let vlan = (1..=4094)
            .contains(&v)
            .then_some(v as u16)
            .ok_or_else(|| CliError::Custom("Invalid VLAN ID".into()))?;
        c.sess.mode = Mode::If;
        c.sess.cur_if = Some(IfCtx::Svi(vlan));
        Ok(String::new())
    }

    /// `interface Port-channel1` / `interface Eth-Trunk 1` / `interface Bridge-Aggregation 1`
    pub fn interface_lag(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let id = c
            .number("lag")
            .filter(|n| (1..=128).contains(n))
            .ok_or_else(|| CliError::Custom("Invalid aggregation group number".into()))?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.ensure_lag(id as u16);
            }
        }
        c.sess.mode = Mode::If;
        c.sess.cur_if = Some(IfCtx::Lag(id as u16));
        Ok(String::new())
    }

    fn bind_lag(
        c: &mut Ctx,
        gid: u32,
        mode: sim_core::device::LagMode,
    ) -> Result<String, CliError> {
        require_switch(c)?;
        if !(1..=128).contains(&gid) {
            return Err(CliError::Custom("Invalid aggregation group number".into()));
        }
        let Some(IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom(
                "Aggregation can only be configured on physical interfaces".into(),
            ));
        };
        if c.eng.net.lag_bind(c.dev, port, gid as u16, mode) {
            c.eng.lacp_config_changed(c.dev);
        }
        Ok(String::new())
    }

    /// IOS：`channel-group <n> mode active|passive|on`（省略 mode 等价 on）。
    pub fn lag_bind_ios(c: &mut Ctx) -> Result<String, CliError> {
        let gid = c.number("group").unwrap_or(0);
        let mode = match c.word("mode").to_lowercase().as_str() {
            "active" => sim_core::device::LagMode::LacpActive,
            "passive" => sim_core::device::LagMode::LacpPassive,
            "on" | "" => sim_core::device::LagMode::Static,
            _ => return Err(CliError::Custom("Invalid channel-group mode".into())),
        };
        bind_lag(c, gid, mode)
    }

    /// VRP：成员口 `eth-trunk <n>` / Comware：`port link-aggregation group <n>`（动态聚合）。
    pub fn lag_bind_dynamic(c: &mut Ctx) -> Result<String, CliError> {
        let gid = c.number("group").unwrap_or(0);
        bind_lag(c, gid, sim_core::device::LagMode::LacpActive)
    }

    /// 聚合口视图：切换工作模式（LACP 主动 / 静态）。
    pub fn lag_mode_lacp(c: &mut Ctx) -> Result<String, CliError> {
        lag_set_mode(c, sim_core::device::LagMode::LacpActive)
    }

    pub fn lag_mode_static(c: &mut Ctx) -> Result<String, CliError> {
        lag_set_mode(c, sim_core::device::LagMode::Static)
    }

    fn lag_set_mode(c: &mut Ctx, mode: sim_core::device::LagMode) -> Result<String, CliError> {
        require_switch(c)?;
        let Some(IfCtx::Lag(gid)) = c.sess.cur_if else {
            return Err(CliError::Custom(
                "This command is only valid in an aggregation interface view".into(),
            ));
        };
        if c.eng.net.set_lag_mode(c.dev, gid, mode) {
            c.eng.lacp_config_changed(c.dev);
        }
        Ok(String::new())
    }

    /// 成员口退出聚合组（`no channel-group` / `undo eth-trunk` / `undo port link-aggregation group`）。
    pub fn lag_unbind(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let Some(IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom(
                "This command is only valid on physical interfaces".into(),
            ));
        };
        if c.eng.net.lag_unbind(c.dev, port).is_some() {
            c.eng.lacp_config_changed(c.dev);
        }
        Ok(String::new())
    }

    pub fn show_lag(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::lag_table(c.eng, c.dev, c.dialect))
    }

    fn require_if(c: &Ctx) -> Result<IfCtx, CliError> {
        c.sess
            .cur_if
            .ok_or_else(|| CliError::Custom("no interface selected".into()))
    }

    pub fn ip_address(c: &mut Ctx) -> Result<String, CliError> {
        let ifc = require_if(c)?;
        let ip = c.ipv4("ip");
        let len = c.mask_len("mask");
        if ip == 0 {
            return Err(CliError::Custom("Invalid IP address".into()));
        }
        match ifc {
            IfCtx::Physical(port) => {
                if matches!(
                    c.device_kind(),
                    DeviceKind::Switch | DeviceKind::AccessPoint
                ) {
                    return Err(CliError::Custom(
                        "This command is not supported on Layer 2 interfaces".into(),
                    ));
                }
                c.eng.net.set_port_ip(c.dev, port, Some(ip), len);
            }
            IfCtx::Svi(vlan) => {
                if !c.eng.net.set_svi(c.dev, vlan, Some(ip), len) {
                    return Err(CliError::Custom(
                        "Command is only available on switches".into(),
                    ));
                }
            }
            IfCtx::Lag(_) => {
                return Err(CliError::Custom(
                    "This command is not supported on Layer 2 interfaces".into(),
                ));
            }
        }
        Ok(String::new())
    }

    pub fn no_ip_address(c: &mut Ctx) -> Result<String, CliError> {
        let ifc = require_if(c)?;
        match ifc {
            IfCtx::Physical(port) => c.eng.net.set_port_ip(c.dev, port, None, 24),
            IfCtx::Svi(vlan) => {
                c.eng.net.set_svi(c.dev, vlan, None, 24);
            }
            IfCtx::Lag(_) => {}
        }
        Ok(String::new())
    }

    pub fn shutdown(c: &mut Ctx, up: bool) -> Result<String, CliError> {
        let ifc = require_if(c)?;
        match ifc {
            IfCtx::Physical(port) => {
                c.eng.net.set_port_admin(c.dev, port, up);
                c.eng.stp_topology_changed();
            }
            IfCtx::Svi(vlan) => {
                c.eng.net.set_svi_admin(c.dev, vlan, up);
            }
            IfCtx::Lag(gid) => {
                let members = c
                    .eng
                    .net
                    .device(c.dev)
                    .and_then(|d| match &d.state {
                        DeviceState::Switch(sw) => sw.lags.get(&gid).map(|g| g.members.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                for m in members {
                    c.eng.net.set_port_admin(c.dev, m, up);
                }
                c.eng.lacp_config_changed(c.dev);
            }
        }
        Ok(String::new())
    }

    // ---------- VLAN（交换机） ----------

    fn require_switch(c: &Ctx) -> Result<(), CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Switch | DeviceKind::AccessPoint
        ) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on switches".into(),
            }));
        }
        Ok(())
    }

    fn require_vlan_id(v: u32) -> Result<u16, CliError> {
        if (1..=4094).contains(&v) {
            Ok(v as u16)
        } else {
            Err(CliError::Custom("Invalid VLAN ID".into()))
        }
    }

    /// 创建 VLAN：IOS `vlan 10`、Comware `vlan 10`。
    pub fn create_vlans(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let v = c
            .number("vlan")
            .ok_or_else(|| CliError::Custom("Invalid VLAN ID".into()))?;
        let vlan = require_vlan_id(v)?;
        c.eng.net.ensure_vlan(c.dev, vlan);
        Ok(String::new())
    }

    /// 批量创建 VLAN：VRP `vlan batch 10 20` / `vlan batch 10-12`。
    pub fn create_vlans_list(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let list = c
            .vlan_list("vlans")
            .ok_or_else(|| CliError::Custom("Invalid VLAN list".into()))?;
        for vlan in list {
            c.eng.net.ensure_vlan(c.dev, vlan);
        }
        Ok(String::new())
    }

    /// VLAN 命名（可选；M1 名字仅接受不落库）。
    pub fn vlan_name(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(String::new())
    }

    /// `switchport mode access`
    pub fn sw_mode_access(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let cli_engine::IfCtx::Physical(port) = require_if(c)? else {
            return Err(CliError::Custom(
                "This command is not supported on VLAN interfaces".into(),
            ));
        };
        c.eng
            .net
            .set_port_mode(c.dev, port, sim_core::device::PortMode::Access);
        Ok(String::new())
    }

    /// `switchport mode trunk`（思科默认放行全部 VLAN）
    pub fn sw_mode_trunk(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let cli_engine::IfCtx::Physical(port) = require_if(c)? else {
            return Err(CliError::Custom(
                "This command is not supported on VLAN interfaces".into(),
            ));
        };
        c.eng.net.set_port_trunk_allowed(c.dev, port, &[]);
        Ok(String::new())
    }

    /// 华为/华三 `port link-type access`
    pub fn lt_access(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let cli_engine::IfCtx::Physical(port) = require_if(c)? else {
            return Err(CliError::Custom(
                "This command is not supported on VLAN interfaces".into(),
            ));
        };
        c.eng
            .net
            .set_port_mode(c.dev, port, sim_core::device::PortMode::Access);
        Ok(String::new())
    }

    /// 华为/华三 `port link-type trunk`（默认仅放行 VLAN1，待 allow-pass 扩充）
    pub fn lt_trunk(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let cli_engine::IfCtx::Physical(port) = require_if(c)? else {
            return Err(CliError::Custom(
                "This command is not supported on VLAN interfaces".into(),
            ));
        };
        c.eng.net.set_port_trunk_allowed(c.dev, port, &[1]);
        Ok(String::new())
    }

    pub fn access_vlan(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let cli_engine::IfCtx::Physical(port) = require_if(c)? else {
            return Err(CliError::Custom(
                "This command is not supported on VLAN interfaces".into(),
            ));
        };
        let v = c
            .number("vlan")
            .ok_or_else(|| CliError::Custom("Invalid VLAN ID".into()))?;
        let vlan = require_vlan_id(v)?;
        c.eng.net.set_port_access_vlan(c.dev, port, vlan);
        Ok(String::new())
    }

    /// Trunk 放行列表（"10,20" / "10 20" / "10-12" 混合）。
    pub fn trunk_allow(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let cli_engine::IfCtx::Physical(port) = require_if(c)? else {
            return Err(CliError::Custom(
                "This command is not supported on VLAN interfaces".into(),
            ));
        };
        let vlans = match c.val("vlans") {
            Some(cli_engine::Val::VlanList(v)) => v.clone(),
            Some(cli_engine::Val::Word(w)) | Some(cli_engine::Val::Raw(w)) => {
                cli_engine::parse_vlan_list(w)
                    .ok_or_else(|| CliError::Custom("Invalid VLAN list".into()))?
            }
            _ => return Err(CliError::Custom("Invalid VLAN list".into())),
        };
        // 华为/华三进入 trunk 时默认收敛为 [1]，显式 allow-pass 后改为完整列表（保留 1）
        let merged: Vec<u16> = match c.dialect {
            Dialect::Ios => vlans,
            _ => {
                let mut v = vlans;
                if !v.contains(&1) {
                    v.push(1);
                }
                v
            }
        };
        c.eng.net.set_port_trunk_allowed(c.dev, port, &merged);
        Ok(String::new())
    }

    // ---------- STP ----------

    fn stp_of<'a>(c: &'a mut Ctx<'_>) -> Result<&'a mut sim_core::device::StpInstance, CliError> {
        if c.device_kind() != DeviceKind::Switch {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on switches".into(),
            }));
        }
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Err(CliError::Custom("device not found".into()));
        };
        match &mut d.state {
            sim_core::device::DeviceState::Switch(sw) => Ok(&mut sw.stp),
            _ => Err(CliError::Custom("not a switch".into())),
        }
    }

    /// IOS `spanning-tree vlan <n> priority <p>` / VRP、Comware `stp priority <p>`
    pub fn stp_priority(c: &mut Ctx) -> Result<String, CliError> {
        let p = c.number("priority").unwrap_or(32768);
        if p > 61440 || !p.is_multiple_of(4096) {
            return Err(CliError::Custom(
                "Priority must be a multiple of 4096, 0-61440".into(),
            ));
        }
        stp_of(c)?.priority = p as u16;
        c.eng.stp_topology_changed();
        Ok(String::new())
    }

    /// IOS `spanning-tree vlan <n>` / VRP、Comware `stp enable`
    pub fn stp_enable(c: &mut Ctx, up: bool) -> Result<String, CliError> {
        stp_of(c)?.enabled = up;
        c.eng.stp_topology_changed();
        Ok(String::new())
    }

    fn stp_set_mode(c: &mut Ctx, mode: sim_core::device::StpMode) -> Result<String, CliError> {
        stp_of(c)?.mode = mode;
        c.eng.stp_topology_changed();
        Ok(String::new())
    }

    /// IOS `spanning-tree mode rapid-pvst`
    pub fn stp_mode_rstp(c: &mut Ctx) -> Result<String, CliError> {
        stp_set_mode(c, sim_core::device::StpMode::Rstp)
    }

    /// IOS `spanning-tree mode pvst` / VRP `stp mode stp`
    pub fn stp_mode_stp(c: &mut Ctx) -> Result<String, CliError> {
        stp_set_mode(c, sim_core::device::StpMode::Dot1d)
    }

    /// IOS `spanning-tree mode mst` / VRP `stp mode mstp`
    pub fn stp_mode_mst(c: &mut Ctx) -> Result<String, CliError> {
        stp_set_mode(c, sim_core::device::StpMode::Mstp)
    }

    /// 进入 MSTP 域配置视图（`spanning-tree mst configuration` / `stp region-configuration`）
    pub fn stp_enter_mst(c: &mut Ctx) -> Result<String, CliError> {
        stp_of(c)?;
        c.sess.mode = Mode::Mst;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    /// 实例优先级：`spanning-tree mst 1 priority 4096` / `stp instance 1 priority 4096`
    pub fn stp_mst_priority(c: &mut Ctx) -> Result<String, CliError> {
        let id = c.number("id").unwrap_or(0);
        if !(1..=4094).contains(&id) {
            return Err(CliError::Custom("Invalid MST instance id".into()));
        }
        let p = c.number("priority").unwrap_or(32768);
        if p > 61440 || !p.is_multiple_of(4096) {
            return Err(CliError::Custom(
                "Priority must be a multiple of 4096, 0-61440".into(),
            ));
        }
        stp_of(c)?.ensure_mst(id as u16).priority = p as u16;
        c.eng.stp_topology_changed();
        Ok(String::new())
    }

    /// 实例映射 VLAN：`instance 1 vlan 10 20`
    pub fn stp_mst_instance_vlan(c: &mut Ctx) -> Result<String, CliError> {
        let id = c.number("id").unwrap_or(0);
        if !(1..=4094).contains(&id) {
            return Err(CliError::Custom("Invalid MST instance id".into()));
        }
        let vlans = c
            .vlan_list("vlans")
            .ok_or_else(|| CliError::Custom("Invalid VLAN list".into()))?;
        let stp = stp_of(c)?;
        // 同一 VLAN 只能属于一个实例：先从其它实例摘除
        for m in stp.mst.values_mut() {
            m.vlans.retain(|v| !vlans.contains(v));
        }
        for v in &vlans {
            stp.vlan_instance.insert(*v, id as u16);
        }
        let m = stp.ensure_mst(id as u16);
        m.vlans = vlans;
        m.vlans.sort_unstable();
        m.vlans.dedup();
        stp.mode = sim_core::device::StpMode::Mstp;
        c.eng.stp_topology_changed();
        Ok(String::new())
    }

    /// MSTP 域名/修订号等仅接受不落库（无域协商）。
    pub fn stp_mst_noop(_c: &mut Ctx) -> Result<String, CliError> {
        Ok(String::new())
    }

    /// 接口成本：IOS `spanning-tree vlan <n> cost <c>` / VRP、Comware `stp cost <c>`
    pub fn stp_cost(c: &mut Ctx) -> Result<String, CliError> {
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let cost = c.number("cost").unwrap_or(4);
        if cost == 0 || cost > 200_000_000 {
            return Err(CliError::Custom("Invalid cost".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(pt) = d.port_mut(port) {
                pt.stp_cost = Some(cost);
            }
        }
        c.eng.stp_topology_changed();
        Ok(String::new())
    }

    // ---------- DHCP 服务器 ----------

    /// IOS `ip dhcp pool NAME` → Pool 模式；VRP/Comware `ip pool NAME` / `dhcp server ip-pool NAME`
    pub fn dhcp_pool_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on routers".into(),
            }));
        }
        let name = c.word("name");
        if name.is_empty() {
            return Err(CliError::Custom("Pool name required".into()));
        }
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Err(CliError::Custom("device not found".into()));
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        if !rs.dhcp_pools.iter().any(|p| p.name == name) {
            rs.dhcp_pools.push(sim_core::device::DhcpPool {
                name: name.clone(),
                network: 0,
                prefix: 24,
                gateway: 0,
                dns: 0,
            });
        }
        c.sess.mode = Mode::Pool;
        c.sess.sub.pool = Some(name);
        Ok(String::new())
    }

    /// Pool 视图：IOS `network A.B.C.D mask` / VRP、Comware `network A.B.C.D mask X` + gateway-list
    pub fn pool_network(c: &mut Ctx) -> Result<String, CliError> {
        let net_ip = c.ipv4("net");
        let len = c.mask_len("mask");
        let Some(name) = c.sess.sub.pool.clone() else {
            return Err(CliError::Custom("no pool selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        match rs.dhcp_pools.iter_mut().find(|p| p.name == name) {
            Some(p) => {
                p.network = net_ip & sim_packets::mask_from_prefix(len);
                p.prefix = len;
            }
            None => return Err(CliError::Custom("pool not found".into())),
        }
        Ok(String::new())
    }

    pub fn pool_gateway(c: &mut Ctx) -> Result<String, CliError> {
        let gw = c.ipv4("gw");
        let Some(name) = c.sess.sub.pool.clone() else {
            return Err(CliError::Custom("no pool selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        match rs.dhcp_pools.iter_mut().find(|p| p.name == name) {
            Some(p) => p.gateway = gw,
            None => return Err(CliError::Custom("pool not found".into())),
        }
        Ok(String::new())
    }

    pub fn pool_dns(c: &mut Ctx) -> Result<String, CliError> {
        let dns = c.ipv4("dns");
        let Some(name) = c.sess.sub.pool.clone() else {
            return Err(CliError::Custom("no pool selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        match rs.dhcp_pools.iter_mut().find(|p| p.name == name) {
            Some(p) => p.dns = dns,
            None => return Err(CliError::Custom("pool not found".into())),
        }
        Ok(String::new())
    }

    /// 接口启用 DHCP 服务器（VRP `dhcp select global` / Comware `dhcp select server`；IOS 无需）
    pub fn dhcp_select(_c: &mut Ctx) -> Result<String, CliError> {
        Ok(String::new()) // M2b：池按接口网段自动匹配，接受并忽略
    }

    // ---------- ACL ----------

    /// VRP/Comware `acl [basic|advanced] <n>` → Acl 模式
    pub fn acl_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        // `acl 3000`：数字被 Word(kind) 参数吞掉的情况 → 从 kind 解析编号
        let (kind_word, n_from_kind) = match c.val("kind") {
            Some(cli_engine::Val::Word(w)) => match w.parse::<u32>() {
                Ok(n2) => (None, Some(n2)),
                Err(_) => (Some(w.clone()), None),
            },
            _ => (None, None),
        };
        let n = c.number("number").or(n_from_kind).unwrap_or(0);
        let advanced = match &kind_word {
            Some(w) => w.eq_ignore_ascii_case("advanced"),
            None => (3000..=3999).contains(&n),
        };
        let ok_range = if advanced {
            (3000..=3999).contains(&n)
        } else {
            (2000..=2999).contains(&n) || (1..=199).contains(&n)
        };
        if !ok_range {
            return Err(CliError::Custom("Invalid ACL number".into()));
        }
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        if !rs.acls.iter().any(|a| a.number == n) {
            rs.acls.push(sim_core::device::Acl {
                number: n,
                rules: vec![],
            });
        }
        c.sess.mode = Mode::Acl;
        c.sess.sub.acl = Some((n, advanced));
        Ok(String::new())
    }

    /// ACL 视图：`rule {deny|permit} [icmp] source X W [destination X W]`
    pub fn acl_rule(c: &mut Ctx) -> Result<String, CliError> {
        acl_rule_with(c, None)
    }

    pub fn acl_rule_icmp(c: &mut Ctx) -> Result<String, CliError> {
        acl_rule_with(c, Some(AclProto::Icmp))
    }

    fn acl_rule_with(c: &mut Ctx, forced: Option<AclProto>) -> Result<String, CliError> {
        let Some((no, _adv)) = c.sess.sub.acl else {
            return Err(CliError::Custom("no acl selected".into()));
        };
        let permit = c.word("action").eq_ignore_ascii_case("permit");
        let proto = forced.unwrap_or(match c.val("proto") {
            Some(cli_engine::Val::Word(w)) if w.eq_ignore_ascii_case("icmp") => AclProto::Icmp,
            _ => AclProto::Any,
        });
        let src = c.ipv4("src");
        let src_w = c.ipv4("src_wildcard");
        let dst = c.ipv4("dst");
        let dst_w = c.ipv4("dst_wildcard");
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        let Some(acl) = rs.acls.iter_mut().find(|a| a.number == no) else {
            return Err(CliError::Custom("acl not found".into()));
        };
        acl.rules.push(AclRule {
            permit,
            proto,
            src,
            src_wildcard: src_w,
            dst,
            dst_wildcard: dst_w,
            hits: 0,
        });
        Ok(String::new())
    }

    /// IOS 全局：`access-list <n> {deny|permit} [icmp] A.B.C.D w [A.B.C.D w]`
    pub fn ios_access_list(c: &mut Ctx) -> Result<String, CliError> {
        ios_access_list_with(c, None)
    }

    pub fn ios_access_list_icmp(c: &mut Ctx) -> Result<String, CliError> {
        ios_access_list_with(c, Some(AclProto::Icmp))
    }

    fn ios_access_list_with(c: &mut Ctx, forced: Option<AclProto>) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom("Invalid input detected".into()));
        }
        let n = c.number("number").unwrap_or(0);
        let permit = c.word("action").eq_ignore_ascii_case("permit");
        let proto = forced.unwrap_or(match c.val("proto") {
            Some(cli_engine::Val::Word(w)) if w.eq_ignore_ascii_case("icmp") => AclProto::Icmp,
            _ => AclProto::Any,
        });
        let src = c.ipv4("src");
        let src_w = c.ipv4("src_wildcard");
        let dst = c.ipv4("dst");
        let dst_w = c.ipv4("dst_wildcard");
        // 基础 ACL (1-99) 只允许源
        let advanced = (100..=199).contains(&n);
        if !advanced && !(1..=99).contains(&n) {
            return Err(CliError::Custom(
                "ACL number must be 1-99 or 100-199".into(),
            ));
        }
        if !advanced && (dst != 0 || dst_w != 0) {
            return Err(CliError::Custom(
                "Basic ACL (1-99) cannot match destination".into(),
            ));
        }
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        let acl = match rs.acls.iter_mut().find(|a| a.number == n) {
            Some(a) => a,
            None => {
                rs.acls.push(Acl {
                    number: n,
                    rules: vec![],
                });
                rs.acls.last_mut().unwrap()
            }
        };
        acl.rules.push(AclRule {
            permit,
            proto,
            src,
            src_wildcard: src_w,
            dst,
            dst_wildcard: dst_w,
            hits: 0,
        });
        Ok(String::new())
    }

    /// 接口应用：IOS `ip access-group <n> in|out` / VRP `traffic-filter <dir> acl <n>` / Comware `packet-filter <n> <dir>`
    pub fn acl_apply(c: &mut Ctx) -> Result<String, CliError> {
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let n = c.number("acl").unwrap_or(0);
        let dir = c.word("dir");
        let inbound = dir.eq_ignore_ascii_case("in") || dir.eq_ignore_ascii_case("inbound");
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        if let Some(p) = d.port_mut(port) {
            if inbound {
                p.acl_in = Some(n);
            } else {
                p.acl_out = Some(n);
            }
        }
        Ok(String::new())
    }

    // ---------- NAT ----------

    /// 接口标记：IOS `ip nat inside`
    pub fn nat_mark_inside(c: &mut Ctx) -> Result<String, CliError> {
        nat_mark_side(c, true)
    }

    /// 接口标记：IOS `ip nat outside`
    pub fn nat_mark_outside(c: &mut Ctx) -> Result<String, CliError> {
        nat_mark_side(c, false)
    }

    fn nat_mark_side(c: &mut Ctx, inside: bool) -> Result<String, CliError> {
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        if inside {
            if !rs.nat_inside.contains(&port) {
                rs.nat_inside.push(port);
            }
            rs.nat_outside.retain(|&x| x != port);
        } else {
            if !rs.nat_outside.contains(&port) {
                rs.nat_outside.push(port);
            }
            rs.nat_inside.retain(|&x| x != port);
        }
        Ok(String::new())
    }

    /// VRP/Comware 接口：`nat outbound`（Easy-IP）
    pub fn nat_outbound(c: &mut Ctx) -> Result<String, CliError> {
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        if !rs.nat_outside.contains(&port) {
            rs.nat_outside.push(port);
        }
        rs.nat_inside.retain(|&x| x != port);
        Ok(String::new())
    }

    /// IOS 全局：`ip nat inside source list <n> interface <if> overload`（EasyIP）
    pub fn nat_easyip_ios(_c: &mut Ctx) -> Result<String, CliError> {
        // M2b：标记所有 ACL 允许的源经出口 NAT；出口由 inside/outside 口对推断，
        // 这里接受命令并把命令中的 interface 忽略（出口 = nat_outside 口）
        Ok(String::new())
    }

    /// 静态 NAT：IOS `ip nat inside source static A B` / VRP、Comware `nat static global B inside A`
    pub fn nat_static(c: &mut Ctx) -> Result<String, CliError> {
        let inside = c.ipv4("inside");
        let outside = c.ipv4("outside");
        if inside == 0 || outside == 0 {
            return Err(CliError::Custom("Invalid NAT addresses".into()));
        }
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        if !rs.nat_static.contains(&(inside, outside)) {
            rs.nat_static.push((inside, outside));
        }
        Ok(String::new())
    }

    // ---------- 管理面（本地用户 / VTY / 远程登录） ----------

    fn require_mgmt_device(c: &Ctx) -> Result<(), CliError> {
        if matches!(
            c.device_kind(),
            DeviceKind::Router | DeviceKind::Switch | DeviceKind::Firewall
        ) {
            Ok(())
        } else {
            Err(CliError::Custom(
                "Command is only available on network devices".into(),
            ))
        }
    }

    /// IOS：`username <name> [privilege <0-15>] secret <pw>`
    pub fn mgmt_username(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let name = c.word("name");
        let secret = c.word("secret");
        if name.is_empty() || secret.is_empty() {
            return Err(CliError::Custom("Invalid username or secret".into()));
        }
        let privilege = c.number("privilege").unwrap_or(1).min(15) as u8;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            let u = d.mgmt.ensure_user(&name);
            u.secret = secret;
            u.privilege = privilege;
        }
        Ok(String::new())
    }

    /// IOS：`enable secret <pw>`
    pub fn mgmt_enable_secret(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let secret = c.word("secret");
        if secret.is_empty() {
            return Err(CliError::Custom("Invalid secret".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.enable_secret = Some(secret);
        }
        Ok(String::new())
    }

    /// `line vty 0 4` / `user-interface vty 0 4` → Line 视图。
    pub fn mgmt_enter_line(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        c.sess.mode = Mode::Line;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    /// IOS：`login local`（启用本地用户认证）
    pub fn mgmt_login_local(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.auth_local = true;
        }
        Ok(String::new())
    }

    fn parse_transport(list: &[String]) -> Option<VtyTransport> {
        let words: Vec<String> = list.iter().map(|s| s.to_lowercase()).collect();
        if words.iter().any(|w| w == "none") {
            return Some(VtyTransport::None);
        }
        let ssh = words
            .iter()
            .any(|w| w == "ssh" || w == "all" || w == "ssh-only");
        let telnet = words.iter().any(|w| w == "telnet" || w == "all");
        match (ssh, telnet) {
            (true, true) => Some(VtyTransport::All),
            (true, false) => Some(VtyTransport::Ssh),
            (false, true) => Some(VtyTransport::Telnet),
            (false, false) => None,
        }
    }

    /// `transport input ssh|telnet|all|none` / `protocol inbound ...`
    pub fn mgmt_transport(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let list = c.word_list("protos").unwrap_or_default();
        let transport = parse_transport(&list)
            .ok_or_else(|| CliError::Custom("Invalid transport protocol".into()))?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.transport = transport;
        }
        Ok(String::new())
    }

    /// VRP：`aaa` → AAA 视图
    pub fn mgmt_enter_aaa(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        c.sess.mode = Mode::Aaa;
        c.sess.cur_if = None;
        c.sess.sub.aaa_user = None;
        Ok(String::new())
    }

    /// Comware：`local-user <name> class manage` → AAA 用户视图
    pub fn mgmt_local_user_enter(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let name = c.word("name");
        if name.is_empty() {
            return Err(CliError::Custom("Invalid username".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.ensure_user(&name);
        }
        c.sess.mode = Mode::Aaa;
        c.sess.cur_if = None;
        c.sess.sub.aaa_user = Some(name);
        Ok(String::new())
    }

    /// AAA 视图中定位用户：命令带 name 参数，否则用当前视图用户。
    fn aaa_user_name(c: &Ctx) -> Option<String> {
        let n = c.word("name");
        if !n.is_empty() {
            Some(n)
        } else {
            c.sess.sub.aaa_user.clone()
        }
    }

    /// `local-user <name> password cipher|simple <pw>` / AAA 视图 `password ...`
    pub fn mgmt_user_password(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let Some(name) = aaa_user_name(c) else {
            return Err(CliError::Custom("No local user selected".into()));
        };
        let secret = c.word("secret");
        if secret.is_empty() {
            return Err(CliError::Custom("Invalid password".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.ensure_user(&name).secret = secret;
        }
        Ok(String::new())
    }

    /// `local-user <name> privilege level <n>` / `authorization-attribute user-role ...`
    pub fn mgmt_user_privilege(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let Some(name) = aaa_user_name(c) else {
            return Err(CliError::Custom("No local user selected".into()));
        };
        let level = c.number("privilege").unwrap_or(1).min(15) as u8;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.ensure_user(&name).privilege = level;
        }
        Ok(String::new())
    }

    /// `authorization-attribute user-role network-admin|network-operator`（Comware）
    pub fn mgmt_user_role(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let Some(name) = aaa_user_name(c) else {
            return Err(CliError::Custom("No local user selected".into()));
        };
        let role = c.word("role").to_lowercase();
        let level = if role == "network-admin" { 15 } else { 1 };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.ensure_user(&name).privilege = level;
        }
        Ok(String::new())
    }

    /// `service-type telnet ssh` / `service-type ssh`
    pub fn mgmt_user_service(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let Some(name) = aaa_user_name(c) else {
            return Err(CliError::Custom("No local user selected".into()));
        };
        let words: Vec<String> = c
            .word_list("services")
            .unwrap_or_default()
            .iter()
            .map(|s| s.to_lowercase())
            .collect();
        let all = words.iter().any(|w| w == "all");
        let telnet = all || words.iter().any(|w| w == "telnet");
        let ssh = all || words.iter().any(|w| w == "ssh");
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            let u = d.mgmt.ensure_user(&name);
            u.telnet = telnet;
            u.ssh = ssh;
        }
        Ok(String::new())
    }

    /// Line 视图：`authentication-mode aaa|scheme|password|none`
    pub fn mgmt_auth_mode(c: &mut Ctx) -> Result<String, CliError> {
        require_mgmt_device(c)?;
        let mode = c.word("mode").to_lowercase();
        let local = matches!(mode.as_str(), "aaa" | "scheme");
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.mgmt.auth_local = local;
        }
        Ok(String::new())
    }

    fn login_start(c: &mut Ctx, proto: MgmtProto) -> Result<String, CliError> {
        let ip = c.ipv4("target");
        if ip == 0 {
            return Err(CliError::Custom("Invalid IP address".into()));
        }
        match c.eng.login_target(c.dev, ip, proto) {
            Ok(target) => {
                c.sess.login = Some(LoginFlow {
                    target,
                    target_ip: ip,
                    proto,
                    step: LoginStep::Username,
                    user: String::new(),
                });
                Ok(match (c.dialect, proto) {
                    (Dialect::Ios, MgmtProto::Telnet) => {
                        format!("Trying {} ... Open", ipv4_str(ip))
                    }
                    (Dialect::Ios, MgmtProto::Ssh) => format!("Connecting to {} ...", ipv4_str(ip)),
                    (_, _) => format!("  Trying {} ...", ipv4_str(ip)),
                })
            }
            Err(e) => Ok(login_error_text(c.device_kind(), c.dialect, e)),
        }
    }

    pub fn telnet_login(c: &mut Ctx) -> Result<String, CliError> {
        login_start(c, MgmtProto::Telnet)
    }

    pub fn ssh_login(c: &mut Ctx) -> Result<String, CliError> {
        login_start(c, MgmtProto::Ssh)
    }

    // ---------- OSPF（简化） ----------

    /// `router ospf 1` / `ospf 1`
    pub fn ospf_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        c.sess.mode = Mode::Ospf;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    /// VRP/Comware 区域视图：`area 0` / `area 0.0.0.0`
    pub fn ospf_area_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        c.sess.mode = Mode::OspfArea;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    /// `network A.B.C.D wildcard [area 0]`：加入 OSPF。
    pub fn ospf_network(c: &mut Ctx) -> Result<String, CliError> {
        let net = c.ipv4("net");
        if net == 0 {
            return Err(CliError::Custom("Invalid network".into()));
        }
        let wc = match c.val("wildcard") {
            Some(cli_engine::Val::Ip(w)) => *w,
            _ => 0,
        };
        let area = match c.val("area") {
            Some(cli_engine::Val::Number(n)) => *n,
            Some(cli_engine::Val::Word(w)) => w.replace('.', "").parse().unwrap_or(0),
            _ => 0,
        };
        if area != 0 {
            return Err(CliError::Custom(
                "Only area 0 is supported in this simulator".into(),
            ));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if !rs.ospf_networks.contains(&(net, wc)) {
                    rs.ospf_networks.push((net, wc));
                }
            }
        }
        c.eng.ospf_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn show_ospf_neighbor(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::ospf_neighbors(c.eng, c.dev, c.dialect))
    }

    pub fn ospfv3_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(c.device_kind(), DeviceKind::Router) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.ospfv3_enabled = true;
            }
        }
        c.sess.mode = Mode::Ospfv3;
        Ok(String::new())
    }

    pub fn ospfv3_router_id(c: &mut Ctx) -> Result<String, CliError> {
        let id = c.ipv4("id");
        if id == 0 {
            return Err(CliError::Custom("Invalid router id".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.ospfv3_router_id = Some(id);
            }
        }
        c.eng.ospfv3_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn ospfv3_area(c: &mut Ctx) -> Result<String, CliError> {
        let area = c.number("area").unwrap_or(0);
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let Some(p) = d.port_mut(port) else {
            return Ok(String::new());
        };
        p.ospfv3_area = Some(area);
        c.eng.ospfv3_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn show_ospfv3_neighbor(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::ospfv3_neighbors(c.eng, c.dev, c.dialect))
    }

    pub fn show_ospfv3_db(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::ospfv3_prefixes(c.eng, c.dev, c.dialect))
    }

    fn ipv6_prefix_arg(c: &mut Ctx, key: &str) -> Result<(sim_packets::Ipv6Addr, u8), CliError> {
        let raw = c.word(key);
        let (addr, len) = raw
            .split_once('/')
            .ok_or_else(|| CliError::Custom("Expected IPv6 prefix, e.g. 2001:db8::/64".into()))?;
        let ip = sim_packets::ipv6_from_str(addr)
            .ok_or_else(|| CliError::Custom("Invalid IPv6 address".into()))?;
        let prefix: u8 = len
            .parse()
            .map_err(|_| CliError::Custom("Invalid prefix length".into()))?;
        if prefix > 128 {
            return Err(CliError::Custom("Invalid prefix length".into()));
        }
        Ok((ip, prefix))
    }

    /// `srv6 locator 2001:db8:100::/64`
    pub fn srv6_locator(c: &mut Ctx) -> Result<String, CliError> {
        let (net, len) = ipv6_prefix_arg(c, "prefix")?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.srv6.locator = Some((net, len));
            }
        }
        Ok(String::new())
    }

    /// `srv6 sid 2001:db8:100::1`
    pub fn srv6_sid(c: &mut Ctx) -> Result<String, CliError> {
        let raw = c.word("sid");
        let sid = sim_packets::ipv6_from_str(&raw)
            .ok_or_else(|| CliError::Custom("Invalid SID".into()))?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if !rs.srv6.sids.contains(&sid) {
                    rs.srv6.sids.push(sid);
                }
            }
        }
        Ok(String::new())
    }

    /// `srv6 route 2001:db8:2::/64 sid 2001:db8:100::1`
    pub fn srv6_route(c: &mut Ctx) -> Result<String, CliError> {
        let (dst, len) = ipv6_prefix_arg(c, "prefix")?;
        let raw = c.word("sid");
        let sid = sim_packets::ipv6_from_str(&raw)
            .ok_or_else(|| CliError::Custom("Invalid SID".into()))?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.srv6.routes.retain(|r| !(r.dst == dst && r.len == len));
                rs.srv6
                    .routes
                    .push(sim_core::device::Srv6Route { dst, len, sid });
            }
        }
        Ok(String::new())
    }

    pub fn show_srv6(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::srv6_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_ospf_db(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::ospf_lsdb(c.eng, c.dev, c.dialect))
    }

    // ---------- VRRP（虚拟网关） ----------

    fn vrrp_if(c: &Ctx) -> Result<(sim_core::device::PortId, sim_packets::Ipv4Addr), CliError> {
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let ip = c
            .eng
            .net
            .device(c.dev)
            .and_then(|d| d.port(port))
            .and_then(|p| p.ip)
            .ok_or_else(|| {
                CliError::Custom("VRRP requires an IP address on the interface".into())
            })?;
        Ok((port, ip))
    }

    pub fn vrrp_vip(c: &mut Ctx) -> Result<String, CliError> {
        let vrid = c.number("vrid").unwrap_or(0);
        if !(1..=255).contains(&vrid) {
            return Err(CliError::Custom("VRRP group must be 1-255".into()));
        }
        let vip = c.ipv4("vip");
        if vip == 0 {
            return Err(CliError::Custom("Invalid virtual IP".into()));
        }
        let (port, _) = vrrp_if(c)?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if let Some(g) = rs.vrrp.iter_mut().find(|g| g.vrid == vrid as u8) {
                    g.vip = vip;
                    g.port = port;
                } else {
                    rs.vrrp.push(sim_core::device::VrrpGroup {
                        vrid: vrid as u8,
                        vip,
                        port,
                        priority: 100,
                        preempt: true,
                        state: sim_core::device::VrrpState::Init,
                        last_advert_ns: 0,
                    });
                }
            }
        }
        c.eng.vrrp_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn vrrp_priority(c: &mut Ctx) -> Result<String, CliError> {
        let vrid = c.number("vrid").unwrap_or(0);
        let priority = c.number("priority").unwrap_or(100);
        if !(1..=254).contains(&priority) {
            return Err(CliError::Custom("VRRP priority must be 1-254".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if let Some(g) = rs.vrrp.iter_mut().find(|g| g.vrid == vrid as u8) {
                    g.priority = priority as u8;
                }
            }
        }
        c.eng.vrrp_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn vrrp_preempt(c: &mut Ctx) -> Result<String, CliError> {
        let vrid = c.number("vrid").unwrap_or(0);
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if let Some(g) = rs.vrrp.iter_mut().find(|g| g.vrid == vrid as u8) {
                    g.preempt = true;
                }
            }
        }
        Ok(String::new())
    }

    pub fn show_vrrp(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::vrrp_table(c.eng, c.dev, c.dialect))
    }

    // ---------- IPv6（简化） ----------

    /// `ipv6 address 2001:db8::1/64`
    pub fn ipv6_address(c: &mut Ctx) -> Result<String, CliError> {
        let raw = c.word("addr");
        let (addr, len) = raw.split_once('/').ok_or_else(|| {
            CliError::Custom("Expected address/prefix, e.g. 2001:db8::1/64".into())
        })?;
        let ip = sim_packets::ipv6_from_str(addr)
            .ok_or_else(|| CliError::Custom("Invalid IPv6 address".into()))?;
        let prefix: u8 = len
            .parse()
            .map_err(|_| CliError::Custom("Invalid prefix length".into()))?;
        if prefix > 128 {
            return Err(CliError::Custom("Invalid prefix length".into()));
        }
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        c.eng.net.set_port_ipv6(c.dev, port, Some(ip), prefix);
        Ok(String::new())
    }

    /// IOS `ipv6 route 2001:db8:2::/64 2001:db8:1::2` / VRP `ipv6 route-static ...`
    pub fn ipv6_route(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let prefix = c.word("prefix");
        let nh_raw = c.word("nexthop");
        let (net_s, len): (&str, u8) = match prefix.split_once('/') {
            Some((n, l)) => (
                n,
                l.parse()
                    .map_err(|_| CliError::Custom("Invalid prefix length".into()))?,
            ),
            None => (
                prefix.as_str(),
                c.number("len")
                    .filter(|n| *n <= 128)
                    .ok_or_else(|| CliError::Custom("Invalid prefix length".into()))?
                    as u8,
            ),
        };
        let dst = sim_packets::ipv6_from_str(net_s)
            .ok_or_else(|| CliError::Custom("Invalid IPv6 prefix".into()))?;
        let nh = sim_packets::ipv6_from_str(&nh_raw)
            .ok_or_else(|| CliError::Custom("Invalid next hop".into()))?;
        c.eng.net.add_static_route6(c.dev, dst, len, nh);
        Ok(String::new())
    }

    pub fn ping_ipv6(c: &mut Ctx) -> Result<String, CliError> {
        let raw = c.word("target");
        let target = sim_packets::ipv6_from_str(&raw)
            .ok_or_else(|| CliError::Custom("Invalid IPv6 address".into()))?;
        c.eng.start_ping6(c.dev, target);
        Ok(String::new())
    }

    pub fn show_ipv6_brief(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::ipv6_int_brief(c.eng, c.dev, c.dialect))
    }

    pub fn show_ipv6_route(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::ipv6_route_table(c.eng, c.dev, c.dialect))
    }

    // ---------- 二层安全与 QoS ----------

    /// 全局开启 DHCP Snooping。
    pub fn dhcp_snooping_enable(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.dhcp_snooping = true;
            }
        }
        Ok(String::new())
    }

    /// 接口设为 DHCP Snooping 信任端口。
    pub fn dhcp_snooping_trust(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.dhcp_trust = true;
            }
        }
        Ok(String::new())
    }

    fn port_sec_upsert(
        c: &mut Ctx,
        f: impl FnOnce(&mut sim_core::device::PortSecurity),
    ) -> Result<String, CliError> {
        require_switch(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let Some(p) = d.port_mut(port) else {
            return Ok(String::new());
        };
        let cfg = p
            .port_security
            .get_or_insert(sim_core::device::PortSecurity {
                max: 1,
                violation: sim_core::device::PortSecViolation::Restrict,
                sticky: false,
            });
        f(cfg);
        Ok(String::new())
    }

    pub fn port_security_enable(c: &mut Ctx) -> Result<String, CliError> {
        port_sec_upsert(c, |_| {})
    }

    pub fn port_security_max(c: &mut Ctx) -> Result<String, CliError> {
        let n = c.number("max").unwrap_or(1);
        if !(1..=1024).contains(&n) {
            return Err(CliError::Custom("max-mac must be 1-1024".into()));
        }
        port_sec_upsert(c, |cfg| cfg.max = n as u16)
    }

    pub fn port_security_violation(c: &mut Ctx) -> Result<String, CliError> {
        let w = c.word("action").to_lowercase();
        let v = match w.as_str() {
            "shutdown" => sim_core::device::PortSecViolation::Shutdown,
            "restrict" => sim_core::device::PortSecViolation::Restrict,
            _ => {
                return Err(CliError::Custom(
                    "violation must be shutdown or restrict".into(),
                ))
            }
        };
        port_sec_upsert(c, |cfg| cfg.violation = v)
    }

    pub fn port_security_sticky(c: &mut Ctx) -> Result<String, CliError> {
        port_sec_upsert(c, |cfg| cfg.sticky = true)
    }

    /// 端口隔离：`port-isolate enable group N` / `switchport protected`。
    pub fn port_isolate_enable(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let group = c.number("group").unwrap_or(0) as u16;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.isolate_group = Some(group);
            }
        }
        Ok(String::new())
    }

    /// 思科 `switchport protected`：所有 protected 端口两两隔离。
    pub fn switchport_protected(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.isolate_group = Some(0);
            }
        }
        Ok(String::new())
    }

    fn mux_set(c: &mut Ctx, role: sim_core::device::MuxRole) -> Result<String, CliError> {
        require_switch(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.mux_role = Some(role);
            }
        }
        Ok(String::new())
    }

    pub fn mux_vlan_principal(c: &mut Ctx) -> Result<String, CliError> {
        mux_set(c, sim_core::device::MuxRole::Principal)
    }

    pub fn mux_vlan_separate(c: &mut Ctx) -> Result<String, CliError> {
        mux_set(c, sim_core::device::MuxRole::Separate)
    }

    pub fn mux_vlan_group(c: &mut Ctx) -> Result<String, CliError> {
        let group = c.number("group").unwrap_or(0) as u16;
        mux_set(c, sim_core::device::MuxRole::Group(group))
    }

    pub fn show_port_isolate(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::port_isolate(c.eng, c.dev, c.dialect))
    }

    pub fn show_mux_vlan(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::mux_vlan(c.eng, c.dev, c.dialect))
    }

    /// 广播风暴抑制（每秒广播帧上限）。
    pub fn storm_control(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let pps = c.number("pps").unwrap_or(0);
        if !(1..=1_000_000).contains(&pps) {
            return Err(CliError::Custom("Invalid broadcast limit".into()));
        }
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.broadcast_limit_pps = Some(pps);
            }
        }
        Ok(String::new())
    }

    /// 接口入口限速（kbps）。
    pub fn rate_limit(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let kbps = c.number("kbps").unwrap_or(0);
        if !(1..=1_000_000).contains(&kbps) {
            return Err(CliError::Custom("Invalid rate limit".into()));
        }
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.rate_limit_kbps = Some(kbps);
            }
        }
        Ok(String::new())
    }

    pub fn show_dhcp_snooping(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::dhcp_snooping_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_port_security(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::port_security_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_qos(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::qos_table(c.eng, c.dev, c.dialect))
    }

    // ---------- BGP（eBGP 简化） ----------

    pub fn bgp_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let asn = c.number("asn").unwrap_or(0);
        if asn == 0 {
            return Err(CliError::Custom("Invalid AS number".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                match &mut rs.bgp {
                    Some(b) => b.asn = asn,
                    None => {
                        rs.bgp = Some(sim_core::device::BgpConfig {
                            asn,
                            neighbors: vec![],
                            networks: vec![],
                            peer_policies: vec![],
                        })
                    }
                }
            }
        }
        c.sess.mode = Mode::Bgp;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    pub fn bgp_neighbor(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let ip = c.ipv4("ip");
        let asn = c.number("asn").unwrap_or(0);
        if ip == 0 || asn == 0 {
            return Err(CliError::Custom("Invalid neighbor".into()));
        }
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        let Some(bgp) = rs.bgp.as_mut() else {
            return Err(CliError::Custom("BGP process not configured".into()));
        };
        if let Some(n) = bgp.neighbors.iter_mut().find(|(n, _)| *n == ip) {
            n.1 = asn;
        } else {
            bgp.neighbors.push((ip, asn));
        }
        c.eng.bgp_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn bgp_network(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let net = c.ipv4("net");
        let len = c.mask_len("mask");
        if net == 0 {
            return Err(CliError::Custom("Invalid network".into()));
        }
        let net = net & sim_packets::mask_from_prefix(len);
        {
            let Some(d) = c.eng.net.device_mut(c.dev) else {
                return Ok(String::new());
            };
            let DeviceState::Router(rs) = &mut d.state else {
                return Ok(String::new());
            };
            let Some(bgp) = rs.bgp.as_mut() else {
                return Err(CliError::Custom("BGP process not configured".into()));
            };
            if !bgp.networks.contains(&(net, len)) {
                bgp.networks.push((net, len));
            }
        }
        c.eng.bgp_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn show_bgp_summary(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::bgp_summary(c.eng, c.dev, c.dialect))
    }

    pub fn show_bgp_table(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::bgp_table(c.eng, c.dev, c.dialect))
    }

    // ---------- route-policy（BGP 邻居策略） ----------

    /// IOS `route-map NAME permit|deny SEQ` / VRP `route-policy NAME permit|deny node SEQ`
    pub fn route_policy_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let name = c.word("name");
        if name.is_empty() {
            return Err(CliError::Custom("Route policy name is required".into()));
        }
        let action = c.word("action");
        let permit = if action.eq_ignore_ascii_case("permit") {
            true
        } else if action.eq_ignore_ascii_case("deny") {
            false
        } else {
            return Err(CliError::Custom("Action must be permit or deny".into()));
        };
        let seq = c.number("seq").unwrap_or(10);
        if seq == 0 || seq > u16::MAX as u32 {
            return Err(CliError::Custom("Node sequence must be 1-65535".into()));
        }
        let seq = seq as u16;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                let rp = match rs.route_policies.iter_mut().find(|p| p.name == name) {
                    Some(p) => p,
                    None => {
                        rs.route_policies.push(RoutePolicy {
                            name: name.clone(),
                            nodes: vec![],
                        });
                        rs.route_policies.last_mut().unwrap()
                    }
                };
                match rp.nodes.iter_mut().find(|n| n.seq == seq) {
                    Some(n) => n.permit = permit,
                    None => rp.nodes.push(RoutePolicyNode {
                        seq,
                        permit,
                        match_acl: None,
                        set_local_pref: None,
                    }),
                }
            }
        }
        c.sess.sub.rp = Some((name, seq));
        c.sess.mode = Mode::RoutePolicy;
        c.sess.cur_if = None;
        c.eng.bgp_policy_changed(c.dev);
        Ok(String::new())
    }

    fn rp_current_node(c: &Ctx) -> Result<(String, u16), CliError> {
        c.sess
            .sub
            .rp
            .clone()
            .ok_or_else(|| CliError::Custom("no route policy node selected".into()))
    }

    /// IOS `match ip address ACL` / VRP `if-match acl ACL`
    pub fn route_policy_match_acl(c: &mut Ctx) -> Result<String, CliError> {
        let (name, seq) = rp_current_node(c)?;
        let acl = c.number("acl").unwrap_or(0);
        if acl == 0 {
            return Err(CliError::Custom("ACL number is required".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if let Some(node) = rs
                    .route_policies
                    .iter_mut()
                    .find(|p| p.name == name)
                    .and_then(|p| p.nodes.iter_mut().find(|n| n.seq == seq))
                {
                    node.match_acl = Some(acl);
                }
            }
        }
        c.eng.bgp_policy_changed(c.dev);
        Ok(String::new())
    }

    /// IOS `set local-preference N` / VRP `apply local-preference N`
    pub fn route_policy_set_local_pref(c: &mut Ctx) -> Result<String, CliError> {
        let (name, seq) = rp_current_node(c)?;
        let lp = c
            .number("pref")
            .ok_or_else(|| CliError::Custom("Local preference is required".into()))?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if let Some(node) = rs
                    .route_policies
                    .iter_mut()
                    .find(|p| p.name == name)
                    .and_then(|p| p.nodes.iter_mut().find(|n| n.seq == seq))
                {
                    node.set_local_pref = Some(lp);
                }
            }
        }
        c.eng.bgp_policy_changed(c.dev);
        Ok(String::new())
    }

    /// IOS `neighbor IP route-map NAME in|out` / VRP `peer IP route-policy NAME import|export`
    pub fn bgp_peer_policy(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let ip = c.ipv4("ip");
        let name = c.word("policy");
        let dir = c.word("dir");
        if ip == 0 || name.is_empty() {
            return Err(CliError::Custom("Invalid peer policy".into()));
        }
        let import = if matches!(dir.to_ascii_lowercase().as_str(), "in" | "import") {
            true
        } else if matches!(dir.to_ascii_lowercase().as_str(), "out" | "export") {
            false
        } else {
            return Err(CliError::Custom("Direction must be in|out".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        let Some(bgp) = rs.bgp.as_mut() else {
            return Err(CliError::Custom("BGP process not configured".into()));
        };
        if !bgp.neighbors.iter().any(|(n, _)| *n == ip) {
            return Err(CliError::Custom("BGP neighbor not configured".into()));
        }
        bgp.peer_policies
            .retain(|p| !(p.neighbor == ip && p.import == import));
        bgp.peer_policies.push(BgpPeerPolicy {
            neighbor: ip,
            import,
            name,
        });
        c.eng.bgp_policy_changed(c.dev);
        Ok(String::new())
    }

    pub fn show_route_policy(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let name = c.word("name");
        let only = if name.is_empty() {
            None
        } else {
            Some(name.as_str())
        };
        Ok(show::route_policies(c.eng, c.dev, c.dialect, only))
    }

    // ---------- 防火墙：安全区域与区域间策略 ----------

    fn require_firewall(c: &Ctx) -> Result<(), CliError> {
        if c.device_kind() == DeviceKind::Firewall {
            Ok(())
        } else {
            Err(CliError::Custom(
                "Security zones are only available on firewalls".into(),
            ))
        }
    }

    fn zone_name(c: &Ctx) -> String {
        let n = c.word("zone");
        if n.is_empty() {
            c.word("name")
        } else {
            n
        }
    }

    pub fn security_zone_enter(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        let name = zone_name(c);
        if name.is_empty() {
            return Err(CliError::Custom("Zone name is required".into()));
        }
        c.sess.sub.zone = Some(name);
        c.sess.mode = Mode::Zone;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    pub fn security_zone_add_if(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        let Some(zone) = c.sess.sub.zone.clone() else {
            return Err(CliError::Custom("no zone selected".into()));
        };
        let raw = c.word("if");
        let Some(ifname::IfRef::Physical(port)) =
            ifname::resolve_if(c.dialect, c.device_kind(), &raw)
        else {
            return Err(CliError::Custom("Invalid interface".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.zone = Some(zone);
            }
        }
        Ok(String::new())
    }

    pub fn security_zone_member(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let zone = c.word("zone");
        if zone.is_empty() {
            return Err(CliError::Custom("Zone name is required".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.zone = Some(zone);
            }
        }
        Ok(String::new())
    }

    /// ASA：`nameif inside`
    pub fn nameif(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let name = c.word("name");
        if name.is_empty() {
            return Err(CliError::Custom("Interface name is required".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.nameif = Some(name);
            }
        }
        Ok(String::new())
    }

    /// ASA：`security-level 0-100`
    pub fn security_level(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let level = c
            .number("level")
            .ok_or_else(|| CliError::Custom("Security level is required".into()))?;
        if level > 100 {
            return Err(CliError::Custom("Security level must be 0-100".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.security_level = Some(level as u8);
            }
        }
        Ok(String::new())
    }

    pub fn security_policy_enter(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        c.sess.mode = Mode::SecurityPolicy;
        c.sess.sub.rule = None;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    pub fn security_rule_enter(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        let name = c.word("name");
        if name.is_empty() {
            return Err(CliError::Custom("Rule name is required".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if !rs.security_policies.iter().any(|p| p.name == name) {
                    rs.security_policies.push(SecurityPolicy {
                        name: name.clone(),
                        src_zone: String::new(),
                        dst_zone: String::new(),
                        permit: false,
                    });
                }
            }
        }
        c.sess.sub.rule = Some(name);
        c.sess.mode = Mode::SecurityRule;
        Ok(String::new())
    }

    fn security_rule_mut(c: &mut Ctx, f: impl FnOnce(&mut SecurityPolicy)) -> Result<(), CliError> {
        let Some(name) = c.sess.sub.rule.clone() else {
            return Err(CliError::Custom("no security rule selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(());
        };
        if let Some(rule) = rs.security_policies.iter_mut().find(|p| p.name == name) {
            f(rule);
        }
        Ok(())
    }

    pub fn security_rule_source(c: &mut Ctx) -> Result<String, CliError> {
        let zone = c.word("src");
        if zone.is_empty() {
            return Err(CliError::Custom("Source zone is required".into()));
        }
        security_rule_mut(c, |r| r.src_zone = zone)?;
        Ok(String::new())
    }

    pub fn security_rule_destination(c: &mut Ctx) -> Result<String, CliError> {
        let zone = c.word("dst");
        if zone.is_empty() {
            return Err(CliError::Custom("Destination zone is required".into()));
        }
        security_rule_mut(c, |r| r.dst_zone = zone)?;
        Ok(String::new())
    }

    pub fn security_action(c: &mut Ctx) -> Result<String, CliError> {
        let action = c.word("action");
        let permit = if action.eq_ignore_ascii_case("permit") {
            true
        } else if action.eq_ignore_ascii_case("deny") {
            false
        } else {
            return Err(CliError::Custom("Action must be permit or deny".into()));
        };
        security_rule_mut(c, |r| r.permit = permit)?;
        Ok(String::new())
    }

    pub fn show_security_policy(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        Ok(show::security_policies(c.eng, c.dev, c.dialect))
    }

    pub fn show_security_zone(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        Ok(show::security_zones(c.eng, c.dev, c.dialect))
    }

    // ---------- IPsec 站点隧道（教学 IP-in-IP） ----------

    fn ipsec_tunnel_mut(c: &mut Ctx, f: impl FnOnce(&mut IpsecTunnel)) -> Result<(), CliError> {
        let Some(name) = c.sess.sub.ipsec.clone() else {
            return Err(CliError::Custom("no IPsec tunnel selected".into()));
        };
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(());
        };
        if let Some(t) = rs.ipsec_tunnels.iter_mut().find(|t| t.name == name) {
            f(t);
        }
        Ok(())
    }

    /// `ipsec tunnel NAME local A remote B`
    pub fn ipsec_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let name = c.word("name");
        let local = c.ipv4("local");
        let remote = c.ipv4("remote");
        if name.is_empty() || local == 0 || remote == 0 {
            return Err(CliError::Custom("Invalid IPsec tunnel".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if !rs.ipsec_tunnels.iter().any(|t| t.name == name) {
                    rs.ipsec_tunnels.push(IpsecTunnel {
                        name: name.clone(),
                        local,
                        remote,
                        local_subnet: (0, 0),
                        remote_subnet: (0, 0),
                    });
                }
            }
        }
        c.sess.sub.ipsec = Some(name);
        c.sess.mode = Mode::Ipsec;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    pub fn ipsec_local_subnet(c: &mut Ctx) -> Result<String, CliError> {
        let net = c.ipv4("net");
        let len = c.mask_len("len");
        let net = net & sim_packets::mask_from_prefix(len);
        ipsec_tunnel_mut(c, |t| t.local_subnet = (net, len))?;
        Ok(String::new())
    }

    pub fn ipsec_remote_subnet(c: &mut Ctx) -> Result<String, CliError> {
        let net = c.ipv4("net");
        let len = c.mask_len("len");
        let net = net & sim_packets::mask_from_prefix(len);
        ipsec_tunnel_mut(c, |t| t.remote_subnet = (net, len))?;
        Ok(String::new())
    }

    pub fn show_ipsec(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::ipsec_tunnels(c.eng, c.dev, c.dialect))
    }

    // ---------- SSL VPN（远程接入） ----------

    pub fn sslvpn_pool(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        let net = c.ipv4("net");
        let len = c.mask_len("len");
        let net = net & sim_packets::mask_from_prefix(len);
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.sslvpn_pool = Some((net, len));
            }
        }
        Ok(String::new())
    }

    pub fn show_sslvpn(c: &mut Ctx) -> Result<String, CliError> {
        require_firewall(c)?;
        Ok(show::sslvpn_sessions(c.eng, c.dev, c.dialect))
    }

    // ---------- WLAN / CAPWAP ----------

    fn require_ac(c: &Ctx) -> Result<(), CliError> {
        if c.device_kind() == DeviceKind::WlanController {
            Ok(())
        } else {
            Err(CliError::Custom(
                "WLAN configuration is only available on the AC".into(),
            ))
        }
    }

    /// AP：`capwap ac <ip>`
    pub fn capwap_ac(c: &mut Ctx) -> Result<String, CliError> {
        if c.device_kind() != DeviceKind::AccessPoint {
            return Err(CliError::Custom("Only an AP can join an AC".into()));
        }
        let ip = c.ipv4("ip");
        if ip == 0 {
            return Err(CliError::Custom("Invalid AC address".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.wlan.capwap_ac = Some(ip);
            d.wlan.joined = false;
        }
        c.eng.capwap_start(c.dev);
        Ok(String::new())
    }

    pub fn wlan_ssid(c: &mut Ctx) -> Result<String, CliError> {
        require_ac(c)?;
        let ssid = c.word("ssid");
        if ssid.is_empty() {
            return Err(CliError::Custom("SSID is required".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if !d.wlan.profiles.iter().any(|p| p.ssid == ssid) {
                d.wlan.profiles.push(WlanProfile {
                    ssid: ssid.clone(),
                    security: WlanSecurity::Open,
                    psk: None,
                    vlan: 1,
                });
            }
            d.wlan.selected = Some(ssid);
        }
        c.eng.wlan_config_changed(c.dev);
        Ok(String::new())
    }

    fn wlan_update_profile(c: &mut Ctx, f: impl FnOnce(&mut WlanProfile)) -> Result<(), CliError> {
        let Some(ssid) = c
            .eng
            .net
            .device(c.dev)
            .and_then(|d| d.wlan.selected.clone())
        else {
            return Err(CliError::Custom("no SSID selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.wlan.profile_mut(&ssid) {
                f(p);
            }
        }
        c.eng.wlan_config_changed(c.dev);
        Ok(())
    }

    pub fn wlan_security_open(c: &mut Ctx) -> Result<String, CliError> {
        require_ac(c)?;
        wlan_update_profile(c, |p| {
            p.security = WlanSecurity::Open;
            p.psk = None;
        })?;
        Ok(String::new())
    }

    pub fn wlan_security_wpa2(c: &mut Ctx) -> Result<String, CliError> {
        require_ac(c)?;
        let psk = c.word("psk");
        if psk.is_empty() {
            return Err(CliError::Custom("WPA2 PSK is required".into()));
        }
        wlan_update_profile(c, |p| {
            p.security = WlanSecurity::Wpa2Psk;
            p.psk = Some(psk);
        })?;
        Ok(String::new())
    }

    pub fn wlan_security_wpa3(c: &mut Ctx) -> Result<String, CliError> {
        require_ac(c)?;
        let psk = c.word("psk");
        if psk.is_empty() {
            return Err(CliError::Custom("WPA3 passphrase is required".into()));
        }
        wlan_update_profile(c, |p| {
            p.security = WlanSecurity::Wpa3Sae;
            p.psk = Some(psk);
        })?;
        Ok(String::new())
    }

    pub fn wlan_vlan(c: &mut Ctx) -> Result<String, CliError> {
        require_ac(c)?;
        let vlan = c.number("vlan").unwrap_or(0);
        if !(1..=4094).contains(&vlan) {
            return Err(CliError::Custom("Invalid VLAN".into()));
        }
        wlan_update_profile(c, |p| p.vlan = vlan as u16)?;
        Ok(String::new())
    }

    /// AP：`wlan priority <n>`（漫游选路）
    pub fn wlan_priority(c: &mut Ctx) -> Result<String, CliError> {
        if c.device_kind() != DeviceKind::AccessPoint {
            return Err(CliError::Custom(
                "WLAN priority is only available on APs".into(),
            ));
        }
        let priority = c.number("priority").unwrap_or(100).min(255) as u8;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            d.wlan.priority = priority;
        }
        Ok(String::new())
    }

    pub fn show_wlan(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::wlan_table(c.eng, c.dev, c.dialect))
    }

    // ---------- VXLAN（EVPN 数据面简化） ----------

    pub fn vxlan_local(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let ip = c.ipv4("ip");
        if ip == 0 {
            return Err(CliError::Custom("Invalid VTEP address".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.vxlan.local = Some(ip);
            }
        }
        c.eng.evpn_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn vxlan_peer(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let ip = c.ipv4("ip");
        if ip == 0 {
            return Err(CliError::Custom("Invalid VTEP peer".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                if !sw.vxlan.peers.contains(&ip) {
                    sw.vxlan.peers.push(ip);
                }
            }
        }
        Ok(String::new())
    }

    pub fn vxlan_vni(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let vni = c.number("vni").unwrap_or(0);
        let vlan = c.number("vlan").unwrap_or(0);
        if vni == 0 || !(1..=4094).contains(&vlan) {
            return Err(CliError::Custom("Invalid VNI mapping".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.ensure_vlan(vlan as u16);
                sw.vxlan.mappings.retain(|(_, v)| *v != vlan as u16);
                sw.vxlan.mappings.push((vni, vlan as u16));
            }
        }
        c.eng.evpn_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn show_vxlan(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::vxlan_table(c.eng, c.dev))
    }

    // ---------- EVPN 控制面（简化 Type-3） ----------

    pub fn evpn_enable(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                sw.evpn.enabled = true;
            }
        }
        c.eng.evpn_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn evpn_peer(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let ip = c.ipv4("ip");
        if ip == 0 {
            return Err(CliError::Custom("Invalid EVPN peer".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                if !sw.evpn.peers.contains(&ip) {
                    sw.evpn.peers.push(ip);
                }
            }
        }
        c.eng.evpn_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn show_evpn(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::evpn_table(c.eng, c.dev))
    }

    // ---------- F5 LTM（虚拟服务器 / 池） ----------

    fn require_lb(c: &Ctx) -> Result<(), CliError> {
        if c.device_kind() == DeviceKind::LoadBalancer {
            Ok(())
        } else {
            Err(CliError::Custom(
                "LTM configuration is only available on a load balancer".into(),
            ))
        }
    }

    pub fn ltm_pool_member(c: &mut Ctx) -> Result<String, CliError> {
        require_lb(c)?;
        let pool = c.word("pool");
        let ip = c.ipv4("ip");
        let port = c.number("port").unwrap_or(0);
        if pool.is_empty() || ip == 0 || port == 0 || port > u16::MAX as u32 {
            return Err(CliError::Custom("Invalid pool member".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            let p = match d.ltm.pools.iter_mut().find(|p| p.name == pool) {
                Some(p) => p,
                None => {
                    d.ltm.pools.push(LtmPool {
                        name: pool.clone(),
                        members: vec![],
                    });
                    d.ltm.pools.last_mut().unwrap()
                }
            };
            let m = LtmMember {
                ip,
                port: port as u16,
            };
            if !p.members.contains(&m) {
                p.members.push(m);
            }
        }
        Ok(String::new())
    }

    pub fn ltm_virtual(c: &mut Ctx) -> Result<String, CliError> {
        require_lb(c)?;
        let name = c.word("name");
        let vip = c.ipv4("vip");
        let port = c.number("port").unwrap_or(0);
        let pool = c.word("pool");
        if name.is_empty() || vip == 0 || port == 0 || port > u16::MAX as u32 || pool.is_empty() {
            return Err(CliError::Custom("Invalid virtual server".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if !d.ltm.pools.iter().any(|p| p.name == pool) {
                return Err(CliError::Custom("Pool not found".into()));
            }
            let v = LtmVirtual {
                name,
                vip,
                port: port as u16,
                pool,
            };
            if let Some(x) = d.ltm.virtuals.iter_mut().find(|x| x.name == v.name) {
                *x = v;
            } else {
                d.ltm.virtuals.push(v);
            }
        }
        Ok(String::new())
    }

    pub fn show_ltm(c: &mut Ctx) -> Result<String, CliError> {
        require_lb(c)?;
        Ok(show::ltm_table(c.eng, c.dev))
    }

    // ---------- 堆叠 / IRF / M-LAG ----------

    pub fn stack_domain(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let domain = c.number("domain").unwrap_or(0);
        if domain == 0 || domain > u16::MAX as u32 {
            return Err(CliError::Custom("Invalid stack domain".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                let s = sw.stack.get_or_insert(StackConfig {
                    domain: domain as u16,
                    member: 1,
                    priority: 100,
                });
                s.domain = domain as u16;
            }
        }
        Ok(String::new())
    }

    pub fn stack_member(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let member = c.number("member").unwrap_or(0);
        if member == 0 || member > 255 {
            return Err(CliError::Custom("Invalid stack member ID".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                if let Some(s) = sw.stack.as_mut() {
                    s.member = member as u8;
                } else {
                    return Err(CliError::Custom("Configure stack domain first".into()));
                }
            }
        }
        Ok(String::new())
    }

    pub fn stack_priority(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let priority = c.number("priority").unwrap_or(100).min(u16::MAX as u32) as u16;
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                if let Some(s) = sw.stack.as_mut() {
                    s.priority = priority;
                } else {
                    return Err(CliError::Custom("Configure stack domain first".into()));
                }
            }
        }
        Ok(String::new())
    }

    pub fn mlag_domain(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let domain = c.number("domain").unwrap_or(0);
        if domain == 0 || domain > u16::MAX as u32 {
            return Err(CliError::Custom("Invalid M-LAG domain".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                let m = sw.mlag.get_or_insert(MlagConfig {
                    domain: domain as u16,
                    peer_links: vec![],
                });
                m.domain = domain as u16;
            }
        }
        Ok(String::new())
    }

    pub fn mlag_peer_link(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Switch(sw) = &mut d.state {
                if let Some(m) = sw.mlag.as_mut() {
                    if !m.peer_links.contains(&port) {
                        m.peer_links.push(port);
                    }
                } else {
                    return Err(CliError::Custom("Configure mlag domain first".into()));
                }
            }
        }
        Ok(String::new())
    }

    pub fn show_stack(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::stack_table(c.eng, c.dev))
    }

    pub fn show_mlag(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::mlag_table(c.eng, c.dev))
    }

    // ---------- IS-IS（简化） ----------

    pub fn isis_enter(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        c.sess.mode = Mode::Isis;
        c.sess.cur_if = None;
        Ok(String::new())
    }

    pub fn isis_net(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let net = c.word("net");
        let Some(sid) = sim_core::device::parse_isis_net(&net) else {
            return Err(CliError::Custom(
                "Invalid NET, e.g. 49.0001.0000.0000.0001.00".into(),
            ));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                rs.isis = Some(sim_core::device::IsisConfig {
                    net: net.clone(),
                    system_id: sid,
                });
            }
        }
        c.eng.isis_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn isis_enable_if(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let Some(p) = d.port_mut(port) {
                p.isis_enable = true;
            }
        }
        c.eng.isis_config_changed(c.dev);
        Ok(String::new())
    }

    pub fn show_isis_neighbors(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::isis_neighbors(c.eng, c.dev, c.dialect))
    }

    pub fn show_isis_db(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::isis_lsdb(c.eng, c.dev, c.dialect))
    }

    // ---------- VRF（简化：连接 + 静态路由） ----------

    pub fn vrf_create(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let name = c.word("name");
        if name.is_empty() {
            return Err(CliError::Custom("VRF name required".into()));
        }
        if let Some(d) = c.eng.net.device_mut(c.dev) {
            if let DeviceState::Router(rs) = &mut d.state {
                if !rs.vrfs.iter().any(|v| v.name == name) {
                    let id = rs.vrfs.iter().map(|v| v.id).max().unwrap_or(0) + 1;
                    rs.vrfs.push(sim_core::device::Vrf {
                        id,
                        name,
                        static_routes: vec![],
                    });
                }
            }
        }
        Ok(String::new())
    }

    pub fn vrf_bind(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let Some(cli_engine::IfCtx::Physical(port)) = c.sess.cur_if else {
            return Err(CliError::Custom("no interface selected".into()));
        };
        let name = c.word("name");
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let id = match &d.state {
            DeviceState::Router(rs) => rs.vrfs.iter().find(|v| v.name == name).map(|v| v.id),
            _ => None,
        };
        let Some(id) = id else {
            return Err(CliError::Custom("VRF not found".into()));
        };
        if let Some(p) = d.port_mut(port) {
            p.vrf = Some(id);
        }
        Ok(String::new())
    }

    pub fn vrf_static(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let name = c.word("name");
        let dst = c.ipv4("dst");
        let len = c.mask_len("mask");
        let nh = c.ipv4("nexthop");
        if nh == 0 {
            return Err(CliError::Custom("Invalid route".into()));
        }
        let net = dst & sim_packets::mask_from_prefix(len);
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        let Some(v) = rs.vrfs.iter_mut().find(|v| v.name == name) else {
            return Err(CliError::Custom("VRF not found".into()));
        };
        if !v
            .static_routes
            .iter()
            .any(|r| r.dst == net && r.len == len && r.nexthop == nh)
        {
            v.static_routes.push(sim_core::device::Route {
                dst: net,
                len,
                nexthop: nh,
                vlan: None,
                port: 0,
                src: sim_core::device::RouteSrc::Static,
            });
        }
        Ok(String::new())
    }

    pub fn show_route_vrf(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        let name = c.word("name");
        let id = c
            .eng
            .net
            .device(c.dev)
            .and_then(|d| d.vrf_id(&name))
            .ok_or_else(|| CliError::Custom("VRF not found".into()))?;
        Ok(show::route_table_vrf(c.eng, c.dev, c.dialect, id))
    }

    // ---------- MPLS（静态 LSP） ----------

    fn mpls_lsp_upsert(c: &mut Ctx, lsp: sim_core::device::StaticLsp) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        if lsp.vpn_label.is_some() && lsp.vrf.is_none() {
            return Err(CliError::Custom("vpn-label requires a vrf".into()));
        }
        let Some(d) = c.eng.net.device_mut(c.dev) else {
            return Ok(String::new());
        };
        if let Some(v) = &lsp.vrf {
            if d.vrf_id(v).is_none() {
                return Err(CliError::Custom("VRF not found".into()));
            }
        }
        let DeviceState::Router(rs) = &mut d.state else {
            return Ok(String::new());
        };
        if let Some(x) = rs.mpls_lsps.iter_mut().find(|x| x.name == lsp.name) {
            *x = lsp;
        } else {
            rs.mpls_lsps.push(lsp);
        }
        Ok(String::new())
    }

    pub fn mpls_lsp_ingress(c: &mut Ctx) -> Result<String, CliError> {
        let name = c.word("name");
        let net = c.ipv4("net") & sim_packets::mask_from_prefix(c.mask_len("len"));
        let len = c.mask_len("len");
        let nh = c.ipv4("nexthop");
        let out = c.number("out");
        let vrf = c.word("vrf");
        if name.is_empty() || net == 0 || nh == 0 || out.is_none() {
            return Err(CliError::Custom("Invalid static LSP".into()));
        }
        mpls_lsp_upsert(
            c,
            sim_core::device::StaticLsp {
                name,
                net: Some((net, len)),
                in_label: None,
                out_label: out,
                nexthop: Some(nh),
                vrf: if vrf.is_empty() { None } else { Some(vrf) },
                vpn_label: c.number("vpn"),
            },
        )
    }

    pub fn mpls_lsp_transit(c: &mut Ctx) -> Result<String, CliError> {
        let name = c.word("name");
        let inside = c.number("in");
        let nh = c.ipv4("nexthop");
        let out = c.number("out");
        if name.is_empty() || inside.is_none() || nh == 0 || out.is_none() {
            return Err(CliError::Custom("Invalid static LSP".into()));
        }
        mpls_lsp_upsert(
            c,
            sim_core::device::StaticLsp {
                name,
                net: None,
                in_label: inside,
                out_label: out,
                nexthop: Some(nh),
                vrf: None,
                vpn_label: None,
            },
        )
    }

    pub fn mpls_lsp_egress(c: &mut Ctx) -> Result<String, CliError> {
        let name = c.word("name");
        let inside = c.number("in");
        let vrf = c.word("vrf");
        if name.is_empty() || inside.is_none() {
            return Err(CliError::Custom("Invalid static LSP".into()));
        }
        mpls_lsp_upsert(
            c,
            sim_core::device::StaticLsp {
                name,
                net: None,
                in_label: inside,
                out_label: None,
                nexthop: None,
                vrf: if vrf.is_empty() { None } else { Some(vrf) },
                vpn_label: None,
            },
        )
    }

    pub fn show_mpls(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(
                "Command is only available on routers".into(),
            ));
        }
        Ok(show::mpls_table(c.eng, c.dev, c.dialect))
    }

    // ---------- 显示 ----------

    pub fn show_acl(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on routers".into(),
            }));
        }
        Ok(show::acl_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_nat(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router
                | DeviceKind::Firewall
                | DeviceKind::WlanController
                | DeviceKind::LoadBalancer
        ) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on routers".into(),
            }));
        }
        Ok(show::nat_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_brief(c: &mut Ctx) -> Result<String, CliError> {
        Ok(show::int_brief(c.eng, c.dev, c.dialect))
    }

    pub fn show_route(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router | DeviceKind::Switch | DeviceKind::Firewall
        ) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on routers".into(),
            }));
        }
        Ok(show::route_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_stp(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::stp_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_vlan(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::vlan_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_mac(c: &mut Ctx) -> Result<String, CliError> {
        require_switch(c)?;
        Ok(show::mac_table(c.eng, c.dev, c.dialect))
    }

    pub fn static_route(c: &mut Ctx) -> Result<String, CliError> {
        if !matches!(
            c.device_kind(),
            DeviceKind::Router | DeviceKind::Switch | DeviceKind::Firewall
        ) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on routers".into(),
            }));
        }
        let dst = c.ipv4("dst");
        let len = c.mask_len("mask");
        let nh = c.ipv4("nexthop");
        if nh == 0 {
            return Err(CliError::Custom("Invalid route parameters".into()));
        }
        let net_dst = dst & mask_from_prefix(len);
        c.eng.net.add_static_route(c.dev, net_dst, len, nh);
        Ok(String::new())
    }

    pub fn ping(c: &mut Ctx) -> Result<String, CliError> {
        let target = c.ipv4("target");
        if target == 0 {
            return Err(CliError::Custom("Invalid IP address".into()));
        }
        c.eng.start_ping(c.dev, target);
        Ok(String::new())
    }

    pub fn save_msg(_c: &mut Ctx) -> Result<String, CliError> {
        Ok(
            "Info: The configuration takes effect immediately in the simulator; no need to save."
                .into(),
        )
    }
}

// ---------- 执行入口 ----------

/// 执行一行命令（不驱动引擎）。空行 / 帮助 / 错误均已格式化。
pub fn exec_line(eng: &mut Engine, sess: &mut Session, dev: DevId, line: &str) -> String {
    // 登录流程优先：后续输入是用户名/口令
    if sess.login.is_some() {
        return login_step(eng, sess, dev, line);
    }
    // 远程会话：命令转发给目标设备，exit/logout 断开
    if sess.remote.is_some() {
        return remote_step(eng, sess, dev, line);
    }
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    if d.kind == DeviceKind::Host {
        return pc::exec(eng, sess, dev, line);
    }
    let dialect = d.dialect;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains('?') {
        return render_help(eng.net.device(dev).unwrap().dialect, sess, line);
    }
    let t = trees(dialect);
    let root = t.root_for(sess.mode);
    match cli_engine::parse(root, line) {
        Ok(Some(parsed)) => {
            let mut ctx = Ctx {
                eng,
                dev,
                sess,
                dialect,
                vals: parsed.vals,
            };
            match (parsed.run)(&mut ctx) {
                Ok(out) => out,
                Err(e) => format_error(dialect, line, e),
            }
        }
        Ok(None) => String::new(),
        Err(e) => format_error(dialect, line, e),
    }
}

/// 登录失败提示（按源设备的方言/主机风格渲染）。
pub fn login_error_text(
    kind: DeviceKind,
    dialect: Dialect,
    e: sim_core::engine::LoginError,
) -> String {
    use sim_core::engine::LoginError;
    if kind == DeviceKind::Host {
        return match e {
            LoginError::Unreachable => "connect: Network is unreachable".into(),
            LoginError::TransportDenied => "connect: Connection refused".into(),
            LoginError::AuthFailed => "Permission denied".into(),
        };
    }
    match dialect {
        Dialect::Ios => match e {
            LoginError::Unreachable => "% Destination unreachable".into(),
            LoginError::TransportDenied => "% Connection refused by remote host".into(),
            LoginError::AuthFailed => "% Authentication failed".into(),
        },
        Dialect::Vrp => match e {
            LoginError::Unreachable => "Error: Destination host unreachable.".into(),
            LoginError::TransportDenied => "Error: The connection was refused.".into(),
            LoginError::AuthFailed => "Error: Authentication failed.".into(),
        },
        Dialect::Comware => match e {
            LoginError::Unreachable => "% Destination unreachable.".into(),
            LoginError::TransportDenied => "% Connection refused.".into(),
            LoginError::AuthFailed => "% Authentication failed.".into(),
        },
    }
}

/// 登录流程：用户名 → 口令 → 建立远程会话。
fn login_step(eng: &mut Engine, sess: &mut Session, dev: DevId, line: &str) -> String {
    let (kind, dialect) = eng
        .net
        .device(dev)
        .map(|d| (d.kind, d.dialect))
        .unwrap_or((DeviceKind::Router, Dialect::Ios));
    let Some(step) = sess.login.as_ref().map(|f| f.step) else {
        return String::new();
    };
    match step {
        LoginStep::Username => {
            let user = line.trim().to_string();
            if user.is_empty() {
                return String::new();
            }
            if let Some(flow) = sess.login.as_mut() {
                flow.user = user;
                flow.step = LoginStep::Password;
            }
            String::new()
        }
        LoginStep::Password => {
            let Some(flow) = sess.login.take() else {
                return String::new();
            };
            match eng.try_login(dev, flow.target_ip, flow.proto, &flow.user, line.trim()) {
                Ok((target, privilege)) => {
                    let target_dialect = eng
                        .net
                        .device(target)
                        .map(|d| d.dialect)
                        .unwrap_or(Dialect::Ios);
                    // 权限 15 直接进特权模式（与 SSH 登录真实行为一致）
                    let mode = if target_dialect == Dialect::Ios && privilege >= 15 {
                        Mode::Priv
                    } else {
                        Mode::Exec
                    };
                    sess.remote = Some(RemoteSession {
                        dev: target,
                        inner: Box::new(Session {
                            mode,
                            ..Default::default()
                        }),
                    });
                    String::new()
                }
                Err(e) => login_error_text(kind, dialect, e),
            }
        }
    }
}

/// 远程会话：转发命令；`exit` / `logout` 断开。
fn remote_step(eng: &mut Engine, sess: &mut Session, dev: DevId, line: &str) -> String {
    let dialect = eng
        .net
        .device(dev)
        .map(|d| d.dialect)
        .unwrap_or(Dialect::Ios);
    let cmd = line.trim().to_lowercase();
    if cmd == "exit" || cmd == "logout" {
        sess.remote = None;
        return match dialect {
            Dialect::Ios => "\n[Connection closed by foreign host]".into(),
            _ => "\nInfo: The connection was closed.".into(),
        };
    }
    let Some(remote) = sess.remote.as_mut() else {
        return String::new();
    };
    let target = remote.dev;
    exec_line(eng, &mut remote.inner, target, line)
}

/// 执行一行命令并驱动引擎至静默；返回完整输出（含 ping 结果）。
pub fn exec_and_run(eng: &mut Engine, sess: &mut Session, dev: DevId, line: &str) -> String {
    let eff = sess.effective_dev(dev);
    let kind = eng.net.device(eff).map(|d| d.kind);
    let out = exec_line(eng, sess, dev, line);
    let horizon = eng.drain_horizon();
    eng.run_to_quiescence(horizon);
    let mut parts: Vec<String> = vec![];
    if !out.trim().is_empty() {
        parts.push(out);
    }
    let dialect = eng
        .net
        .device(eff)
        .map(|d| d.dialect)
        .unwrap_or(Dialect::Ios);
    for s in eng.take_unreported_pings(eff) {
        let text = if kind == Some(DeviceKind::Host) {
            show::render_ping_generic(&s)
        } else {
            show::render_ping(dialect, &s)
        };
        parts.push(text);
    }
    for s in eng.take_unreported_ping6(eff) {
        let text = if kind == Some(DeviceKind::Host) {
            show::render_ping6_generic(&s)
        } else {
            show::render_ping6(dialect, &s)
        };
        parts.push(text);
    }
    for p in eng.take_unreported_tcp(eff) {
        parts.push(show::render_tcp_probe(&p));
    }
    parts.join("\n")
}

pub fn prompt(eng: &Engine, sess: &Session, dev: DevId) -> String {
    if let Some(flow) = &sess.login {
        return match flow.step {
            LoginStep::Username => "Username: ".into(),
            LoginStep::Password => "Password: ".into(),
        };
    }
    if let Some(remote) = &sess.remote {
        return prompt(eng, &remote.inner, remote.dev);
    }
    let Some(d) = eng.net.device(dev) else {
        return "?".into();
    };
    if d.kind == DeviceKind::Host {
        return format!("{}>", d.name);
    }
    let dialect = d.dialect;
    match dialect {
        Dialect::Ios => match sess.mode {
            Mode::Exec => format!("{}>", d.name),
            Mode::Priv => format!("{}#", d.name),
            Mode::Config => format!("{}(config)#", d.name),
            Mode::If => format!("{}(config-if)#", d.name),
            Mode::Pool => format!("{}(dhcp-config)#", d.name),
            Mode::Acl => format!("{}(config-acl)#", d.name),
            Mode::Line => format!("{}(config-line)#", d.name),
            Mode::Aaa => format!("{}(config-aaa)#", d.name),
            Mode::Mst => format!("{}(config-mst)#", d.name),
            Mode::Ospf | Mode::OspfArea | Mode::Bgp | Mode::Isis | Mode::Ospfv3 => {
                format!("{}(config-router)#", d.name)
            }
            Mode::RoutePolicy => format!("{}(config-route-map)#", d.name),
            Mode::Zone => format!("{}(config-zone)#", d.name),
            Mode::SecurityPolicy => format!("{}(config-sec-policy)#", d.name),
            Mode::SecurityRule => format!("{}(config-zone-pair)#", d.name),
            Mode::Ipsec => format!("{}(config-ipsec)#", d.name),
        },
        Dialect::Vrp | Dialect::Comware => match sess.mode {
            Mode::Exec | Mode::Priv => format!("<{}>", d.name),
            Mode::Config => format!("[{}]", d.name),
            Mode::If => {
                let ifn = match sess.cur_if {
                    Some(cli_engine::IfCtx::Svi(v)) => ifname::svi_canonical(dialect, v),
                    Some(cli_engine::IfCtx::Physical(p)) => ifname::canonical(dialect, d.kind, p),
                    Some(cli_engine::IfCtx::Lag(id)) => ifname::lag_canonical(dialect, id),
                    None => String::new(),
                };
                format!("[{}-{}]", d.name, ifn)
            }
            Mode::Pool => {
                let pool = sess.sub.pool.clone().unwrap_or_default();
                format!("[{}-ip-pool-{}]", d.name, pool)
            }
            Mode::Acl => match sess.sub.acl {
                Some((n, true)) => format!("[{}-acl-adv-{}]", d.name, n),
                Some((n, false)) => format!("[{}-acl-basic-{}]", d.name, n),
                None => format!("[{}]", d.name),
            },
            Mode::Line => {
                let prefix = if dialect == Dialect::Comware {
                    "line-vty"
                } else {
                    "ui-vty"
                };
                format!("[{}-{}0-4]", d.name, prefix)
            }
            Mode::Aaa => match (dialect, &sess.sub.aaa_user) {
                (Dialect::Comware, Some(u)) => format!("[{}-luser-manage-{}]", d.name, u),
                _ => format!("[{}-aaa]", d.name),
            },
            Mode::Mst => format!("[{}-mst-region]", d.name),
            Mode::Ospf => format!("[{}-ospf-1]", d.name),
            Mode::OspfArea => format!("[{}-ospf-1-area-0.0.0.0]", d.name),
            Mode::Bgp => {
                let asn = eng
                    .net
                    .device(dev)
                    .and_then(|d| match &d.state {
                        DeviceState::Router(rs) => rs.bgp.as_ref().map(|b| b.asn),
                        _ => None,
                    })
                    .unwrap_or(0);
                format!("[{}-bgp-{}]", d.name, asn)
            }
            Mode::Isis => format!("[{}-isis-1]", d.name),
            Mode::RoutePolicy => format!("[{}-route-policy]", d.name),
            Mode::Zone => format!(
                "[{}-zone-{}]",
                d.name,
                sess.sub.zone.clone().unwrap_or_default()
            ),
            Mode::SecurityPolicy => format!("[{}-security-policy]", d.name),
            Mode::SecurityRule => format!(
                "[{}-security-policy-rule-{}]",
                d.name,
                sess.sub.rule.clone().unwrap_or_default()
            ),
            Mode::Ipsec => format!(
                "[{}-ipsec-{}]",
                d.name,
                sess.sub.ipsec.clone().unwrap_or_default()
            ),
            Mode::Ospfv3 => format!("[{}-ospfv3-1]", d.name),
        },
    }
}

/// Tab 补全（对主机无效）。
pub fn complete_line(
    eng: &Engine,
    sess: &Session,
    dev: DevId,
    line: &str,
) -> (String, Vec<String>) {
    if sess.login.is_some() {
        return (line.to_string(), vec![]);
    }
    if let Some(remote) = &sess.remote {
        return complete_line(eng, &remote.inner, remote.dev, line);
    }
    let Some(d) = eng.net.device(dev) else {
        return (line.to_string(), vec![]);
    };
    if d.kind == DeviceKind::Host {
        return (line.to_string(), vec![]);
    }
    let t = trees(d.dialect);
    cli_engine::complete(t.root_for(sess.mode), line)
}

fn render_help(dialect: Dialect, sess: &Session, line: &str) -> String {
    let t = trees(dialect);
    let Some(h) = cli_engine::help_for(t.root_for(sess.mode), line, dialect) else {
        return String::new();
    };
    format_help(dialect, &h)
}

fn format_help(_dialect: Dialect, h: &HelpList) -> String {
    let mut out = String::new();
    for (w, help) in &h.items {
        out.push_str(&format!("  {:<20}{}\n", w, help));
    }
    if h.cr {
        out.push_str("  <cr>\n");
    }
    out.trim_end().to_string()
}

pub fn format_error(dialect: Dialect, line: &str, e: CliError) -> String {
    match dialect {
        Dialect::Ios => match e {
            CliError::Custom(msg) => format!("% {}", msg),
            CliError::Incomplete { .. } => "% Incomplete command.".into(),
            other => {
                let pos = match other {
                    CliError::Unknown { pos }
                    | CliError::TooMany { pos }
                    | CliError::BadParam { pos } => pos,
                    _ => 0,
                };
                let chars: Vec<char> = line.chars().collect();
                let n = pos.min(chars.len());
                let mut caret = String::new();
                for _ in 0..n {
                    caret.push(' ');
                }
                caret.push('^');
                format!("{}\n% Invalid input detected at '^' marker.", caret)
            }
        },
        Dialect::Vrp => match e {
            CliError::Custom(msg) => format!("Error: {}", msg),
            CliError::Incomplete { .. } => {
                "Error: Incomplete command found at '^' position.".into()
            }
            CliError::TooMany { .. } => "Error: Too many parameters found at '^' position.".into(),
            CliError::BadParam { .. } => "Error: Wrong parameter found at '^' position.".into(),
            CliError::Unknown { .. } => "Error: Unrecognized command found at '^' position.".into(),
        },
        Dialect::Comware => match e {
            CliError::Custom(msg) => format!("% {}", msg),
            CliError::Incomplete { .. } => "% Incomplete command found at '^' position.".into(),
            CliError::TooMany { .. } => "% Too many parameters found at '^' position.".into(),
            CliError::BadParam { .. } => "% Wrong parameter found at '^' position.".into(),
            CliError::Unknown { .. } => "% Unrecognized command found at '^' position.".into(),
        },
    }
}
