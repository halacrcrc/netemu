//! 接口名解析与生成（方言差异集中在这里）。
//!
//! 命名规则（端口以 0 起始的内部序号 idx 表示）：
//! - IOS：`GigabitEthernet0/{idx}`（路由器与交换机一致）
//! - VRP 路由器：`GigabitEthernet0/0/{idx}`；VRP 交换机：`GigabitEthernet0/0/{idx+1}`
//! - Comware：`GigabitEthernet1/0/{idx+1}`（显示缩写 `GE1/0/{idx+1}`）
//! - 主机：`eth0`

use sim_core::device::{DeviceKind, Dialect, VlanId};

/// 接口引用：物理口、SVI 或链路聚合口。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IfRef {
    Physical(usize),
    Svi(VlanId),
    Lag(u16),
}

/// 聚合口的完整名称。
pub fn lag_canonical(dialect: Dialect, id: u16) -> String {
    match dialect {
        Dialect::Ios => format!("Port-channel{}", id),
        Dialect::Vrp => format!("Eth-Trunk{}", id),
        Dialect::Comware => format!("Bridge-Aggregation{}", id),
    }
}

/// 聚合口的短名（表格用）。
pub fn lag_short(dialect: Dialect, id: u16) -> String {
    match dialect {
        Dialect::Ios => format!("Po{}", id),
        Dialect::Vrp => format!("Eth-Trunk{}", id),
        Dialect::Comware => format!("BAGG{}", id),
    }
}

/// SVI 的完整名称。
pub fn svi_canonical(dialect: Dialect, vlan: VlanId) -> String {
    match dialect {
        Dialect::Ios => format!("Vlan{}", vlan),
        Dialect::Vrp => format!("Vlanif{}", vlan),
        Dialect::Comware => format!("Vlan-interface{}", vlan),
    }
}

/// SVI 的短名（表格用）。
pub fn svi_short(dialect: Dialect, vlan: VlanId) -> String {
    match dialect {
        Dialect::Ios => format!("Vlan{}", vlan),
        Dialect::Vrp => format!("Vlanif{}", vlan),
        Dialect::Comware => format!("Vlanif{}", vlan),
    }
}

/// 解析接口名为 IfRef（物理口 / SVI / 聚合口）。SVI 与聚合口仅交换机支持。
pub fn resolve_if(dialect: Dialect, kind: DeviceKind, s: &str) -> Option<IfRef> {
    let lower = s.to_lowercase();
    // SVI 命名
    let svi_prefix = match dialect {
        Dialect::Ios => "vlan",
        Dialect::Vrp => "vlanif",
        Dialect::Comware => "vlan-interface",
    };
    if let Some(rest) = lower.strip_prefix(svi_prefix) {
        if kind != DeviceKind::Switch {
            return None;
        }
        let vlan: u16 = rest.parse().ok()?;
        if vlan == 0 {
            return None;
        }
        return Some(IfRef::Svi(vlan));
    }
    // 聚合口命名（单 token 形式：Port-channel1 / Eth-Trunk1 / Bridge-Aggregation1）
    if kind == DeviceKind::Switch {
        let lag_prefixes: &[&str] = match dialect {
            Dialect::Ios => &["port-channel", "portchannel", "po"],
            Dialect::Vrp => &["eth-trunk"],
            Dialect::Comware => &["bridge-aggregation", "bagg"],
        };
        for p in lag_prefixes {
            if let Some(rest) = lower.strip_prefix(p) {
                if rest.is_empty() {
                    continue;
                }
                if let Ok(id) = rest.parse::<u16>() {
                    if (1..=128).contains(&id) {
                        return Some(IfRef::Lag(id));
                    }
                }
            }
        }
    }
    resolve(dialect, kind, s).map(IfRef::Physical)
}

pub fn canonical(dialect: Dialect, kind: DeviceKind, idx: usize) -> String {
    if kind == DeviceKind::Host {
        return "eth0".into();
    }
    match dialect {
        Dialect::Ios => format!("GigabitEthernet0/{}", idx),
        Dialect::Vrp => match kind {
            DeviceKind::Router => format!("GigabitEthernet0/0/{}", idx),
            _ => format!("GigabitEthernet0/0/{}", idx + 1),
        },
        Dialect::Comware => format!("GigabitEthernet1/0/{}", idx + 1),
    }
}

/// 紧凑缩写（show vlan / mac 表用）：IOS Gi0/x、VRP GE0/0/x、Comware GE1/0/x。
pub fn short(dialect: Dialect, kind: DeviceKind, idx: usize) -> String {
    if kind == DeviceKind::Host {
        return "eth0".into();
    }
    match dialect {
        Dialect::Ios => format!("Gi0/{}", idx),
        Dialect::Vrp => match kind {
            DeviceKind::Router => format!("GE0/0/{}", idx),
            _ => format!("GE0/0/{}", idx + 1),
        },
        Dialect::Comware => format!("GE1/0/{}", idx + 1),
    }
}

/// 表格等显示场合用的缩写（Comware 用 GE 前缀）。
pub fn display(dialect: Dialect, kind: DeviceKind, idx: usize) -> String {
    if kind == DeviceKind::Host {
        return "eth0".into();
    }
    match dialect {
        Dialect::Comware => format!("GE1/0/{}", idx + 1),
        _ => canonical(dialect, kind, idx),
    }
}

/// 把用户输入的接口名解析为端口序号。
pub fn resolve(dialect: Dialect, kind: DeviceKind, s: &str) -> Option<usize> {
    if kind == DeviceKind::Host {
        return if s.eq_ignore_ascii_case("eth0") {
            Some(0)
        } else {
            None
        };
    }
    let lower = s.to_lowercase();
    // 前缀必须是 gigabitethernet 的前缀；VRP/Comware 亦接受标准缩写 "ge"
    let base = "gigabitethernet";
    let pfx_len = lower
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .count();
    if pfx_len == 0 || lower.len() == pfx_len {
        return None;
    }
    let alpha = &lower[..pfx_len];
    if !base.starts_with(alpha) && alpha != "ge" {
        return None;
    }
    let rest = &lower[pfx_len..];
    if rest.is_empty() {
        return None;
    }
    let parts: Vec<&str> = rest.split('/').collect();
    let nums: Option<Vec<u32>> = parts.iter().map(|p| p.parse().ok()).collect();
    let nums = nums?;
    let within = |idx: usize| idx < kind.port_count();
    match (dialect, kind, nums.as_slice()) {
        (Dialect::Ios, _, [0, idx]) if within(*idx as usize) => Some(*idx as usize),
        (Dialect::Vrp, DeviceKind::Router, [0, 0, idx]) if within(*idx as usize) => {
            Some(*idx as usize)
        }
        (Dialect::Vrp, DeviceKind::Switch, [0, 0, idx])
            if *idx >= 1 && within(*idx as usize - 1) =>
        {
            Some(*idx as usize - 1)
        }
        (Dialect::Comware, _, [1, 0, idx]) if *idx >= 1 && within(*idx as usize - 1) => {
            Some(*idx as usize - 1)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ios_roundtrip() {
        let k = DeviceKind::Router;
        for idx in 0..4 {
            let name = canonical(Dialect::Ios, k, idx);
            assert_eq!(resolve(Dialect::Ios, k, &name), Some(idx));
            assert_eq!(resolve(Dialect::Ios, k, &name.to_lowercase()), Some(idx));
        }
        assert_eq!(resolve(Dialect::Ios, k, "g0/0"), Some(0));
        assert_eq!(resolve(Dialect::Ios, k, "gig0/3"), Some(3));
        assert!(resolve(Dialect::Ios, k, "g0/4").is_none());
        assert!(resolve(Dialect::Ios, k, "ethernet0/0").is_none());
    }

    #[test]
    fn vrp_rules() {
        let r = DeviceKind::Router;
        let s = DeviceKind::Switch;
        assert_eq!(resolve(Dialect::Vrp, r, "GigabitEthernet0/0/0"), Some(0));
        assert_eq!(resolve(Dialect::Vrp, r, "g0/0/2"), Some(2));
        assert!(resolve(Dialect::Vrp, r, "g0/0/4").is_none());
        assert_eq!(resolve(Dialect::Vrp, s, "GigabitEthernet0/0/1"), Some(0));
        assert_eq!(resolve(Dialect::Vrp, s, "g0/0/8"), Some(7));
        assert!(resolve(Dialect::Vrp, s, "g0/0/0").is_none());
    }

    #[test]
    fn comware_rules() {
        let s = DeviceKind::Switch;
        assert_eq!(
            resolve(Dialect::Comware, s, "GigabitEthernet1/0/1"),
            Some(0)
        );
        assert_eq!(resolve(Dialect::Comware, s, "ge1/0/8"), Some(7));
        assert!(resolve(Dialect::Comware, s, "g1/0/9").is_none());
        assert!(resolve(Dialect::Comware, s, "g0/0/1").is_none());
        assert_eq!(display(Dialect::Comware, s, 0), "GE1/0/1");
    }
}
