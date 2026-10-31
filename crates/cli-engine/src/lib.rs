//! CLI 命令树框架（与方言无关）。
//!
//! 方言（IOS/VRP/Comware）在 cli-dialects 中把命令树"数据"建出来，
//! 本 crate 负责解析、前缀缩写、`?` 帮助与 Tab 补全的通用机制。

use sim_core::device::{DevId, DeviceKind, Dialect, MgmtProto, PortId, VlanId};
use sim_core::engine::Engine;

/// 参数取值
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    Ip(u32),
    /// 点分掩码（已转为前缀长度）
    MaskLen(u8),
    PrefixLen(u8),
    Number(u32),
    /// VLAN 列表（已展开为编号集合）
    VlanList(Vec<u16>),
    Word(String),
    /// 空格分隔的多词列表（贪婪到行尾）
    WordList(Vec<String>),
    /// 接口名原始串（方言层解析为端口号）
    Raw(String),
}

/// 命令处理函数：参数在 ctx.vals 中按 (参数名, 值) 存放。
pub type CmdFn = fn(&mut Ctx) -> Result<String, CliError>;

pub enum Tok {
    Lit(String),
    Param { kind: ParamKind, name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    Ipv4,
    DottedMask,
    /// ACL 通配符：点分十进制或单个整数（0 = 0.0.0.0）
    Wildcard,
    PrefixLen,
    Number,
    /// VLAN 列表（"10,20" / "10 20" / "10-12"）
    VlanList,
    Word,
    /// 空格分隔的多词列表（贪婪到行尾）
    WordList,
    /// 接口名（由方言层解析）
    IfName,
}

/// 命令树节点。
pub struct CNode {
    pub tok: Tok,
    pub help: String,
    pub run: Option<CmdFn>,
    pub kids: Vec<CNode>,
}

impl CNode {
    pub fn lit(word: &str, help: &str) -> Self {
        Self {
            tok: Tok::Lit(word.to_string()),
            help: help.to_string(),
            run: None,
            kids: vec![],
        }
    }

    pub fn param(kind: ParamKind, name: &str, help: &str) -> Self {
        Self {
            tok: Tok::Param {
                kind,
                name: name.to_string(),
            },
            help: help.to_string(),
            run: None,
            kids: vec![],
        }
    }

    pub fn run(mut self, f: CmdFn) -> Self {
        self.run = Some(f);
        self
    }

    pub fn kid(mut self, k: CNode) -> Self {
        self.kids.push(k);
        self
    }

    pub fn kids(mut self, ks: Vec<CNode>) -> Self {
        self.kids.extend(ks);
        self
    }
}

/// 当前进入的接口上下文。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IfCtx {
    /// 物理端口
    Physical(PortId),
    /// 三层交换机 SVI
    Svi(VlanId),
    /// 链路聚合逻辑接口
    Lag(u16),
}

/// 当前进入的子视图上下文（地址池 / ACL / AAA 用户）。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SubCtx {
    /// 地址池名（Pool 模式）
    pub pool: Option<String>,
    /// ACL 编号（Acl 模式）；basic = 1-199 中的基础段
    pub acl: Option<(u32, bool /*advanced*/)>,
    /// AAA 视图中正在配置的本地用户
    pub aaa_user: Option<String>,
    /// 路由策略视图中正在编辑的 (策略名, 节点 seq)
    pub rp: Option<(String, u16)>,
}

/// 远程登录输入流程。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginFlow {
    /// 目标设备（仅用于展示；认证时按 target_ip 重新定位）
    pub target: DevId,
    pub target_ip: u32,
    pub proto: MgmtProto,
    pub step: LoginStep,
    pub user: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginStep {
    Username,
    Password,
}

/// 已建立的远程会话（命令转发到目标设备）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteSession {
    pub dev: DevId,
    pub inner: Box<Session>,
}

/// CLI 会话状态（每设备一份，由前端持有）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    pub mode: Mode,
    pub cur_if: Option<IfCtx>,
    pub sub: SubCtx,
    /// 正在等待用户名/口令的登录流程
    pub login: Option<LoginFlow>,
    /// 已建立、尚未断开的远程会话
    pub remote: Option<RemoteSession>,
}

impl Session {
    /// 当前实际接收命令的设备（远程会话时是目标设备）。
    pub fn effective_dev(&self, local: DevId) -> DevId {
        self.remote.as_ref().map(|r| r.dev).unwrap_or(local)
    }
}

/// 模式编号三方言共用：IOS 用满四层；VRP/Comware 用 Exec(用户视图)/Config(系统视图)/If。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Exec,
    /// 仅 IOS：特权模式
    Priv,
    /// IOS 全局配置 / VRP、Comware 系统视图
    Config,
    /// 接口配置
    If,
    /// IOS `ip dhcp pool` / VRP、Comware `ip pool` 地址池视图
    Pool,
    /// VRP、Comware `acl` 视图
    Acl,
    /// VTY 线路视图
    Line,
    /// AAA / 本地用户视图
    Aaa,
    /// MSTP 域配置视图
    Mst,
    /// OSPF 进程视图
    Ospf,
    /// OSPF 区域视图（VRP/Comware）
    OspfArea,
    /// BGP 视图
    Bgp,
    /// IS-IS 视图
    Isis,
    /// 路由策略视图（route-map / route-policy node）
    RoutePolicy,
}

#[derive(Debug)]
pub enum CliError {
    Unknown {
        pos: usize,
    },
    Incomplete {
        pos: usize,
    },
    TooMany {
        pos: usize,
    },
    BadParam {
        pos: usize,
    },
    /// 处理器主动报错（内容由方言层包装输出）
    Custom(String),
}

pub struct Ctx<'a> {
    pub eng: &'a mut Engine,
    pub dev: DevId,
    pub sess: &'a mut Session,
    pub dialect: Dialect,
    pub vals: Vec<(String, Val)>,
}

impl Ctx<'_> {
    pub fn val(&self, name: &str) -> Option<&Val> {
        self.vals.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn ipv4(&self, name: &str) -> u32 {
        match self.val(name) {
            Some(Val::Ip(ip)) => *ip,
            _ => 0,
        }
    }

    pub fn mask_len(&self, name: &str) -> u8 {
        match self.val(name) {
            Some(Val::MaskLen(l)) => *l,
            Some(Val::PrefixLen(l)) => *l,
            _ => 24,
        }
    }

    pub fn word(&self, name: &str) -> String {
        match self.val(name) {
            Some(Val::Word(w)) | Some(Val::Raw(w)) => w.clone(),
            _ => String::new(),
        }
    }

    pub fn number(&self, name: &str) -> Option<u32> {
        match self.val(name) {
            Some(Val::Number(n)) => Some(*n),
            _ => None,
        }
    }

    pub fn vlan_list(&self, name: &str) -> Option<Vec<u16>> {
        match self.val(name) {
            Some(Val::VlanList(v)) => Some(v.clone()),
            _ => None,
        }
    }

    pub fn word_list(&self, name: &str) -> Option<Vec<String>> {
        match self.val(name) {
            Some(Val::WordList(v)) => Some(v.clone()),
            _ => None,
        }
    }

    pub fn device_kind(&self) -> DeviceKind {
        self.eng
            .net
            .device(self.dev)
            .map(|d| d.kind)
            .unwrap_or(DeviceKind::Router)
    }
}

// ---------- 解析 ----------

pub struct Parsed {
    pub run: CmdFn,
    pub vals: Vec<(String, Val)>,
}

/// 词法：返回 (token, 起始字符序号)。
pub fn tokenize(line: &str) -> Vec<(String, usize)> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut start = 0usize;
    let mut in_tok = false;
    for (i, ch) in line.chars().enumerate() {
        if ch.is_whitespace() {
            if in_tok {
                out.push((cur.clone(), start));
                cur.clear();
                in_tok = false;
            }
        } else {
            if !in_tok {
                start = i;
                in_tok = true;
            }
            cur.push(ch);
        }
    }
    if in_tok {
        out.push((cur, start));
    }
    out
}

fn parse_param(kind: ParamKind, word: &str) -> Option<Val> {
    match kind {
        ParamKind::Ipv4 => sim_packets::ipv4_from_str(word).map(Val::Ip),
        ParamKind::DottedMask => {
            let ip = sim_packets::ipv4_from_str(word)?;
            sim_packets::prefix_from_mask(ip).map(Val::MaskLen)
        }
        ParamKind::Wildcard => {
            if word.contains('.') {
                sim_packets::ipv4_from_str(word).map(Val::Ip)
            } else {
                word.parse::<u32>().ok().map(Val::Ip)
            }
        }
        ParamKind::PrefixLen => {
            let n: u8 = word.parse().ok()?;
            if n <= 32 {
                Some(Val::PrefixLen(n))
            } else {
                None
            }
        }
        ParamKind::Number => word.parse::<u32>().ok().map(Val::Number),
        ParamKind::VlanList => parse_vlan_list(word).map(Val::VlanList),
        ParamKind::Word => Some(Val::Word(word.to_string())),
        ParamKind::WordList => Some(Val::WordList(
            word.split_whitespace().map(|s| s.to_string()).collect(),
        )),
        ParamKind::IfName => Some(Val::Raw(word.to_string())),
    }
}

/// 解析 VLAN 列表：逗号/空白分隔，支持 10-12 区间。
pub fn parse_vlan_list(s: &str) -> Option<Vec<u16>> {
    let mut out = vec![];
    for part in s.split([',', ' ', '\t']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let a: u16 = a.parse().ok()?;
            let b: u16 = b.parse().ok()?;
            if a == 0 || b < a || b > 4094 {
                return None;
            }
            out.extend(a..=b);
        } else {
            let v: u16 = part.parse().ok()?;
            if !(1..=4094).contains(&v) {
                return None;
            }
            out.push(v);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

enum MatchFail {
    Unknown,
}

/// 在当前层子节点中匹配一个 token。返回 (节点, 是否作为参数消费)。
fn match_token<'a>(kids: &'a [CNode], word: &str) -> Result<(&'a CNode, Option<Val>), MatchFail> {
    let mut exact: Option<&CNode> = None;
    let mut abbr: Vec<&CNode> = vec![];
    let mut param: Option<(&CNode, Val)> = None;
    for n in kids {
        match &n.tok {
            Tok::Lit(w) => {
                if w.eq_ignore_ascii_case(word) {
                    exact = Some(n);
                } else if word.len() < w.len() && w.to_lowercase().starts_with(&word.to_lowercase())
                {
                    abbr.push(n);
                }
            }
            Tok::Param { kind, .. } => {
                if param.is_none() {
                    if let Some(v) = parse_param(*kind, word) {
                        param = Some((n, v));
                    }
                }
            }
        }
    }
    if let Some(e) = exact {
        return Ok((e, None));
    }
    if abbr.len() == 1 {
        return Ok((abbr[0], None));
    }
    if abbr.len() > 1 {
        return Err(MatchFail::Unknown); // 缩写歧义
    }
    if let Some((n, v)) = param {
        return Ok((n, Some(v)));
    }
    Err(MatchFail::Unknown)
}

/// 节点是否带有必填参数子节点。
fn has_param_kids(node: &CNode) -> bool {
    node.kids.iter().any(|k| matches!(k.tok, Tok::Param { .. }))
}

/// 解析一行命令。空行返回 Ok(None)。
///
/// 结束判定规则：
/// 1. 末节点还有必填参数 → Incomplete
/// 2. 末节点有 run → 执行
/// 3. 末节点是参数叶子 → 继承最近的祖先 run 执行（`hostname R1` 的 R1 处）
/// 4. 其他 → Incomplete
pub fn parse(root: &[CNode], line: &str) -> Result<Option<Parsed>, CliError> {
    let toks = tokenize(line);
    if toks.is_empty() {
        return Ok(None);
    }
    let mut kids = root;
    let mut vals: Vec<(String, Val)> = vec![];
    let mut last_run: Option<CmdFn> = None;
    let mut path_had_run = false;
    let n_toks = toks.len();
    for (i, (word, start)) in toks.iter().enumerate() {
        let last = i + 1 == n_toks;
        match match_token(kids, word) {
            Ok((node, v)) => {
                if let (Some(v), Tok::Param { name, .. }) = (v, &node.tok) {
                    // VlanList / WordList 参数天然贪婪：吞掉行内剩余 token
                    let greedy = matches!(v, Val::VlanList(_) | Val::WordList(_)) && i + 1 < n_toks;
                    let v = if greedy {
                        let rest: Vec<String> =
                            toks[i + 1..].iter().map(|(w, _)| w.clone()).collect();
                        let joined = format!("{} {}", word, rest.join(" "));
                        let kind = match v {
                            Val::VlanList(_) => ParamKind::VlanList,
                            _ => ParamKind::WordList,
                        };
                        match parse_param(kind, &joined) {
                            Some(v2) => {
                                // 以贪婪消费后的位置作为结束
                                let end_char = toks
                                    .last()
                                    .map(|(w, st)| st + w.chars().count())
                                    .unwrap_or(*start);
                                let name2 = name.clone();
                                vals.push((name2, v2));
                                // 直接按"最后一段参数"收尾
                                let run = node.run.or(last_run);
                                return match run {
                                    Some(run) => Ok(Some(Parsed { run, vals })),
                                    None => Err(CliError::Incomplete { pos: end_char }),
                                };
                            }
                            None => v,
                        }
                    } else {
                        v
                    };
                    vals.push((name.clone(), v));
                }
                if last {
                    let end_pos = *start + word.chars().count();
                    if has_param_kids(node) {
                        return Err(CliError::Incomplete { pos: end_pos });
                    }
                    if let Some(run) = node.run {
                        return Ok(Some(Parsed { run, vals }));
                    }
                    if matches!(node.tok, Tok::Param { .. }) {
                        if let Some(run) = last_run {
                            return Ok(Some(Parsed { run, vals }));
                        }
                    }
                    return Err(CliError::Incomplete { pos: end_pos });
                }
                if node.run.is_some() {
                    last_run = node.run;
                    path_had_run = true;
                }
                kids = &node.kids;
            }
            Err(MatchFail::Unknown) => {
                // 命令路径上出现过完整命令 → 多余参数；否则无法识别
                let pos = *start;
                return if path_had_run {
                    Err(CliError::TooMany { pos })
                } else {
                    Err(CliError::Unknown { pos })
                };
            }
        }
    }
    unreachable!("循环内必返回")
}

// ---------- 帮助与补全 ----------

pub struct HelpList {
    /// (词/占位符, 帮助文本)
    pub items: Vec<(String, String)>,
    /// 该位置是否可以是完整命令（显示 <cr>）
    pub cr: bool,
}

/// 解析 `?` 位置：`?` 独立 token 或行尾粘着的 `word?`。
/// 返回该层可选项列表。
pub fn help_for(root: &[CNode], line: &str, dialect: Dialect) -> Option<HelpList> {
    let trimmed = line.trim_end();
    if !trimmed.contains('?') {
        return None;
    }
    let (before, glued) = match trimmed.rsplit_once('?') {
        Some((b, suffix)) => {
            if !suffix.is_empty() {
                return None; // '?' 后还有内容：不当作帮助
            }
            // 粘着形式：'?' 紧跟在非空白字符后（如 `conf?`）
            (
                b.to_string(),
                !b.is_empty() && !b.ends_with(char::is_whitespace),
            )
        }
        None => return None,
    };
    let (walk_line, prefix) = if glued {
        let b = before.trim_end();
        match b.rsplit_once(char::is_whitespace) {
            Some((head, last)) => (head.to_string(), Some(last.to_string())),
            None => (String::new(), Some(b.to_string())),
        }
    } else {
        (before.clone(), None)
    };

    let toks = tokenize(&walk_line);
    let mut kids = root;
    for (word, _) in &toks {
        match match_token(kids, word) {
            Ok((node, _)) => {
                kids = &node.kids;
            }
            Err(MatchFail::Unknown) => {
                return Some(HelpList {
                    items: vec![],
                    cr: false,
                });
            }
        }
    }
    let mut items = vec![];
    for n in kids {
        match &n.tok {
            Tok::Lit(w) => {
                if let Some(p) = &prefix {
                    if !w.to_lowercase().starts_with(&p.to_lowercase()) {
                        continue;
                    }
                }
                items.push((w.clone(), n.help.clone()));
            }
            Tok::Param { kind, .. } => {
                if prefix.is_none() {
                    items.push((placeholder::of(*kind, dialect).to_string(), n.help.clone()));
                }
            }
        }
    }
    // <cr>：当前层存在可直接结束的命令（有 run 且无必填参数）
    let cr = prefix.is_none() && kids.iter().any(|n| n.run.is_some() && !has_param_kids(n));
    Some(HelpList { items, cr })
}

/// Tab 补全：返回 (补全后的整行, 候选列表)。
pub fn complete(root: &[CNode], line: &str) -> (String, Vec<String>) {
    let ends_with_space = line.ends_with(char::is_whitespace);
    let toks = tokenize(line);
    let (last, walk_n) = if ends_with_space {
        (String::new(), toks.len())
    } else {
        match toks.last() {
            Some((w, _)) => (w.clone(), toks.len() - 1),
            None => return (line.to_string(), vec![]),
        }
    };
    // 前缀部分 = 行首到 last token 之前
    let head: String = if ends_with_space {
        line.to_string()
    } else {
        let start = toks[walk_n].1;
        line.chars().take(start).collect()
    };
    // 沿树下行
    let mut kids = root;
    for (word, _) in toks.iter().take(walk_n) {
        match match_token(kids, word) {
            Ok((node, _)) => kids = &node.kids,
            Err(MatchFail::Unknown) => return (line.to_string(), vec![]),
        }
    }
    let mut cands: Vec<&str> = vec![];
    for n in kids {
        if let Tok::Lit(w) = &n.tok {
            if last.is_empty()
                || (last.len() <= w.len() && w.to_lowercase().starts_with(&last.to_lowercase()))
            {
                cands.push(w);
            }
        }
    }
    if cands.is_empty() {
        return (line.to_string(), vec![]);
    }
    if cands.len() == 1 {
        return (format!("{}{} ", head, cands[0]), vec![cands[0].to_string()]);
    }
    let lowers: Vec<String> = cands.iter().map(|s| s.to_lowercase()).collect();
    let common = common_prefix(&lowers);
    if common.chars().count() > last.to_lowercase().chars().count() {
        let take = common.chars().count();
        let ext: String = cands[0].chars().take(take).collect();
        return (
            format!("{}{}", head, ext),
            cands.iter().map(|s| s.to_string()).collect(),
        );
    }
    (
        line.to_string(),
        cands.iter().map(|s| s.to_string()).collect(),
    )
}

fn common_prefix(words: &[String]) -> String {
    if words.is_empty() {
        return String::new();
    }
    let mut p = String::new();
    let first = &words[0];
    for (i, ch) in first.chars().enumerate() {
        if words.iter().all(|w| w.chars().nth(i) == Some(ch)) {
            p.push(ch);
        } else {
            break;
        }
    }
    p
}

pub mod placeholder {
    //! 参数占位符（`?` 帮助列表中显示）。
    use super::ParamKind;
    use sim_core::device::Dialect;

    pub fn of(kind: ParamKind, dialect: Dialect) -> &'static str {
        match (kind, dialect) {
            (ParamKind::Ipv4, Dialect::Ios) => "A.B.C.D",
            (ParamKind::Ipv4, _) => "<ip>",
            (ParamKind::DottedMask, Dialect::Ios) => "A.B.C.D",
            (ParamKind::DottedMask, _) => "<mask>",
            (ParamKind::Wildcard, Dialect::Ios) => "wildcard-bits",
            (ParamKind::Wildcard, _) => "<wildcard>",
            (ParamKind::PrefixLen, _) => "<0-32>",
            (ParamKind::Number, Dialect::Ios) => "<0-4294967295>",
            (ParamKind::Number, _) => "<0-4294967295>",
            (ParamKind::VlanList, Dialect::Ios) => "VLAN_LIST",
            (ParamKind::VlanList, _) => "<vlans>",
            (ParamKind::Word, Dialect::Ios) => "WORD",
            (ParamKind::Word, _) => "<name>",
            (ParamKind::WordList, Dialect::Ios) => "WORD...",
            (ParamKind::WordList, _) => "<word-list>",
            (ParamKind::IfName, Dialect::Ios) => "INTERFACE",
            (ParamKind::IfName, _) => "<interface>",
        }
    }
}
