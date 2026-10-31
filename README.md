# 网工模拟器

一个面向网络工程师的**浏览器端网络仿真学习平台**：纯前端运行（Rust→WebAssembly 内核 + TypeScript 界面），无需安装，部署在 GitHub Pages 上即可使用。

> 目标：覆盖 HCIA/HCIP/HCIE、CCNA/CCNP 等课程体系（数通 / 安全 / 无线 / 数据中心），对同一知识点**同时讲解思科、华为、华三的命令写法**，并提供从简单办公网到大型园区网、数据中心的在线练习场，附带笔记与进度追踪。

## 当前状态：M0 ✅ + M1 ✅ + M2（IA 补全）✅ + M3a（课程/笔记/云同步）✅

**M0 = 可运行的骨架**：

- **逐包级仿真内核**（Rust）：虚拟时钟 + 事件堆，以太网帧逐跳处理——MAC 学习、VLAN 就绪的帧模型、ARP 请求/应答、ICMP echo、最长前缀路由查表、TTL 递减、带宽串行化时延与传播时延。
- **三方言 CLI**（思科 IOS / 华为 VRP / 华三 Comware）：厂商中立的状态内核 + 各自独立的命令树，支持 `?` 上下文帮助、Tab 补全、命令前缀缩写（`conf t`、`int g0/0`、`sys`），错误提示还原各家风格（IOS 的 `^` 插入符 / VRP 的 `Error: ...` / Comware 的 `% ...`）。`show` / `display` 输出全部从模拟状态实时渲染。
- **拓扑编辑器**：拖放设备（路由器/交换机/PC）、连线、平移缩放、状态点（端口 up 联动链路颜色）。
- **多标签终端**（xterm.js）：每设备独立会话与模式栈，支持历史（↑/↓）、补全、Ctrl+C、`?` 即时帮助、粘贴。
- **PC 迷你 shell**：`ping` / `ipconfig` / `arp`（Linux 风格输出，TTL 逐跳可见）。
- **入门任务卡**：加载"PC1—R1—R2—PC2"示例拓扑，按任务配置接口 IP 与静态路由，一键自动判分（lab-checks 检查引擎）。
- **持久化**：localStorage 自动保存 + 文件导出/导入，刷新页面完整恢复。

### M1 新增（数通 IA 闭环）

- **VLAN / Access / Trunk / SVI**（三层交换机）：按 (VLAN, MAC) 学习、入方向归类、出方向加/剥 802.1Q tag、Trunk 放行列表（思科默认全放行 / 华为华三默认仅 VLAN1 的方言差异保留）、SVI 网关与 VLAN 间路由（跨 SVI TTL 递减可见）。
- **三方言 VLAN 命令**：`vlan / vlan batch`、`switchport mode access|trunk / switchport access vlan / switchport trunk allowed vlan`、`port link-type / port default vlan / port trunk allow-pass|permit`、`interface Vlan(N) / Vlanif(N) / vlan-interface(N)`；`show vlan brief / display vlan`、`show mac address-table / display mac-address`（MAC 格式按方言：`aabb.ccdd.eeff` vs `xxxx-xxxx-xxxx`）。
- **报文查看器**：选中任意链路开启抓包，逐帧列出（ARP 请求/应答、Echo 请求/应答、VLAN tag、TTL），环形缓冲 500 帧。
- **两个入门实验**（含任务卡 + 自动判分 + 一键重置）：实验一"二层办公网 VLAN 隔离"（跨 Trunk 同 VLAN 互通 + 跨 VLAN 隔离）、实验二"三层交换机 VLAN 间路由（SVI）"。

### M2a 新增（STP 二层可靠性）

- **802.1D STP 逐包仿真**：配置 BPDU 每 2 秒泛洪、根桥选举（优先级+MAC）、根端口/指定端口/替换端口角色计算、Listening→Learning→Forwarding 状态机（15s Forward Delay）、**Max Age 20s 邻居老化**、根端口变化触发全网 MAC 冲刷（TCN 近似）。阻塞端口不转数据但照常收 BPDU——真实语义。
- **广播风暴演示**：两台交换机环路 + `stp disable` → ARP 风暴，事件数护栏（20 万/次驱动）保证页面不卡，状态栏丢弃数飙升。
- **三方言 STP 命令**：IOS `spanning-tree vlan 1 priority / cost`、VRP/Comware `stp priority / enable|disable / cost`；`show spanning-tree`（IOS 风格 Root ID/Bridge ID 块+接口表）、`display stp brief`（华为/华三风格）。
- **实验三「冗余链路与 STP」**：环路拓扑 → 配 Trunk/改根桥 → 抓包看 BPDU → shutdown 主链路 → max-age 老化 + 重新收敛 → 备份链路 5/5 通。

### M2b 新增（DHCP / ACL / NAT）

- **DHCP**：DORA 四步逐包交互（Discover 广播 → Offer → Request → Ack），路由器地址池（按接口网段匹配、租约表防冲突），主机侧"DHCP 自动获取"开关与 `ipconfig /renew`；三方言池配置（IOS `ip dhcp pool` 子模式、VRP `ip pool`、Comware `dhcp server ip-pool`）。
- **ACL**：基础/高级编号 ACL（1-99 / 100-199，VRP/Comware 2000/3000 段）、通配符掩码（支持 `0` 主机缩写）、首条命中+隐式拒绝、命中计数（`display acl all` 可见 matched 次数）、接口 in/out 应用（IOS `ip access-group`、VRP `traffic-filter`、Comware 风格）。
- **NAT**：Easy-IP PAT（ICMP 会话级转换表）与静态 NAT，内外接口标记，出向改写/入向还原（还原优先于本机交付）；`show ip nat translations` / `display nat session`。
- **实验四「企业上网」**：DHCP 自动获取 → NAT 上外网（ping 公网服务器 5/5、转换表可见）——IA 综合大实验。

### M2c 新增（题库 / 错题本 / 进度看板）

- **题库 v1**：30 道 IA 单选题（网络基础/VLAN/STP/DHCP/ACL/NAT 六主题，原创题面+解析），顶栏「题库」进入，按主题或全部练习，即答即判 + 解析。
- **错题本**：答错自动入本，"错题重做"专项练习；统计页一键清空记录。
- **进度看板**：总体进度（已答/错题数/最近正确率）+ 分主题掌握度条形图；本地持久化。
- **实验进度徽章**：实验判分全过会在实验卡上打 ✓（本地记忆）。

### M3a 新增（课程 / 三厂商对照 / 笔记 / 云同步）

- **课程视图**：4 篇原创课程（VLAN/STP/DHCP/ACL+NAT），Markdown 阅读器（零依赖自研渲染器），frontmatter 关联实验——课程页一键跳进对应实验。
- **三厂商命令对照表**：单一数据源（`cmdmap.ts`）驱动，**每条命令在运行时经 wasm `verify_command` 校验**，表格标注 ✓ = 与模拟器行为一致（29 条对照全绿）。这是与 eNSP/Packet Tracer 的根本差异之一。
- **笔记**：Markdown 编辑/预览、自动保存本地。
- **Gist 云同步**：经典 PAT（gist 权限）备份/恢复笔记+题库记录+实验进度到私有 Gist；题库按答题次数合并、实验进度并集。Token 仅存本机。
- **导航**：练习场 / 题库 / 课程 / 笔记 四视图切换。

**已验证**：48 个 Rust 测试全绿；浏览器端实测：四课对照表 54/54 全 ✓、笔记新建/编辑/渲染、四个实验判分全过。

## 本地开发

```bash
# 1. 构建 WASM 引擎（需要 rustup + wasm32-unknown-unknown target + wasm-pack）
wasm-pack build crates/sim-wasm --target web --out-name net_sim --out-dir ../../web/src/wasm

# 2. 前端
cd web
pnpm install
pnpm dev        # 开发（http://localhost:5173）
pnpm build      # 产物在 web/dist

# 3. Rust 测试
cargo test --workspace
```

## 仓库结构

```
crates/
├── sim-packets/    # 分层报文模型（typed headers，非字节流）
├── sim-core/       # 拓扑/设备模型、事件引擎、逐包转发管线、ARP/ICMP/路由
├── cli-engine/     # 命令树框架：解析/缩写/?帮助/Tab补全/错误定位
├── cli-dialects/   # IOS / VRP / Comware 三方言命令树与输出渲染
├── lab-checks/     # 实验声明式校验（ping/路由/接口/VLAN/SVI/Trunk…）
└── sim-wasm/       # wasm-bindgen 桥（拓扑 JSON / 命令执行 / 统计）
web/
├── src/components/ # TopoCanvas / LeftPanel / TerminalPanel / StatusBar
├── src/lib/        # sim.ts（桥类型）/ store.svelte.ts（状态）/ persistence.ts
└── src/wasm/       # wasm-pack 产物（构建时生成，不入库）
```

核心设计原则：**设备状态厂商中立，CLI 方言只是"前端"**。同一拓扑可随时切换思科/华为/华三方言练习——这是与 eNSP / Packet Tracer / HCL 的根本差异，也让"三厂商命令对照表"未来能直接从命令树元数据生成，与模拟器行为永远一致。

## 路线图

| 里程碑 | 内容 | 状态 |
|---|---|---|
| M0 骨架 | 逐包转发闭环 + 拓扑编辑器 + 三方言最小命令集 + CI/Pages 部署 | ✅ |
| M1 数通IA闭环（MVP） | VLAN/Trunk/SVI/静态路由（三方言）+ 校验引擎 + 报文查看器 + 办公网实验集 | ✅ |
| M2 IA 补全 | STP ✅、DHCP ✅、ACL ✅、NAT ✅、题库/进度 ✅、LACP、SSH | 进行中 |
| M3 内容+笔记+同步 | 课程管线、三厂商对照表组件、Gist 同步（经典 PAT）、PWA 离线 | |
| M4 IP 进阶 | OSPF 多区域、VRRP、DHCP Snooping、端口安全、IPv6、QoS 基础 | |
| M5 IE 数通 | BGP、IS-IS、route-policy、VRF、MPLS L3VPN + 大型园区实验 | |
| M6 安全方向 | 防火墙三方言（ASA/USG/SecPath）、区域/策略、IPSec/SSL VPN | |
| M7 无线方向 | AC/AP、CAPWAP、WLAN 安全 | |
| M8 数据中心 | 堆叠/IRF/CSS、M-LAG、VXLAN EVPN + DC 实验；F5 LTM 基础 | |
| M9 打磨 | 大拓扑 Canvas 渲染、英文界面、社区内容贡献管线 | |

## 部署（GitHub Pages）

推送到 GitHub 的 `main` 分支后，`deploy.yml` 会自动：构建 WASM → 构建前端 → 发布到 Pages。需要在仓库 Settings → Pages 中把 Source 设为 **GitHub Actions**。

```bash
git init   # 已执行
git remote add origin git@github.com:<你的用户名>/<仓库名>.git
git add -A && git commit -m "M0: 逐包仿真内核 + 三方言 CLI + 拓扑编辑器"
git push -u origin main
```

## 合规说明

本项目所有课程内容与帮助文本均为原创，仅以"描述性使用"提及各厂商命令语法；不嵌入、不分发任何厂商设备镜像或固件，不使用厂商 Logo。

## License

MIT
