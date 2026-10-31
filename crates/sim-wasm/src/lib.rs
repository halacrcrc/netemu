//! WASM 桥：把仿真内核与 CLI 暴露给前端。
//!
//! 边界约定：所有结构化数据以 JSON 字符串传递，TS 侧类型见 web/src/lib/sim.ts。

use std::collections::HashMap;

use sim_core::device::{DeviceKind, DeviceState, Dialect};
use sim_core::engine::{Engine, PING_HORIZON_NS};
use sim_core::summary::NetSummary;
use sim_core::{DevId, Net};
use sim_packets::ipv4_from_str;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Sim {
    eng: Engine,
    sessions: HashMap<DevId, cli_engine::Session>,
    kind_counts: HashMap<String, u32>,
}

fn kind_from(s: &str) -> Option<DeviceKind> {
    match s {
        "Router" | "router" => Some(DeviceKind::Router),
        "Switch" | "switch" => Some(DeviceKind::Switch),
        "Host" | "host" | "PC" | "pc" => Some(DeviceKind::Host),
        _ => None,
    }
}

fn dialect_from(s: &str) -> Option<Dialect> {
    match s {
        "Ios" | "ios" | "cisco" => Some(Dialect::Ios),
        "Vrp" | "vrp" | "huawei" => Some(Dialect::Vrp),
        "Comware" | "comware" | "h3c" => Some(Dialect::Comware),
        _ => None,
    }
}

impl Sim {
    fn sync_sessions(&mut self) {
        // 移除已删除设备的会话
        let ids: Vec<u32> = self.sessions.keys().copied().collect();
        for id in ids {
            if self.eng.net.device(id).is_none() {
                self.sessions.remove(&id);
            }
        }
    }
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Sim {
        Sim {
            eng: Engine::new(Net::new()),
            sessions: HashMap::new(),
            kind_counts: HashMap::new(),
        }
    }

    /// 添加设备，自动命名（Router1 / Switch1 / PC1…），返回设备 id。
    pub fn add_device(&mut self, kind: &str, dialect: &str) -> Result<u32, JsValue> {
        let kind = kind_from(kind).ok_or_else(|| JsValue::from_str("unknown device kind"))?;
        let dialect = dialect_from(dialect).ok_or_else(|| JsValue::from_str("unknown dialect"))?;
        let base = match kind {
            DeviceKind::Router => "Router",
            DeviceKind::Switch => "Switch",
            DeviceKind::Host => "PC",
        };
        let n = self.kind_counts.entry(base.to_string()).or_insert(0);
        *n += 1;
        let name = format!("{}{}", base, n);
        let id = self.eng.net.add_device(name, kind, dialect);
        self.sessions.insert(id, cli_engine::Session::default());
        Ok(id)
    }

    pub fn remove_device(&mut self, dev: u32) {
        self.eng.net.remove_device(dev);
        self.sessions.remove(&dev);
        self.sync_sessions();
    }

    pub fn rename_device(&mut self, dev: u32, name: &str) -> Result<(), JsValue> {
        let Some(d) = self.eng.net.device_mut(dev) else {
            return Err(JsValue::from_str("device not found"));
        };
        d.name = name.to_string();
        Ok(())
    }

    pub fn set_dialect(&mut self, dev: u32, dialect: &str) -> Result<(), JsValue> {
        let d = dialect_from(dialect).ok_or_else(|| JsValue::from_str("unknown dialect"))?;
        let Some(dev_ref) = self.eng.net.device_mut(dev) else {
            return Err(JsValue::from_str("device not found"));
        };
        dev_ref.dialect = d;
        self.sessions.insert(dev, cli_engine::Session::default());
        Ok(())
    }

    /// 连接两台设备（自动选取两端第一个空闲端口），返回链路 id。
    pub fn add_link(&mut self, a: u32, b: u32) -> Result<u32, JsValue> {
        self.eng
            .net
            .add_link(a, b)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn remove_link(&mut self, link: u32) {
        self.eng.net.remove_link(link);
    }

    /// 启用/关闭主机 DHCP，并立即发起 Discover（驱动至静默）。
    pub fn set_host_dhcp(&mut self, dev: u32, enable: bool) -> Result<String, JsValue> {
        let Some(d) = self.eng.net.device_mut(dev) else {
            return Err(JsValue::from_str("device not found"));
        };
        if d.kind != DeviceKind::Host {
            return Err(JsValue::from_str("not a host"));
        }
        if let DeviceState::Host(h) = &mut d.state {
            h.dhcp_enable = enable;
            h.dhcp = if enable {
                Some(sim_core::device::HostDhcp { discovering: false })
            } else {
                None
            };
        }
        if enable {
            self.eng.dhcp_discover(dev);
        } else if let Some(p) = d.port_mut(0) {
            p.ip = None;
            if let DeviceState::Host(h) = &mut d.state {
                h.gw = None;
            }
        }
        let h = self.eng.drain_horizon();
        self.eng.run_to_quiescence(h);
        let Some(d) = self.eng.net.device(dev) else {
            return Ok("{}".into());
        };
        Ok(serde_json::json!({
            "ip": d.ports[0].ip.map(sim_packets::ipv4_str),
            "prefix": d.ports[0].prefix,
        })
        .to_string())
    }

    /// 设置主机 IP/掩码/网关（空串表示清除）。
    pub fn set_host_config(
        &mut self,
        dev: u32,
        ip: &str,
        prefix: u8,
        gw: &str,
    ) -> Result<(), JsValue> {
        let Some(d) = self.eng.net.device(dev) else {
            return Err(JsValue::from_str("device not found"));
        };
        if d.kind != DeviceKind::Host {
            return Err(JsValue::from_str("not a host"));
        }
        let ip = if ip.trim().is_empty() {
            None
        } else {
            Some(ipv4_from_str(ip).ok_or_else(|| JsValue::from_str("invalid ip"))?)
        };
        let gw = if gw.trim().is_empty() {
            None
        } else {
            Some(ipv4_from_str(gw).ok_or_else(|| JsValue::from_str("invalid gateway"))?)
        };
        self.eng.net.set_host(dev, ip, prefix, gw);
        Ok(())
    }

    /// 执行一行 CLI 并驱动引擎至静默，返回输出文本。
    pub fn exec_command(&mut self, dev: u32, line: &str) -> Result<String, JsValue> {
        if self.eng.net.device(dev).is_none() {
            return Err(JsValue::from_str("device not found"));
        }
        self.sessions.entry(dev).or_default();
        let Some(sess) = self.sessions.get_mut(&dev) else {
            return Err(JsValue::from_str("device not found"));
        };
        Ok(cli_dialects::exec_and_run(&mut self.eng, sess, dev, line))
    }

    /// 当前提示符。
    pub fn prompt(&mut self, dev: u32) -> Result<String, JsValue> {
        self.sessions.entry(dev).or_default();
        let Some(sess) = self.sessions.get_mut(&dev) else {
            return Err(JsValue::from_str("device not found"));
        };
        Ok(cli_dialects::prompt(&self.eng, sess, dev))
    }

    /// Tab 补全：返回 JSON {line, options}。
    pub fn complete(&mut self, dev: u32, line: &str) -> Result<String, JsValue> {
        self.sessions.entry(dev).or_default();
        let Some(sess) = self.sessions.get_mut(&dev) else {
            return Err(JsValue::from_str("device not found"));
        };
        let (l, opts) = cli_dialects::complete_line(&self.eng, sess, dev, line);
        Ok(serde_json::json!({ "line": l, "options": opts }).to_string())
    }

    /// 运行引擎至静默（加载拓扑或外部改动后调用）。
    pub fn run(&mut self) -> String {
        let s = self.eng.run_to_quiescence(PING_HORIZON_NS);
        serde_json::json!({
            "virtualNs": s.virtual_ns,
            "events": s.events,
            "inFlight": s.in_flight,
            "dropped": s.dropped,
        })
        .to_string()
    }

    /// 引擎统计。
    pub fn stats(&mut self) -> String {
        let s = self.eng.stats();
        serde_json::json!({
            "virtualNs": s.virtual_ns,
            "events": s.events,
            "inFlight": s.in_flight,
            "dropped": s.dropped,
        })
        .to_string()
    }

    /// 拓扑摘要（含配置态）。
    pub fn dump(&self) -> String {
        serde_json::to_string(&self.eng.net.summary()).unwrap_or_else(|_| "{}".into())
    }

    /// 应用拓扑摘要（保存/加载/分享的数据格式）。
    pub fn load(&mut self, json: &str) -> Result<(), JsValue> {
        let summary: NetSummary = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("invalid topology: {}", e)))?;
        self.eng = Engine::new(Net::new());
        self.sessions.clear();
        self.kind_counts.clear();
        self.eng
            .net
            .apply_summary(&summary)
            .map_err(|e| JsValue::from_str(&e))?;
        // 重建自动命名计数（从现有名字推断）
        for d in &self.eng.net.devices {
            let base = match d.kind {
                DeviceKind::Router => "Router",
                DeviceKind::Switch => "Switch",
                DeviceKind::Host => "PC",
            };
            if let Some(sfx) = d.name.strip_prefix(base) {
                if let Ok(n) = sfx.parse::<u32>() {
                    let slot = self.kind_counts.entry(base.to_string()).or_insert(0);
                    *slot = (*slot).max(n);
                }
            }
        }
        Ok(())
    }

    /// 清空一切。
    pub fn reset(&mut self) {
        self.eng = Engine::new(Net::new());
        self.sessions.clear();
        self.kind_counts.clear();
    }

    // ---------- 实验检查（lab-checks） ----------

    /// 检查连通性：从 from 设备 ping 目标 IP。
    pub fn check_ping(&mut self, from: u32, to: &str) -> Result<String, JsValue> {
        let to = ipv4_from_str(to).ok_or_else(|| JsValue::from_str("invalid ip"))?;
        let checks = vec![lab_checks::Check {
            label: String::new(),
            kind: lab_checks::CheckKind::Ping {
                from,
                to,
                expect: true,
            },
        }];
        let r = lab_checks::run_checks(&mut self.eng, &checks);
        Ok(serde_json::json!({
            "passed": r[0].passed,
            "detail": r[0].detail,
        })
        .to_string())
    }

    /// 检查路由器是否存在去往 dst/len 的静态路由。
    pub fn check_static_route(&mut self, dev: u32, dst: &str) -> Result<String, JsValue> {
        let (dst, len) = lab_checks::parse_net(dst)
            .ok_or_else(|| JsValue::from_str("invalid net, expect a.b.c.d/len"))?;
        let checks = vec![lab_checks::Check {
            label: String::new(),
            kind: lab_checks::CheckKind::RouteExists {
                dev,
                dst,
                len,
                src: Some(sim_core::device::RouteSrc::Static),
            },
        }];
        let r = lab_checks::run_checks(&mut self.eng, &checks);
        Ok(serde_json::json!({
            "passed": r[0].passed,
            "detail": r[0].detail,
        })
        .to_string())
    }

    /// 检查 VLAN 已创建。
    pub fn check_vlan(&mut self, dev: u32, vlan: u16) -> Result<String, JsValue> {
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::VlanExists { dev, vlan },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    /// 检查端口 Access VLAN。
    pub fn check_access_vlan(&mut self, dev: u32, port: u32, vlan: u16) -> Result<String, JsValue> {
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::AccessVlan {
                    dev,
                    port: port as usize,
                    vlan,
                },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    /// 检查 Trunk 放行 VLAN。
    pub fn check_trunk_allows(
        &mut self,
        dev: u32,
        port: u32,
        vlan: u16,
    ) -> Result<String, JsValue> {
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::TrunkAllows {
                    dev,
                    port: port as usize,
                    vlan,
                },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    /// 检查接口 IP（含主机口）。
    pub fn check_port_ip(&mut self, dev: u32, port: u32, ip: &str) -> Result<String, JsValue> {
        use sim_packets::ipv4_from_str;
        let ip = ipv4_from_str(ip).ok_or_else(|| JsValue::from_str("invalid ip"))?;
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::PortIp {
                    dev,
                    port: port as usize,
                    ip,
                },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    /// 检查路由存在（连接或静态均可）。
    pub fn check_route(&mut self, dev: u32, dst: &str) -> Result<String, JsValue> {
        let (dst, len) = lab_checks::parse_net(dst)
            .ok_or_else(|| JsValue::from_str("invalid net, expect a.b.c.d/len"))?;
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::RouteExists {
                    dev,
                    dst,
                    len,
                    src: None,
                },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    /// 校验一条命令在指定方言/模式下能否被解析（对照表与模拟器一致性的运行时验证）。
    /// dialect: Ios | Vrp | Comware；mode: Exec | Priv | Config | If | Pool | Acl
    pub fn verify_command(&self, dialect: &str, mode: &str, line: &str) -> bool {
        let d = match dialect {
            "Ios" | "ios" => Dialect::Ios,
            "Vrp" | "vrp" => Dialect::Vrp,
            "Comware" | "comware" => Dialect::Comware,
            _ => return false,
        };
        let m = match mode {
            "Exec" => cli_engine::Mode::Exec,
            "Priv" => cli_engine::Mode::Priv,
            "Config" => cli_engine::Mode::Config,
            "If" => cli_engine::Mode::If,
            "Pool" => cli_engine::Mode::Pool,
            "Acl" => cli_engine::Mode::Acl,
            _ => return false,
        };
        let trees = cli_dialects::trees(d);
        cli_engine::parse(trees.root_for(m), line)
            .map(|r| r.is_some())
            .unwrap_or(false)
    }

    /// 检查是否 STP 根桥。
    pub fn check_root_bridge(&mut self, dev: u32) -> Result<String, JsValue> {
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::RootBridge { dev },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    /// 检查端口 STP 角色：role ∈ "Designated" | "Root" | "Alternate"。
    pub fn check_port_role(&mut self, dev: u32, port: u32, role: &str) -> Result<String, JsValue> {
        use sim_core::device::StpRole;
        let role = match role {
            "Designated" | "DESI" => StpRole::Designated,
            "Root" | "ROOT" => StpRole::Root,
            "Alternate" | "ALTE" => StpRole::Alternate,
            _ => return Err(JsValue::from_str("unknown role")),
        };
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::PortRole {
                    dev,
                    port: port as usize,
                    role,
                },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    /// 检查 SVI IP。
    pub fn check_svi_ip(&mut self, dev: u32, vlan: u16, ip: &str) -> Result<String, JsValue> {
        use sim_packets::ipv4_from_str;
        let ip = ipv4_from_str(ip).ok_or_else(|| JsValue::from_str("invalid ip"))?;
        let r = lab_checks::run_checks(
            &mut self.eng,
            &[lab_checks::Check {
                label: String::new(),
                kind: lab_checks::CheckKind::SviIp { dev, vlan, ip },
            }],
        );
        Ok(serde_json::json!({ "passed": r[0].passed, "detail": r[0].detail }).to_string())
    }

    // ---------- 抓包 ----------

    /// 开关某条链路的抓包。
    pub fn set_capture(&mut self, link: u32, on: bool) -> Result<(), JsValue> {
        if !self.eng.net.set_capture(link, on) {
            return Err(JsValue::from_str("link not found"));
        }
        Ok(())
    }

    /// 读取抓包记录：返回 JSON { next, rows: [{seq,ms,dir,src,dst,kind,vlan,info}] }。
    /// since 之前的记录被跳过；next = 建议下次传入的 since。
    pub fn capture_rows(&self, link: u32, since: u32) -> Result<String, JsValue> {
        use sim_packets::{ArpOp, L3};
        let Some(l) = self.eng.net.link(link) else {
            return Err(JsValue::from_str("link not found"));
        };
        let rows: Vec<serde_json::Value> = l
            .capture_buf
            .iter()
            .filter(|c| c.seq >= since)
            .map(|c| {
                let (kind, info) = match &c.frame.payload {
                    L3::Arp(a) => (
                        "ARP",
                        format!(
                            "{} {} → {}  谁有 {}? 告诉 {}",
                            if a.op == ArpOp::Request {
                                "请求"
                            } else {
                                "应答"
                            },
                            sim_packets::ipv4_str(a.sender_ip),
                            sim_packets::ipv4_str(a.target_ip),
                            sim_packets::ipv4_str(a.target_ip),
                            sim_packets::ipv4_str(a.sender_ip),
                        ),
                    ),
                    L3::Ipv4(p) => (
                        if p.icmp.is_reply {
                            "EchoReply"
                        } else {
                            "EchoRequest"
                        },
                        format!(
                            "{} → {}  seq={} ttl={}",
                            sim_packets::ipv4_str(p.src),
                            sim_packets::ipv4_str(p.dst),
                            p.icmp.seq,
                            p.ttl
                        ),
                    ),
                    L3::Bpdu(b) => (
                        "BPDU",
                        format!(
                            "root 优先级={} cost={} 发送者优先级={}",
                            b.root.priority, b.cost, b.sender.priority
                        ),
                    ),
                    L3::Dhcp(d) => (
                        match d.op {
                            sim_packets::DhcpOp::Discover => "DHCP发现",
                            sim_packets::DhcpOp::Offer => "DHCP提供",
                            sim_packets::DhcpOp::Request => "DHCP请求",
                            sim_packets::DhcpOp::Ack => "DHCP确认",
                        },
                        format!(
                            "{} ← {}  提议 {}",
                            sim_packets::mac_str(&d.client_mac),
                            sim_packets::ipv4_str(d.server_ip),
                            sim_packets::ipv4_str(d.offered_ip)
                        ),
                    ),
                };
                serde_json::json!({
                    "seq": c.seq,
                    "ms": c.ns as f64 / 1e6,
                    "dir": c.dir,
                    "srcMac": sim_packets::mac_str(&c.frame.src),
                    "dstMac": sim_packets::mac_str(&c.frame.dst),
                    "kind": kind,
                    "vlan": c.frame.vlan,
                    "info": info,
                })
            })
            .collect();
        let next = l.capture_next;
        Ok(serde_json::json!({ "next": next, "rows": rows }).to_string())
    }
}

impl Default for Sim {
    fn default() -> Self {
        Self::new()
    }
}
