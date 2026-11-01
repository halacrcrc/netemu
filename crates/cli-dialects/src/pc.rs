//! 主机（PC）的简易命令行：与方言无关的迷你 shell。

use cli_engine::{tokenize, LoginFlow, LoginStep, Session};
use sim_core::device::{ArpEntry, DevId, DeviceKind, DeviceState, Dialect, MgmtProto};
use sim_core::engine::Engine;
use sim_packets::{ipv4_str, Ipv4Addr};

use crate::ipv4_from_str;

pub fn exec(eng: &mut Engine, sess: &mut Session, dev: DevId, line: &str) -> String {
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
        "ping6" => {
            if toks.len() < 2 {
                return "用法: ping6 <IPv6地址>".into();
            }
            let Some(target) = sim_packets::ipv6_from_str(&toks[1].0) else {
                return format!("无效的 IPv6 地址: {}", toks[1].0);
            };
            eng.start_ping6(dev, target);
            String::new()
        }
        "tcp" => {
            if toks.len() < 3 {
                return "用法: tcp <IP地址> <端口>".into();
            }
            let Some(target) = ipv4_from_str(&toks[1].0) else {
                return format!("无效的 IP 地址: {}", toks[1].0);
            };
            let Ok(port) = toks[2].0.parse::<u16>() else {
                return format!("无效的端口: {}", toks[2].0);
            };
            eng.start_tcp_probe(dev, target, port);
            String::new()
        }
        "wifi" => {
            let sub = toks.get(1).map(|t| t.0.to_lowercase()).unwrap_or_default();
            if sub == "status" {
                let Some(d) = eng.net.device(dev) else {
                    return String::new();
                };
                if let DeviceState::Host(hs) = &d.state {
                    return match &hs.wifi {
                        Some(w) => {
                            let ap = eng
                                .net
                                .device(w.ap)
                                .map(|x| x.name.clone())
                                .unwrap_or_else(|| "?".into());
                            format!("Wi-Fi: 已连接 SSID {}（AP {}）", w.ssid, ap)
                        }
                        None => "Wi-Fi: 未连接".into(),
                    };
                }
                return String::new();
            }
            if sub == "roam" {
                return match eng.wifi_roam(dev) {
                    Ok(ap) => {
                        let ap_name = eng
                            .net
                            .device(ap)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| "?".into());
                        format!("Wi-Fi 已漫游到 AP {}", ap_name)
                    }
                    Err(e) => format!("Wi-Fi 漫游失败: {}", e),
                };
            }
            if sub != "connect" || toks.len() < 3 {
                return "用法: wifi connect <SSID> [口令] | wifi roam | wifi status".into();
            }
            let ssid = &toks[2].0;
            let pass = toks.get(3).map(|t| t.0.as_str()).unwrap_or("");
            match eng.wifi_connect(dev, ssid, pass) {
                Ok(ap) => {
                    let ap_name = eng
                        .net
                        .device(ap)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "?".into());
                    format!("Wi-Fi 已连接：{}（AP {}）", ssid, ap_name)
                }
                Err(e) => format!("Wi-Fi 连接失败: {}", e),
            }
        }
        "sslvpn" => {
            if toks.len() < 5 || !toks[1].0.eq_ignore_ascii_case("connect") {
                return "用法: sslvpn connect <网关IP> <用户名> <口令>".into();
            }
            let Some(gw) = ipv4_from_str(&toks[2].0) else {
                return format!("无效的 IP 地址: {}", toks[2].0);
            };
            match eng.sslvpn_connect(dev, gw, &toks[3].0, &toks[4].0) {
                Ok(vip) => format!(
                    "SSL VPN 已连接：虚拟 IP {}（网关 {}）",
                    ipv4_str(vip),
                    ipv4_str(gw)
                ),
                Err(e) => format!("SSL VPN 连接失败: {}", e),
            }
        }
        "telnet" | "ssh" => {
            if toks.len() < 2 {
                return "用法: telnet <IP地址> 或 ssh <IP地址>".into();
            }
            let Some(target_ip) = ipv4_from_str(&toks[1].0) else {
                return format!("无效的 IP 地址: {}", toks[1].0);
            };
            let proto = if cmd.eq_ignore_ascii_case("ssh") {
                MgmtProto::Ssh
            } else {
                MgmtProto::Telnet
            };
            start_login(eng, sess, dev, target_ip, proto)
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
            "可用命令:\n  ping <IP>       测试到目标 IP 的连通性\n  ping6 <IPv6>    测试 IPv6 连通性\n  tcp <IP> <端口> 发起 TCP 握手（22/80/443 开放）\n  wifi connect <SSID> [口令]  连接无线 AP\n  wifi roam       漫游到同 SSID 的另一 AP\n  wifi status     查看无线连接状态\n  sslvpn connect <网关IP> <用户> <口令> 建立 SSL VPN\n  telnet <IP>     用 Telnet 登录网络设备\n  ssh <IP>        用 SSH 登录网络设备\n  ipconfig        查看本机网络配置\n  ipconfig /renew 重新发起 DHCP 获取（或输入 dhcp）\n  arp             查看本机 ARP 缓存\n  help            显示本帮助".into()
        }
        other => format!("未知命令: {}（输入 help 查看可用命令）", other),
    }
}

fn start_login(
    eng: &mut Engine,
    sess: &mut Session,
    dev: DevId,
    target_ip: Ipv4Addr,
    proto: MgmtProto,
) -> String {
    match eng.login_target(dev, target_ip, proto) {
        Ok(target) => {
            sess.login = Some(LoginFlow {
                target,
                target_ip,
                proto,
                step: LoginStep::Username,
                user: String::new(),
            });
            format!("Trying {} ...", ipv4_str(target_ip))
        }
        Err(e) => crate::login_error_text(DeviceKind::Host, Dialect::Ios, e),
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
    if let Some(ip6) = p.ipv6 {
        out.push_str(&format!(
            "\n  IPv6 地址 . . . . : {}/{}",
            sim_packets::ipv6_str(&ip6),
            p.prefix6
        ));
        let gw6 = match &d.state {
            DeviceState::Host(h) => h.gw6,
            _ => None,
        };
        out.push_str("\n  IPv6 默认网关 . . : ");
        match gw6 {
            Some(g) => out.push_str(&sim_packets::ipv6_str(&g)),
            None => out.push_str("未配置"),
        }
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
