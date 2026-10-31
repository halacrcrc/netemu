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

use cli_engine::{CliError, Ctx, HelpList, Mode, Session};
use sim_core::device::{DevId, DeviceKind, DeviceState, Dialect};
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
    use sim_core::device::{Acl, AclProto, AclRule};
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
        if c.device_kind() != DeviceKind::Switch {
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
                if c.device_kind() == DeviceKind::Switch {
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
        }
        Ok(String::new())
    }

    pub fn shutdown(c: &mut Ctx, up: bool) -> Result<String, CliError> {
        let ifc = require_if(c)?;
        match ifc {
            IfCtx::Physical(port) => {
                c.eng.net.set_port_admin(c.dev, port, up);
            }
            IfCtx::Svi(vlan) => {
                c.eng.net.set_svi_admin(c.dev, vlan, up);
            }
        }
        Ok(String::new())
    }

    // ---------- VLAN（交换机） ----------

    fn require_switch(c: &Ctx) -> Result<(), CliError> {
        if c.device_kind() != DeviceKind::Switch {
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

    /// VRP/Comware `stp enable|disable`；IOS `spanning-tree vlan <n>` / `no spanning-tree vlan <n>`
    pub fn stp_enable(c: &mut Ctx, up: bool) -> Result<String, CliError> {
        stp_of(c)?.enabled = up;
        c.eng.stp_topology_changed();
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
        if c.device_kind() != DeviceKind::Router {
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
        if c.device_kind() != DeviceKind::Router {
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

    /// ACL 视图：`rule [n] {deny|permit} [icmp] source X W [destination X W]`
    pub fn acl_rule(c: &mut Ctx) -> Result<String, CliError> {
        let Some((no, _adv)) = c.sess.sub.acl else {
            return Err(CliError::Custom("no acl selected".into()));
        };
        let permit = c.word("action").eq_ignore_ascii_case("permit");
        let proto = match c.val("proto") {
            Some(cli_engine::Val::Word(w)) if w.eq_ignore_ascii_case("icmp") => AclProto::Icmp,
            _ => AclProto::Any,
        };
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
        if c.device_kind() != DeviceKind::Router {
            return Err(CliError::Custom("Invalid input detected".into()));
        }
        let n = c.number("number").unwrap_or(0);
        let permit = c.word("action").eq_ignore_ascii_case("permit");
        let proto = match c.val("proto") {
            Some(cli_engine::Val::Word(w)) if w.eq_ignore_ascii_case("icmp") => AclProto::Icmp,
            _ => AclProto::Any,
        };
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

    // ---------- 显示 ----------

    pub fn show_acl(c: &mut Ctx) -> Result<String, CliError> {
        if c.device_kind() != DeviceKind::Router {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on routers".into(),
            }));
        }
        Ok(show::acl_table(c.eng, c.dev, c.dialect))
    }

    pub fn show_nat(c: &mut Ctx) -> Result<String, CliError> {
        if c.device_kind() != DeviceKind::Router {
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
        if !matches!(c.device_kind(), DeviceKind::Router | DeviceKind::Switch) {
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
        if !matches!(c.device_kind(), DeviceKind::Router | DeviceKind::Switch) {
            return Err(CliError::Custom(match c.dialect {
                Dialect::Ios => "Invalid input detected".into(),
                _ => "Command is only available on routers".into(),
            }));
        }
        let dst = c.ipv4("dst");
        let len = c.mask_len("mask");
        let nh = c.ipv4("nexthop");
        if dst == 0 || nh == 0 {
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
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    if d.kind == DeviceKind::Host {
        return pc::exec(eng, dev, line);
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

/// 执行一行命令并驱动引擎至静默；返回完整输出（含 ping 结果）。
pub fn exec_and_run(eng: &mut Engine, sess: &mut Session, dev: DevId, line: &str) -> String {
    let kind = eng.net.device(dev).map(|d| d.kind);
    let out = exec_line(eng, sess, dev, line);
    let horizon = eng.drain_horizon();
    eng.run_to_quiescence(horizon);
    let mut parts: Vec<String> = vec![];
    if !out.trim().is_empty() {
        parts.push(out);
    }
    let dialect = eng
        .net
        .device(dev)
        .map(|d| d.dialect)
        .unwrap_or(Dialect::Ios);
    for s in eng.take_unreported_pings(dev) {
        let text = if kind == Some(DeviceKind::Host) {
            show::render_ping_generic(&s)
        } else {
            show::render_ping(dialect, &s)
        };
        parts.push(text);
    }
    parts.join("\n")
}

pub fn prompt(eng: &Engine, sess: &Session, dev: DevId) -> String {
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
        },
        Dialect::Vrp | Dialect::Comware => match sess.mode {
            Mode::Exec | Mode::Priv => format!("<{}>", d.name),
            Mode::Config => format!("[{}]", d.name),
            Mode::If => {
                let ifn = match sess.cur_if {
                    Some(cli_engine::IfCtx::Svi(v)) => ifname::svi_canonical(dialect, v),
                    Some(cli_engine::IfCtx::Physical(p)) => ifname::canonical(dialect, d.kind, p),
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
