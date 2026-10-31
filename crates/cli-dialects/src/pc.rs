//! 主机（PC）的简易命令行：与方言无关的迷你 shell。

use cli_engine::tokenize;
use sim_core::device::{ArpEntry, DevId, DeviceKind, DeviceState};
use sim_core::engine::Engine;
use sim_packets::ipv4_str;

use crate::ipv4_from_str;

pub fn exec(eng: &mut Engine, dev: DevId, line: &str) -> String {
    let toks = tokenize(line.trim());
    if toks.is_empty() {
        return String::new();
    }
    let (cmd, _) = &toks[0];
    match cmd.to_lowercase().as_str() {
        "ping" => {
            if toks.len() < 2 {
                return "用法: ping <IP地址>".into();
            }
            let Some(target) = ipv4_from_str(&toks[1].0) else {
                return format!("无效的 IP 地址: {}", toks[1].0);
            };
            eng.start_ping(dev, target);
            String::new() // 结果在引擎静默后由 render_ping_generic 输出
        }
        "ipconfig" => {
            // ipconfig /renew：重新发起 DHCP
            if toks.len() > 1 && toks[1].0.to_lowercase().contains("renew") {
                eng.dhcp_discover(dev);
                String::new()
            } else {
                ipconfig(eng, dev)
            }
        }
        "dhcp" => {
            // dhcp renew：重新发起 Discover
            eng.dhcp_discover(dev);
            String::new()
        }
        "arp" => arp_table(eng, dev),
        "help" | "?" => {
            "可用命令:\n  ping <IP>       测试到目标 IP 的连通性\n  ipconfig        查看本机网络配置\n  ipconfig /renew 重新发起 DHCP 获取（或输入 dhcp）\n  arp             查看本机 ARP 缓存\n  help            显示本帮助".into()
        }
        other => format!("未知命令: {}（输入 help 查看可用命令）", other),
    }
}

fn ipconfig(eng: &Engine, dev: DevId) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let p = &d.ports[0];
    let gw = match &d.state {
        DeviceState::Host(h) => h.gw,
        _ => None,
    };
    let mut out = format!(
        "网卡 eth0:\n  硬件地址 . . . . : {}\n  IPv4 地址  . . . : ",
        sim_packets::mac_str(&p.mac)
    );
    match p.ip {
        Some(ip) => out.push_str(&format!("{}/{}", ipv4_str(ip), p.prefix)),
        None => out.push_str("未配置"),
    }
    out.push_str("\n  默认网关 . . . . : ");
    match gw {
        Some(gw) => out.push_str(&ipv4_str(gw)),
        None => out.push_str("未配置"),
    }
    out.push_str("\n  状态 . . . . . . : ");
    if p.admin_up && p.link.is_some() {
        out.push_str("已连接");
    } else {
        out.push_str("链路断开");
    }
    out
}

fn arp_table(eng: &Engine, dev: DevId) -> String {
    let Some(d) = eng.net.device(dev) else {
        return String::new();
    };
    let DeviceState::Host(h) = &d.state else {
        return String::new();
    };
    if h.arp.is_empty() {
        return "ARP 缓存为空。".into();
    }
    let mut entries: Vec<(&u32, &ArpEntry)> = h.arp.iter().collect();
    entries.sort_by_key(|(ip, _)| **ip);
    let mut out = String::new();
    out.push_str("Internet 地址          物理地址              类型\n");
    for (ip, e) in entries {
        out.push_str(&format!(
            "{:<22} {:<20} 动态\n",
            ipv4_str(*ip),
            sim_packets::mac_str(&e.mac)
        ));
    }
    out.trim_end().to_string()
}

/// 主机不支持方言命令树，占位保证类型完整。
pub const KIND: DeviceKind = DeviceKind::Host;
