# 网工模拟器

一个面向网络工程师的**浏览器端网络仿真学习平台**：纯前端运行（Rust→WebAssembly 内核 + TypeScript 界面），无需安装，部署在 GitHub Pages 上即可使用。

> 目标：覆盖 HCIA/HCIP/HCIE、CCNA/CCNP 等课程体系（数通 / 安全 / 无线 / 数据中心），对同一知识点**同时讲解思科、华为、华三的命令写法**，并提供从简单办公网到大型园区网、数据中心的在线练习场，附带笔记与进度追踪。

## 当前状态：M0 ✅ + M1 ✅ + M2（IA 补全，含 LACP/SSH/RSTP/MSTP）✅ + M3（课程/笔记/云同步/PWA）✅ + M3b（链路聚合）✅ + M4（OSPF/VRRP/IPv6/安全/QoS）✅ + M5（BGP/IS-IS/VRF/MPLS/route-policy/L3VPN + 园区实验）✅ + M6（防火墙/TCP/IPsec/SSL VPN）✅ + M7（AC/AP/CAPWAP/多 SSID/WPA3/漫游）✅ + M8（VXLAN/EVPN/堆叠/M-LAG/F5 LTM）✅

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

- **题库 v5**：228 道单选题，27 个主题——网络基础 23、OSPF/IS-IS/网络服务各 10，IPv4/静态路由/IPv6/WLAN/BGP/MPLS VPN/QoS/链路聚合与堆叠/IP 组播/网络可靠性/运维入门/园区规划/SD-WAN/智能运维/同 VLAN 隔离/OSPFv3/SRv6/自动化各 5，VLAN/STP/DHCP/ACL/NAT 各 17；原创题面 + 解析，顶栏「题库」进入，按主题或全部练习，即答即判 + 解析。
- **错题本**：答错自动入本，"错题重做"专项练习；统计页一键清空记录。
- **进度看板**：总体进度（已答/错题数/最近正确率）+ 分主题掌握度条形图；本地持久化。
- **实验进度徽章**：实验判分全过会在实验卡上打 ✓（本地记忆）。

### M3a 新增（课程 / 三厂商对照 / 笔记 / 云同步）

- **课程视图**：46 篇原创课程，按 **IA 初级（13）/ IP 中级（12）/ IE 高级（14）/ 厂商对照（7）** 分组——IA 覆盖 TCP/IP、IPv4、静态路由、OSPF 单区域、IPv6、网络服务/接入安全、WLAN、链路聚合/堆叠、运维与自动化入门；IP 覆盖 OSPF+VRRP、BGP、MPLS L3VPN、防火墙、IPsec、SSL VPN、无线、IS-IS 基础、BFD/可靠性、组播、OSPFv3、QoS；IE 覆盖 OSPF 多区域、BGP 选路/RR、IS-IS+SR、MPLS 跨域/TE、组播进阶、VXLAN/EVPN、堆叠/M-LAG、F5 LTM、园区综合、CloudCampus 规划、SD-WAN/CloudWAN、智能运维与排障、SRv6；厂商对照讲 EtherChannel/Eth-Trunk、StackWise/iStack/IRF、vPC/M-LAG/DRNI、HSRP/VRRP、PVST+/MSTP、VXLAN/EVPN、MUX VLAN/私有 VLAN/端口隔离。Markdown 阅读器（零依赖自研渲染器），frontmatter 关联实验——课程页一键跳进对应实验。
- **三厂商命令对照表**：单一数据源（`cmdmap.ts`）驱动，**每条命令在运行时经 wasm `verify_command` 校验**，表格标注 ✓ = 与模拟器行为一致（141 条对照全绿）。这是与 eNSP/Packet Tracer 的根本差异之一。
- **笔记**：Markdown 编辑/预览、自动保存本地。
- **Gist 云同步**：经典 PAT（gist 权限）备份/恢复笔记+题库记录+实验进度到私有 Gist；题库按答题次数合并、实验进度并集。Token 仅存本机。
- **导航**：练习场 / 题库 / 课程 / 笔记 四视图切换。

### M3b 新增（链路聚合 LACP）

- **逐包 LACPDU**：1 秒周期、3.5 秒老化；Active / Passive / Static 三种模式，成员 Selected/Standby 选举，成员故障后自动剔除并切换。
- **聚合口是逻辑端口**：出方向按流哈希在成员间分担，MAC 学习固定在逻辑端口；STP 只看到一条链路（代表端口），双链路聚合不会被误阻塞。
- **三方言命令**：IOS `channel-group … mode active|passive|on` + `interface Port-channel` + `show etherchannel summary`；VRP `interface Eth-Trunk` + `eth-trunk` + `mode lacp-static` + `display eth-trunk`；Comware `interface Bridge-Aggregation` + `port link-aggregation group` + `link-aggregation mode dynamic` + `display link-aggregation summary`。
- **实验五「链路聚合与带宽冗余」**：双链路聚合 → 抓包可见 LACPDU → 断一条成员业务不中断，自动判分 7 项。

### M2 收尾（SSH / Telnet 管理面）

- **本地认证**：三方言本地用户与权限（IOS `username ... privilege ... secret`、VRP `local-user ... password cipher`、Comware `local-user ... class manage` + `password simple`），口令明文仅存本机（教学用途）。
- **VTY 接入控制**：`line vty` / `user-interface vty` 视图配置本地认证与传输协议（`transport input` / `protocol inbound` 支持 ssh / telnet / all / none）。
- **远程登录会话**：设备与 PC 均可 `telnet <IP>` / `ssh <IP>`，逐行输入用户名/口令；认证通过后命令**转发到目标设备**（可 show、可改配置），`exit` 断开；SSH 权限 15 直接进入特权模式。
- **实验六「设备管理面加固」**：只开 SSH、关闭 Telnet，SSH 登录成功并在远端执行 show，Telnet 被拒绝，自动判分 4 项。

### M2 收尾（RSTP 快速收敛）

- **模式切换**：IOS `spanning-tree mode rapid-pvst`、VRP/Comware `stp mode rstp`（可切回 802.1D）。
- **快速收敛（简化实现）**：RSTP 模式下根端口/指定端口角色确定即进入转发，根端口故障时替换端口**立即**接替；802.1D 仍走 Listening/Learning 30 秒。`show spanning-tree` / `display stp brief` 显示当前模式。
- 说明：未模拟提案/同意 BPDU 标志与边缘端口，详见 ADR D16。

### M3c 新增（PWA 离线）

- **可安装 + 离线**：`vite-plugin-pwa` 生成 Service Worker，预缓存页面、WASM、课程与题库；断网刷新仍可打开练习场。自带 192/512 图标（暗色网络节点徽标）与 Web App Manifest。

### 体验增强（MSTP / i18n / 拓扑分享）

- **MSTP 多实例**：`spanning-tree mode mst` / `stp mode mstp`，`spanning-tree mst 1 priority 4096` / `stp instance 1 priority 4096`，MST 域视图配置 `instance 1 vlan 10` 映射；每个实例独立发送 BPDU、独立选根，数据面按 VLAN 归属实例转发（同一端口可在不同 VLAN 分别转发/阻塞，实现负载分担）。`show spanning-tree` / `display stp brief` 按实例展示。
  说明：不做 MST 域协商（name/revision 仅接受），详见 ADR D17。
- **界面双语**：顶栏「EN / 中」一键切换，界面骨架、练习场、题库/课程/笔记的操作文案均为中英双语；46 篇课程与题库内容也提供英文译文。
- **URL 分享拓扑**：拓扑管理面板「复制分享链接」把工作区（含设备位置）压缩进 `#topo=`，打开链接即还原；同时补上导出/导入文件入口。分享链接优先于本地工作区。

### M4（IP 进阶与二层安全）

- **OSPF 单区域**：Hello（2s）建立邻居、Router-LSA 泛洪、SPF 计算动态路由（`O` 路由）；三方言配置（`router ospf 1` + `network … area 0` / `ospf 1` + `area 0` + `network`），`show/display ospf neighbor|peer / database|lsdb`。
- **VRRP 主备**：优先级选举（平局比接口 IP）、1s 通告、3.5s 失效切换、虚拟 MAC 网关、Master 应答 VIP 的 ARP 与 ICMP；三方言配置与 `show/display vrrp`。
- **IPv6**：接口 IPv6 地址、NDP（NS/NA + solicited-node 组播）、ICMPv6 ping6、连接/静态 v6 路由；三方言 `ipv6 address`、`ipv6 route`/`route-static`、`ping ipv6`、`show/display ipv6 interface brief|routing-table`；主机属性面板可直接配置 IPv6 地址/网关，PC 终端 `ping6`。
- **DHCP Snooping**：全局开关 + 信任端口；非信任口的 Offer/Ack 被丢弃，从 Request/Ack 学习绑定表（MAC/IP/VLAN/端口），`show ip dhcp snooping binding` / `display dhcp snooping user-binding`。
- **端口安全**：最大 MAC 数、`restrict` / `shutdown` 违规动作、sticky 标记；超限帧丢弃与违规计数，`show/display port-security`。
- **QoS 基础**：入方向广播风暴抑制（每秒广播上限）与令牌桶限速（kbps），显示丢弃/抑制计数；`storm-control` / `broadcast-suppression`、`rate-limit input` / `qos car|lr inbound cir`、`show/display storm-control|qos`。
- **英文内容**：46 篇课程、228 道题库、22 个实验的文案均已有英文译文，切 EN 后内容与界面一起切换。

### M5（BGP / IS-IS / VRF / MPLS / route-policy / L3VPN）

- **eBGP 单跳会话**：Open／Keepalive 建立与保活、广播 Update 泛洪、AS_PATH 环路检测、按下一跳安装 `B` 路由（来源优先级：直连 > 静态 > BGP > OSPF）；邻居 5s 无包自动断开并撤销路由。
- **IS-IS 单区域**：Hello 建邻、LSP 泛洪、SPF，安装 `i` 路由（优先级最低，115）；三方言 `router isis`/`isis 1` + `net`/`network-entity` + 接口使能，`show/display isis peer|lsdb`。
- **VRF-lite**：每 VRF 独立连接/静态路由表与转发上下文，接口按端口归属 VRF；三方言 `vrf definition`/`ip vpn-instance`、接口绑定、`show/display ip route vrf|vpn-instance`；跨 VRF 隔离。
- **MPLS-lite**：静态 LSP 的压签/换签/弹签，标签帧按 ILM 转发；三方言 `static-lsp ingress|transit|egress` 与 `show/display mpls lsp`。
- **route-policy**：按 ACL 前缀匹配的路由策略（IOS `route-map` + `match ip address` + `set local-preference`；VRP/Comware `route-policy … node` + `if-match acl` + `apply local-preference`），绑定到 BGP 邻居的 import/export 方向（`neighbor … route-map … in|out` / `peer … route-policy … import|export`）；策略按 seq 升序首条命中决定 permit/deny，无命中隐式拒绝；Local Preference 参与 BGP 最优路径选择（高者优先，其次 AS_PATH 短者）；`show route-map` / `display route-policy` 从状态渲染。
- **MPLS L3VPN-lite**：PE 上 VRF 隔离客户路由，入口按 VRF + FEC 压入“传输标签 + VPN 标签”（`static-lsp ingress … vrf CUST1 vpn-label 1001`），P 只交换外层标签，出口 PE 弹外层后按 VPN 标签把报文投递回 VRF（`static-lsp egress … vrf CUST1`）；支持 CE—PE—P—PE—CE 双向往返。
- **实验七「BGP 路由策略」**：R1 宣告两个网段，R2 用 ACL + route-policy 只过滤受控前缀；判分验证“允许前缀可达、受控前缀不可达”。
- **实验八「MPLS L3VPN」**：CE—PE—P—PE—CE 拓扑，配置双标签 VPN 与 VRF 映射，判分验证 LSP 与双向 ping。
- **实验九「大型园区综合实验」**：双楼接入（VLAN + LACP 双链路）→ DSW SVI 间路由 → CORE1/CORE2 VRRP + OSPF → CORE1 NAT 上外网；一次串起 M2~M5。
- 简化边界见 ADR D21/D22/D23/D24（无 TCP/iBGP、IS-IS 无 DIS/多级、VRF 无动态协议、L3VPN 无 MP-BGP/LDP、route-policy 仅 ACL 匹配与 Local Preference）。

**已验证**：118 个 Rust 测试全绿；浏览器实测 BGP Established + `B` 路由、IS-IS 邻居 Up + LSDB、VRF 路由表隔离、MPLS 静态 LSP 展示、实验七 5/5、实验八 4/4、实验九 8/8（含 VRRP Master、OSPF 邻居、LACP、跨 VLAN 与 NAT 上网）。

### M6（防火墙 / TCP 状态机 / IPsec / SSL VPN）

- **防火墙设备**：拓扑编辑器新增 Firewall 类型（复用路由器数据面），独立图标与颜色；支持接口 IP、静态路由、NAT、ACL、ping 等路由器能力。
- **安全区域**：VRP `firewall zone trust` + `add interface`，Comware `security-zone name trust` + `import interface`，IOS `zone security INSIDE` + 接口 `zone-member security INSIDE`；`display firewall zone` / `display security-zone` / `show zone security` 从端口状态渲染。
- **区域间策略**：VRP/Comware `security-policy` → `rule name` → `source-zone` / `destination-zone` / `action permit|deny`；IOS 简化 ZBF `zone-pair security` → `source` / `destination` / `action`。按规则顺序首条匹配，无命中默认拒绝；同区域/未划分区域放行。
- **状态化放行**：策略 permit 的反向 ICMP 回包由 30 秒会话表自动放行，无需额外配置回程规则。
- **ASA 安全级别模型**：IOS 防火墙接口支持 `nameif inside` / `security-level 100`；默认高→低放行、低→高拒绝，回包走状态会话。
- **TCP 状态机**：新增 TCP 段（SYN/ACK/FIN/RST）与 PC `tcp <IP> <端口>` 探测；主机监听 22/80/443，完成 SYN → SYN-ACK → ACK → FIN 挥手；防火墙维护五元组状态表（SynSent/Established/FinWait/Closed）。
- **IPsec 站点隧道**：`ipsec tunnel NAME local A remote B` + `local-subnet`/`remote-subnet`，命中保护子网的流量以 IP-in-IP 封装到对端并在出口解封装；三方言 `show/display ipsec tunnel`。
- **SSL VPN 远程接入**：防火墙 `sslvpn virtual-ip` 地址池 + 本地用户认证；PC `sslvpn connect <网关> <用户> <口令>` 获取虚拟 IP，访问内网流量封装到网关、回程再封装回客户端；`display sslvpn session`。
- **实验十「防火墙安全区域与策略」**：trust/untrust 划分 + trust→untrust permit；判分验证区域、策略、外网主动访问被拒绝与内网访问状态化放行。
- **实验十一「IPsec 站点到站点 VPN」**：双站点隧道，判分验证两端隧道与双向 ping。
- **实验十二「SSL VPN 远程接入」**：外网 PC 认证获取虚拟 IP 后访问内网，判分验证会话与内网连通。
- 简化边界见 ADR D25/D26（无 ESP 加密/IKE、无 TCP 应用数据、SSL VPN 仅 ICMP 隧道）。

**已验证**：128 个 Rust 测试全绿；实验十 5/5、实验十一 4/4、实验十二 2/2；TCP 握手/拒绝、ASA 默认策略、IPsec 隧道、SSL VPN 隧道均有端到端测试。

### M7（无线：AC / AP / CAPWAP / 多 SSID / WPA3 / 漫游）

- **AC/AP 设备**：拓扑编辑器新增 AccessPoint / WlanController，独立图标与颜色；AP 复用二层桥接状态，AC 复用路由器状态。
- **CAPWAP**：AP 配置 `capwap ac <IP>` 后发送 Join，AC 回 Config 下发 SSID/安全/VLAN；AP 应用后把非 Trunk 端口划入 WLAN 业务 VLAN。
- **WLAN 安全**：AC `wlan ssid CORP`、`wlan security wpa2 psk <口令>`、`wlan vlan 10`；PC `wifi connect <SSID> [口令]` / `wifi status`，WPA2 口令错误拒绝关联；未关联终端帧在 AP 入口被丢弃。
- **展示**：AC/AP `show/display wlan`（SSID、安全、VLAN、AP 列表/Joined 状态）。
- **实验十三「无线园区」**：AC—SW—AP—无线 PC + 有线服务器；判分验证 AP 注册、SSID、PC 关联、有线连通与未关联终端被丢弃。
- **多 SSID**：AC 可配置多个 `wlan ssid`（各自安全/VLAN），CAPWAP 以 ConfigReset+Config 批量下发；客户端按 SSID 关联，AP 为每个客户端端口划分对应业务 VLAN，不同 SSID 默认隔离。
- **WPA3-SAE**：`wlan security wpa3 sae <口令>`；客户端口令错误返回 WPA3 失败，与 WPA2 分开提示。
- **无线漫游**：`wlan priority` 设置 AP 选路优先级；PC `wifi roam` 在同 SSID 的另一 AP 间切换（动态重关联、业务不中断）。
- **实验十四「多 SSID / WPA3 / 漫游」**：CORP(WPA2/VLAN10) + GUEST(WPA3/VLAN20)、双 AP、PC1 漫游到 AP2；判分 9 项。
- 简化边界见 ADR D27/D28（无 802.11 空口/信道/射频、CAPWAP 仅二层、WPA3 为口令校验而非完整 SAE、漫游为显式/重关联）。

**已验证**：131 个 Rust 测试全绿；实验十三 5/5、实验十四 9/9。

### M8（VXLAN / EVPN / 堆叠 / IRF / M-LAG / F5 LTM）

- **VTEP/VNI**：三层交换机作为 VTEP，`vxlan local <IP>`、`vxlan peer <IP>`、`vxlan vni <VNI> vlan <VLAN>`；`show/display vxlan` 展示本端 VTEP、对端与 VNI 映射。
- **数据面**：本地 VLAN 的广播/未知单播按 VNI 封装为 VXLAN（外层 IP + 内层以太帧），对端 VTEP 解封装后按 VLAN 注入本地端口；同网段主机跨 DC 二层互通，抓包可见外层 VXLAN 帧。
- **EVPN 控制面**：`evpn enable` + `evpn peer`，VTEP 周期性通告 VNI/VLAN 路由（简化 Type-3），对端自动学习远端 VTEP 并加入 `vxlan.peers`；`display evpn` 显示 EVPN 路由。实验十八验证不配静态 peer 也能跨 DC 通。
- **堆叠/IRF/CSS**：`stack domain` + `stack member` + `stack priority`；同 domain 成员间链路视为内部 fabric，不参与 STP；一台逻辑设备跨成员二层转发，`display stack` 显示 Master/成员。
- **M-LAG**：`mlag domain` + 接口 `mlag peer-link`；两台交换机共享系统 MAC（跨设备 LACP 呈现同一对端），peer-link 作为内部链路并做水平分割，接入交换机可 LACP 双归两台成员，双链路同时转发。
- **F5 LTM**：新增 LoadBalancer 设备，`ltm pool NAME member IP PORT` 与 `ltm virtual NAME vip IP PORT pool NAME`；TCP 连接按轮询选择池成员，目的地址改为成员、回程源地址改回 VIP；`show/display ltm` 显示虚拟服务器/池/会话。
- **实验十五「VXLAN 数据中心跨 DC 二层互通」**、**实验十六「堆叠/IRF」**、**实验十七「M-LAG」**、**实验十八「EVPN 控制面」**、**实验十九「F5 LTM」**、**实验二十「MUX VLAN 与端口隔离」**、**实验二十一「OSPFv3」**、**实验二十二「SRv6 BE」**。
- 简化边界见 ADR D29/D30/D31（静态/简化 EVPN Type-3、无真实 BGP EVPN 属性/RT、堆叠/M-LAG 为共享 MAC+fabric、LTM 无健康检查/持久化/SSL 卸载）。

**已验证**：139 个 Rust 测试全绿；实验十五 3/3、实验十六 3/3、实验十七 4/4、实验十八 3/3、实验十九 3/3、实验二十 5/5、实验二十一 3/3、实验二十二 3/3。

### M9 补充（同 VLAN 隔离 / OSPFv3 / SRv6）

- **MUX VLAN / 私有 VLAN / 端口隔离**：端口角色 `principal / separate / group N` 与隔离组 `port-isolate enable group N`；数据面在同一 VLAN 内按角色/组阻断二层互通，`display mux-vlan` / `display port-isolate` 展示配置；实验二十验证 separate 与 group 隔离、principal 可达。
- **OSPFv3**：`ospfv3 1` + `router-id` + 接口 `ospfv3 1 area 0`；邻居交换 IPv6 前缀并安装 `O` 路由，`display ospfv3 peer` / `show ipv6 ospf neighbor`；实验二十一验证跨路由器 ping6。
- **SRv6 BE**：`srv6 locator` / `srv6 sid` / `srv6 route ... sid`；入口 IPv6-in-IPv6 封装到远端 SID，出口解封装，`display srv6` 展示 Locator/SID/路由；实验二十二验证跨网段 ping6。
- 实验二十/二十一/二十二已加入实验列表；拓扑变化后自动重算并启动 STP 定时器，判分前等待 STP 收敛，避免 Learning 状态误判。
- 简化边界见 ADR D32（MUX VLAN 不做跨 VLAN 桥接、OSPFv3 仅直连邻居前缀学习、SRv6 仅 BE 无 SRH/Policy/TI-LFA）。

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

## 与华为 Datacom 认证大纲的对照

- **HCIA-Datacom V2.0（H12-811）**：网络基础、IP 网络、以太交换、安全接入、网络服务、WLAN、WAN、运维、IPv6、SDN/自动化。IA 的 13 篇课程覆盖 TCP/IP、IPv4/子网、静态路由、OSPF 单区域、IPv6、网络服务/接入安全、WLAN、链路聚合/堆叠、运维与自动化入门；**WAN 基础（PPP/PPPoE）** 课程未单列、模拟器也未建模。
- **HCIP-Datacom Core（H12-821）**：OSPF/IS-IS/BGP、交换（VLAN/STP/堆叠）、IPv6、BFD/VRRP、组播、QoS、MPLS VPN、运维。IP 的 12 篇课程覆盖 OSPF/VRRP、OSPFv3、BGP、IS-IS 基础、BFD/可靠性、组播、QoS、MPLS VPN、安全与 WLAN；**MUX VLAN / 端口隔离** 已建模（端口角色 + 隔离组，同一 VLAN 内生效；不做 MUX VLAN 的跨 VLAN 二层桥接）。
- **HCIE-Datacom V2.0（H12-891）**：园区 43%、广域 33%、自动化 10%、智能运维 14%。IE 的 40/41/42/45 分别对应园区规划/CloudCampus、SD-WAN/CloudWAN、智能运维与结构化排障、SRv6；EVPN、MPLS 跨域、自动化由 IE 12/31/33 承接。
- **已知缺口（下一批）**：
  - WAN 基础（PPP/PPPoE）：课程与模拟器均未实现；
  - MUX VLAN：已做同 VLAN 端口隔离；真正的跨 VLAN primary/subordinate 桥接未建模；
  - 大型 WLAN 组网：基础/漫游已有，信道/射频与大规模容量规划未建模；
  - SRv6 Policy/网络切片：BE 已建模；多段 Segment List、TI-LFA、切片未建模。

## 路线图

| 里程碑 | 内容 | 状态 |
|---|---|---|
| M0 骨架 | 逐包转发闭环 + 拓扑编辑器 + 三方言最小命令集 + CI/Pages 部署 | ✅ |
| M1 数通IA闭环（MVP） | VLAN/Trunk/SVI/静态路由（三方言）+ 校验引擎 + 报文查看器 + 办公网实验集 | ✅ |
| M2 IA 补全 | STP ✅、RSTP ✅、MSTP ✅、DHCP ✅、ACL ✅、NAT ✅、题库/进度 ✅、LACP ✅、SSH ✅ | ✅ |
| M3 内容+笔记+同步 | 课程管线 ✅、三厂商对照表 ✅、Gist 同步 ✅、PWA 离线 ✅ | ✅ |
| M4 IP 进阶 | OSPF 单区域 ✅（多区域可选）、VRRP ✅、IPv6 ✅、DHCP Snooping ✅、端口安全 ✅、QoS 基础 ✅ | ✅ |
| M5 IE 数通 | BGP ✅（eBGP 单跳）、IS-IS ✅（单区域）、VRF ✅（静态）、MPLS ✅（静态 LSP）、route-policy ✅、MPLS L3VPN ✅、大型园区实验 ✅ | ✅ |
| M6 安全方向 | 防火墙设备/安全区域/区域策略 ✅、ASA 安全级别 ✅、TCP 状态机 ✅、IPsec 站点 VPN ✅、SSL VPN ✅ | ✅ |
| M7 无线方向 | AC/AP ✅、CAPWAP ✅、多 SSID ✅、WPA2/WPA3 ✅、无线 VLAN 隔离 ✅、漫游 ✅、实验十三/十四 ✅；信道/射频/802.1X/AP 冗余 | ✅ |
| M8 数据中心 | VXLAN ✅、EVPN ✅、堆叠/IRF ✅、M-LAG ✅、F5 LTM ✅、实验十五~十九 ✅ | ✅ |
| M9 打磨 | 大拓扑 Canvas 渲染、英文界面与内容（界面+课程/题库/实验文案 ✅）、MUX VLAN/端口隔离 ✅、OSPFv3 ✅、SRv6 BE ✅、社区内容贡献管线 | 进行中 |

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
