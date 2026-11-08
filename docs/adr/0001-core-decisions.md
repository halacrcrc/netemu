# ADR 0001：核心架构决策

日期：2026-10-30 · 状态：已接受

## 背景

浏览器端网工模拟器，目标是课程学习 + 多厂商命令对照 + 在线练习场。需要先固定地基性决策，避免后续返工。

## 决策

### D1 混合架构：Rust 内核 + TypeScript UI

协议/CLI/仿真全部 Rust→WASM；拓扑编辑器、终端、笔记等 UI 用 TypeScript（Svelte 5）。理由：UI 迭代速度与组件生态（xterm.js 等）价值远大于"全 Rust"的纯度；Rust 侧保持纯逻辑，便于单元/黄金输出测试。

### D2 逐包级仿真（Packet-level）

不做 GNS3 式镜像模拟（版权 + 浏览器不可行），也不止步于控制面仿真：每个以太网帧逐跳处理，带虚拟时钟、串行化/传播时延、TTL。报文用 typed 结构体流转（非字节流），抓包视图延迟"解码"。性能护栏：在途报文上限（2 万）、每次命令驱动至静默（12s 虚拟视野）。

### D3 厂商中立状态内核 + 方言命令树

设备状态（接口/ARP/RIB/MAC 表）不感知方言；cli-dialects 为每家维护一棵命令树，共享 cmd 处理器。换来：同一拓扑切换三种方言练习、`show` 输出从状态渲染、未来"三厂商命令对照表"由命令树元数据自动生成（单一事实来源）。

### D4 云同步用经典 PAT（M3）

GitHub Pages 纯静态，OAuth 设备码流程受 CORS 限制。MVP 采用用户自建经典 PAT（细粒度 PAT 不支持 gist）粘贴本地保存，直连 api.github.com（该 REST API 支持 CORS）。文件导入导出作为永久兜底。

### D5 里程碑纪律

MVP = M1（数通 IA 三方言闭环）。M0 只做可运行骨架；每个里程碑交付"能玩"的东西，防止范围爆炸。课程内容与实验用声明式文件（MD/TOML）承载，为社区贡献留口。

### D6（M1 补充）交换机即三层交换机

`Switch` 设备同时承载二层（VLAN/MAC 表）与三层（SVI/静态路由）行为，与真实 S 系列交换机一致。让"VLAN 间路由"无需引入新设备类型；纯 L2 场景不配 SVI 即可。Trunk 放行的方言默认差异（思科全放行 vs 华为/华三仅 VLAN1）放在方言命令处理器里初始化，状态内核只存显式列表。

### D7（M1 补充）跨模块 UI 状态修改必须经 store 函数

Svelte 5 模块级 `$state` 对象在其他模块直接赋值属性不触发界面更新（实践验证）。约定：`store.svelte.ts` 暴露 `setXxx()` 函数，闭包内修改；外部模块（labs.ts 等）一律调用函数而非直接赋值。

### D8（M2a 补充）STP 采用真实计时器而非快进

STP 用 802.1D 真实常量（hello 2s / forward delay 15s / max age 20s），虚拟时钟随命令驱动自然流逝——用户能看到收敛的真实节奏（改根桥后 30s 才恢复转发），这是教学价值的一部分。配套两道护栏：STP 活跃时静默视野提升到 60s（`drain_horizon()` 自适应），单次驱动事件数上限 20 万（广播风暴场景保命）。M2a 实现 802.1D 而非 RSTP：状态机更简单、教学更经典；RSTP 的 proposal/agreement 可在后续作为可选模式。

### D9（M2a 补充）跨模块响应性铁律

Svelte 5 模块级 `$state` 的属性赋值必须发生在定义模块内部（函数闭包）才触发更新。`store.svelte.ts` 对外只暴露 `setXxx()` 函数；labs.ts 等外部模块禁止直接 `app.x = y`。这条在 M1 排障中用二分探针证实，M2a 起作为硬约定。

### D10（M2b 补充）UDP 类协议直接承载于 L3

DHCP 不实现通用 UDP/IP 栈，报文直接作为 `L3::Dhcp` 载荷（目的以受限广播 255.255.255.255 识别）。教学语义完全保留（DORA 逐包可见、抓包可看），实现成本大幅降低；后续需要真实 UDP 时再引入传输层。

### D11（M2b 补充）CLI 参数的"贪心字面量"陷阱

命令树里同一位置既有字面量又有 Word 参数时（如 `acl [basic|advanced] <n>`），Word 会吞掉数字 token。约定：此类位置用专用参数类型（Wildcard 收整数缩写），或处理器里做兜底解析（从 Word 中 parse 数字）。另：`match_token` 的 exact 匹配必须取**第一个**同名节点——重复字面量曾让 `show ip route` 落到后建的 `ip` 节点上。

### D12（M3a 补充）对照表数据驱动 + 运行时验证

三厂商命令对照表不是手写表格，而是 `cmdmap.ts` 数据 + wasm `verify_command`（对每条命令在对应方言/模式下跑一次纯解析）。表与模拟器永远一致，解析不过的条目直接标 ⚠ 暴露。教训：命令树构建器 `.run().kid()` 链式语义中 run 返回 Self——`.run(x).kid(y)` 会把 y 挂到 run 所在节点；含参数链必须写成 `.kid(param.run(x).kid(y))`。此类错误靠对照表的 ⚠ + parse 探针定位。

**配置查看命令同源渲染**（M9n）：`show running-config`（IOS）与 `display current-configuration`（VRP/Comware）由 `config_render.rs` 从 `DevSummary`（完整配置快照）实时生成配置文本，与 show 各视图同源——不做单独的“配置缓冲区”状态跟踪；接口名必须走 `ifname::canonical`（方言/设备类型规则：IOS 全 0 起、VRP 路由器 0 起而交换机 1 起、Comware 1/0 起），L3 设备不渲染 switchport 段。

### D13（M3a 补充）Markdown 零依赖渲染

课程/笔记的 Markdown 用自研 ~90 行渲染器（标题/列表/代码块/加粗/行内代码，HTML 先转义），不引 marked 等外部依赖——内容为自控/用户自写，子集可控，且规避离线安装问题。

### D14（M3b 补充）LACP 用逻辑端口模型接入 STP 与转发

链路聚合不新建"聚合口设备/端口"，而是在端口上记录 `lag` 归属，交换机内维护 `LagGroup`（模式/成员/选中集合）。数据面：出方向按 (目的 MAC, 源 MAC) 哈希在选中成员间分担，入方向学习归到逻辑端口以避免成员间漂移；STP 只在每组的最小选中成员（代表端口）上运行，其余成员继承代表端口的转发状态，BPDU 收发均归一到代表端口。这样"环路 + 双链路聚合"在 STP 看来只有一条链路，不会误阻塞。

LACP 采用 1s 快速周期与 3.5s 超时（真实设备为 1s/30s 慢速），收敛速度优先于计时保真；静态聚合（`mode on` / `manual` / `static`）不跑 LACPDU，端口 up 即转发。当前仅交换机支持聚合，路由器 LAG 留待后续。

### D15（M2 收尾补充）远程登录用"会话转发"而非模拟终端

Telnet/SSH 不做真实协议栈，而是：登录命令进入用户名/口令输入流程（状态存在 CLI `Session` 中），认证由引擎 `try_login` 校验（管理 IP 归属 + 路由可达 + VTY 传输协议 + 本地用户/服务类型）。认证成功后创建一个**远程子会话**，后续命令行转发给目标设备的命令树执行、输出原样返回，`exit`/`logout` 断开。这样 SSH 登录后能真正 show 与改配置，教学语义完整，又不需要跨设备终端复用或真实 TCP。

局限（有意为之）：口令在本地明文存储、SSH 不做密钥交换、远程会话不校验 enable 口令；会话状态由前端每设备持有，刷新后断开。

### D16（M2 收尾补充）RSTP 采用"快速角色切换"简化实现

不实现完整的 RSTP 提案/同意（P/A）标志与边缘端口：模式切到 RSTP 后，根端口/指定端口在角色确定时直接进入 Forwarding，端口故障重算后替换端口立即接替根端口；BPDU 仍沿用 802.1D 格式与 2s 周期。教学目标是把"RSTP 收敛快"与"802.1D 需 30s"的差异直观呈现，而非协议比特级保真。显示层按模式渲染 `rapid-pvst` / `RSTP`。

### D17（M2 收尾补充）MSTP 按实例复用 STP 算法，不做域协商

MSTP 简化为"按实例独立计算"：BPDU 增加 `instance` 字段，各实例每 2s 独立收发；实例数据（根/角色/端口状态/BPDU 缓存）存放在 `MstInstance` 中，计算时**临时换入 CIST 字段**复用现有 802.1D/RSTP 算法，算完换回，避免复制一套算法。数据面 `can_forward(dev, port, vlan)` 按 `vlan_instance` 选择实例状态，因此同一端口可在不同 VLAN 分别转发/阻塞。

有意省略：MST 域（region name/revision/digest）协商、CIST 与 MSTI 的同步标志、instance 级拓扑变化传播；配置视图接受 name/revision 但不参与计算。MSTI 与 CIST 一样在每次 hello 时重算，首帧即可完成选举。

### D18（M4 补充）OSPF 用"Hello + Router-LSA 泛洪"简化，VRRP 用接口级主备

OSPF 不做完整报文族（DBD/LSR/LSUpdate 分片、DR/BDR 选举、多区域、认证）：Hello 每 2s 携带自己的 Router-LSA，收到新 LSA 时按"除入端口外泛洪"扩散；每个路由器用 LSDB 跑 Dijkstra（边权 1），只安装非直连网段的路由（来源优先级：连接 > 静态 > OSPF）。邻居 6s 未收到 Hello 失效并重新泛洪。Router ID 取最高接口 IP。这样三台路由器即可演示动态路由与收敛，字段与真实报文不对应。

VRRP 只实现单组主备：优先级高者 Master（平局比接口 IP），Master 每秒发通告，Backup 3.5s 收不到即接管；虚拟 MAC 固定 00:00:5e:00:01:{vrid}，Master 用它应答 VIP 的 ARP/ICMP，帧目的为虚拟 MAC 时按本机处理。无抢占延迟、无认证、无 IPv6 VRRP。

### D19（M4 补充）IPv6 只做"地址 + NDP + ICMPv6 + 静态路由"

不实现链路本地地址、SLAAC、DHCPv6、扩展头与分片：接口地址为全局单播（可多个接口各一个），NDP 用 solicited-node 组播完成 NS/NA，邻居缓存立即生效（无 DAD）；ICMPv6 仅 Echo/NS/NA；路由为连接路由 + 静态路由（下一跳需在连接网段），仅路由器转发、交换机 SVI 不参与 IPv6。主机通过属性面板或 wasm `set_host_v6` 配置，终端用 `ping6`。v6 ping 会话简化为"5 次发送 + 末次后 2s 结束"，不逐探针判定超时。

### D20（M4 补充）二层安全与 QoS 采用可观测的简化模型

DHCP Snooping：全局开关 + 端口信任；非信任端口的 Offer/Ack 直接丢弃，Request/Ack 学习 (MAC, IP, VLAN, 端口) 绑定表；无 Option 82、无速率限制、无绑定表强制校验（绑定仅展示）。

端口安全：每端口最大 secure MAC 数、违规动作 restrict（丢帧）/ shutdown（关端口并触发 STP 重算），sticky 仅记录标记；MAC 表为会话级，不跨刷新持久化。

QoS 只做**入口 policing**：广播帧每秒上限（storm control）与令牌桶限速（kbps，桶容量 1 秒流量）；不做 DSCP/CoS 分类、队列调度与拥塞管理。实现位于 `switch_ingress`（VLAN 归类之后、STP 丢弃之前），统计通过 `qos_stats_of` 暴露给 show。

### D21（M5 补充）BGP 用广播报文模拟，仅 eBGP 单跳语义

不做 TCP 179、iBGP、MED/community、路由反射与聚合：BGP 报文（Open/Keepalive/Update）以广播帧发送，邻居按源 IP 识别，1s 周期 Keepalive、5s 无包断会话。Update 携带固定 8 条路由（net/len + AS_PATH），接收方做 AS_PATH 环路检测，按最短 AS_PATH 选路并安装 `RouteSrc::Bgp`；来源优先级为直连 > 静态 > BGP > OSPF。配置（AS、邻居、network 宣告）进摘要持久化，学习到的路由为瞬态。LOCAL_PREF 与路由过滤由 D23 的 route-policy 补充。

### D22（M5 补充）IS-IS / VRF / MPLS 的教学型简化

IS-IS：单区域 Level-1，无 DIS/IIH 定时器细节与多级路由；Hello（广播）每 2s 携带本机 LSP，收到新 LSP 除入端口外泛洪；LSDB 跑 Dijkstra，网络需接口配置 IPv4（下一跳用），安装 `RouteSrc::Isis`（AD 115，优先级最低）。

VRF：`Vrf` 只承载**连接路由 + 静态路由**，端口按 `vrf` 归属；入口按端口选择 VRF 表转发，NAT/动态协议/管理面只作用于全局表；跨 VRF 天然隔离。配置进摘要（端口保存 VRF 名称）。

MPLS：只做**静态 LSP**：ingress（FEC→out-label）压签、transit 换签、egress 弹签；标签帧以广播 MAC 在链路上传输（免 LDP/RSVP 与 ARP 解析），最多 2 层标签。属于教学演示，不对应真实 LDP 行为。

### D23（M5 补充）route-policy 只做 BGP 邻居策略：ACL 前缀匹配 + Local Preference

`RoutePolicy` 由若干个 `RoutePolicyNode { seq, permit, match_acl, set_local_pref }` 组成，按 seq 升序匹配，首个命中节点决定 permit/deny 与属性；无节点命中 = 隐式拒绝。`match_acl` 只做**前缀匹配**：ACL 规则用 `matches_route` 仅比较源地址与通配符（协议、目的不参与），ACL 判定 permit 视为命中。策略绑定在 BGP 邻居的 import/export 方向：import 在接收 Update 时过滤并可设置 Local Preference，export 在发送 Update 前过滤（不携带 LOCAL_PREF，符合 eBGP 非传递语义）；引用不存在的策略按拒绝处理。Local Preference 参与最优路径比较（高者优先，其次 AS_PATH 短者），重新向邻居通告时重置为 100。策略变更时清空已学习路由并重算，等待邻居下一轮 Update 重新学习。

命令：IOS `route-map NAME permit|deny SEQ` → `match ip address ACL` / `set local-preference N`，BGP 下 `neighbor IP route-map NAME in|out`；VRP/Comware `route-policy NAME permit|deny node SEQ` → `if-match acl N` / `apply local-preference N`，BGP 下 `peer IP route-policy NAME import|export`；`show route-map` / `display route-policy` 列出全部策略（当前不支持按名称过滤）。

不做：MED、community、AS_PATH prepend、OSPF/IS-IS 的 filter-policy、策略嵌套/子策略、ACL 时间范围与高级匹配。另修正 ACL 通配符比较方向（此前比较了被忽略位），基础 ACL 未限制目的字段时不再误判目的地址。

### D24（M5 补充）MPLS L3VPN 用静态双标签 + VRF 标签映射

`StaticLsp` 增加 `vrf` 与 `vpn_label`：ingress 按 **VRF + FEC** 匹配，压入 `[传输标签, VPN 标签]`（depth=2）；P 只交换外层标签；出口 PE 的外层 egress LSP 无 `vrf`，弹掉外层后把内层 VPN 标签再查一次 LSP，命中带 `vrf` 的 egress 项即把裸 IPv4 报文投递到对应 VRF，再由 VRF 路由表发给 CE。VRF 入口的压签先于 VRF 查表（PE 的 VPN 路由下一跳位于全局 MPLS 域），因此不需要在 VRF 内解析远端 PE 下一跳。命令为 `static-lsp ingress … out-label N vrf NAME vpn-label M` / `static-lsp egress … incoming-label M vrf NAME`；`show/display mpls lsp` 增加 VRF 与 VPN 列。配置（含 vrf/vpn_label）进摘要。

不做：MP-BGP VPNv4 路由分发、LDP/RSVP、PHP、VPN 实例 RD/RT、跨 VRF 泄漏；`vpn-label` 必须配合 `vrf`，标签值不做全局唯一性校验（教学拓扑由使用者保证）。大型园区实验复用既有 VLAN/LACP/SVI/OSPF/VRRP/NAT，不新增引擎能力；静态路由现已允许 `0.0.0.0/0` 默认路由（原先误判为非法）。

### D25（M6 起步）防火墙复用路由器数据面，区域策略为状态化 ICMP 简化

`DeviceKind::Firewall` 复用 `RouterState`（接口、路由、NAT、ACL 全部可用），只额外增加端口 `zone` 与 `RouterState.security_policies/security_sessions`。转发时在路由查表后、出向 ACL/NAT 前判定：同区域或未划分区域放行；跨区域按策略顺序首条匹配，无命中默认拒绝；策略 permit 后记录 `(src,dst,icmp_id)` 会话，30 秒内反向回包直接放行。当前只对 ICMP 建模状态（本模拟器三层载荷只有 ICMP），不对 TCP 状态机、序列号与超时细节建模。

命令按厂商语义分叉：VRP `firewall zone` + `security-policy` + `rule name`，Comware `security-zone name` + `security-policy ip`，IOS 用简化 ZBF（`zone security`、接口 `zone-member security`、`zone-pair security` + `source`/`destination`/`action`，省略 class-map/policy-map/inspect）。不做：ASA `nameif`/`security-level`、IPSec/SSL VPN、应用层识别、NAT 与策略联动、区域间 IPv6 策略。

### D26（M6 补全）ASA 安全级别、TCP 状态机、IPsec/SSL VPN 的教学实现

**ASA 模型**：端口增加 `nameif`/`security_level`。防火墙在未配置区域时按安全级别处理：`in_level >= out_level` 放行并记录 ICMP 会话，低→高仅允许已有会话回包；配置区域时区域策略优先。不做完整 ASA ACL/object-group/application inspection。

**TCP 状态机**：新增 `TcpPacket` 与 `L3::Tcp`，PC `tcp <IP> <port>` 发起 SYN；主机监听 22/80/443，回 SYN-ACK，未监听端口回 RST；客户端完成 ACK 后发 FIN，服务端回 ACK+FIN，客户端再 ACK，实现 SYN_SENT → ESTABLISHED → FIN_WAIT → CLOSED。防火墙用五元组 `TcpFlow` 追踪状态（60s 老化），首包按区域/级别策略，后续报文命中流即放行。TCP 报文用广播 MAC 逐跳发送（免 ARP，和 BGP 一致），不承载应用数据，不做窗口/重传/序列号校验。

**IPsec**：`ipsec tunnel NAME local A remote B` + `local-subnet`/`remote-subnet`，命中保护子网的 IPv4 报文以 `IpsecPacket` 做 IP-in-IP 封装，外层按全局表路由，对端解封装后按内层目的转发；仅 ICMP 载荷，不做 IKE/ESP/AH/加密/抗重放。

**SSL VPN**：防火墙 `sslvpn virtual-ip NET LEN` 定义虚拟地址池，复用本地用户认证；PC `sslvpn connect` 认证成功后获得虚拟 IP，访问非本地网段时封装到网关（内层源地址改为虚拟 IP），网关解封装后按内网路由转发；回程目的为虚拟 IP 时按客户端映射封装回真实 IP。仅 ICMP 隧道，不做 TLS/证书/隧道分片/资源授权。

### D27（M7 起步）无线用 CAPWAP 配置面 + 二层桥接模拟

`DeviceKind::AccessPoint` 复用 `SwitchState`（VLAN/桥接/MAC 学习），`DeviceKind::WlanController` 复用 `RouterState`（IP/路由）。AP 的 WLAN 配置存在 `Device.wlan`：SSID、Open/WPA2-PSK、业务 VLAN、CAPWAP AC 地址、joined 状态。CAPWAP 报文（Discovery/Join/Config）用广播 MAC 在二层泛洪，字段为固定长度数组；AP `capwap ac` 发 Join，AC 按自身 WLAN 配置回 Config，AP 应用并把非 Trunk 端口划入业务 VLAN。PC `wifi connect` 校验 SSID 与 PSK 后写入 `HostState.wifi`；AP 入口对本端主机检查关联，未关联帧丢弃。无线关联是瞬态，重载后需重新连接；WLAN 配置进摘要。

不做：802.11 空口帧/速率/信道/射频、漫游/切换、WPA3/802.1X/EAP、多 SSID、AP 冗余与 AC 热备、CAPWAP 跨三层（DTLS/隧道），口令明文仅存本机（教学用途）。

### D28（M7 补充）多 SSID、WPA3-SAE 与显式漫游

`WlanConfig` 从单 SSID 改为 `profiles: Vec<WlanProfile>`（SSID/Open|WPA2-PSK|WPA3-SAE/口令/业务 VLAN）。AC 用 `wlan ssid` 创建并选中 profile，后续 `wlan security`/`wlan vlan` 作用于选中项；CAPWAP 下发改为 `ConfigReset` + 每条 profile 一个 `Config`，AP 收到 Reset 清空后逐条合并。客户端 `wifi connect` 在所有已加入且含该 SSID 的 AP 中按 `wlan priority` 选最高优先级 AP，动态建立 Host↔AP 链路，并把该 AP 端口划入 profile VLAN；不同 SSID 的客户端落在不同 VLAN，默认隔离。

WPA3 是**口令校验**而非完整 SAE：`WlanSecurity::Wpa3Sae` 与 WPA2 分开校验/报错，不做 Dragonfly 握手、PMF 或过渡模式。漫游由 PC `wifi roam` 显式触发：移除当前 Host↔AP 链路，按同 SSID 选择另一 AP 重关联；不做 802.11r/k/v、信号强度/RSSI、AP 间隧道或自动触发（自动切换留待后续）。

### D29（M8 起步）VXLAN 用静态 VTEP + 二层帧封装

`SwitchState.vxlan: VxlanConfig { local, mappings(VNI↔VLAN), peers }`；命令 `vxlan local`、`vxlan peer`、`vxlan vni N vlan V`。交换机收到本地 VLAN 的广播/未知单播且该 VLAN 有 VNI 映射时，复制一份封装为 `VxlanPacket`（外层 src/dst VTEP + VNI + 内层 MAC/VLAN + Ipv4/Arp 载荷），通过 Underlay 路由按 VLAN 泛洪到对端；对端 VTEP 匹配 VNI/VLAN 后解封装，按内层目的 MAC 查本地表或泛洪（排除入口端口）注入。只为 ICMP/ARP 载荷建模，不封装 TCP；Underlay 需能路由到对端 VTEP（本实验用直连 VLAN100）。

不做：EVPN BGP 控制面（Route Type 2/3/5、MAC/IP 学习分发）、ARP 代理/未知单播抑制、多租户 VRF 映射、VXLAN-GPO/EVPN 多归、堆叠/IRF/CSS 与 M-LAG；这些作为 M8 后续。

### D30（M8 补充）堆叠/IRF 与 M-LAG 用共享系统 MAC + fabric 链路模拟

`SwitchState.stack { domain, member, priority }` 与 `mlag { domain, peer_links }`。同一 stack domain 的成员间链路、以及显式 `mlag peer-link` 端口被视为 **fabric 链路**：不参与 STP、始终可转发；堆叠/M-LAG 交换机不跑 STP，BPDU 直接丢弃（避免关 STP 后 BPDU 泛洪成环）。LACP 发送时若交换机属于 stack/M-LAG，actor system 使用由 domain 派生的共享系统 MAC（`02:00:00:<domain>:01|02`），使接入交换机的跨设备 LACP 把两台成员视为同一对端并同时选中。M-LAG peer-link 做水平分割：从 peer-link 进入的广播/未知单播不再回送跨设备 LAG，避免接入 LAG 的 MAC 被反射帧覆盖。

堆叠/IRF/CSS 为简化：不做真实堆叠协议、成员选举报文、配置同步、分裂检测（split-brain），Master 仅按 priority/member 排序展示；M-LAG 不做一致性检查、DFS/双主检测、peer-link 故障隔离与静态/动态 MAC 同步。实验十七在接入交换机上 `stp disable`，符合 M-LAG 双归不依赖 STP 的常见做法。

### D31（M8 补充）EVPN 用简化 Type-3 通告，F5 LTM 用 TCP 轮询与地址转换

EVPN：交换机 `evpn enable` + `evpn peer <IP>`，每 2s 向 EVPN peer 通告本端 VTEP 与每个 VNI/VLAN（简化 Type-3），对端学习后写入 `evpn.routes` 并自动把 VTEP 加入 `vxlan.peers`；不需要静态 `vxlan peer`。不做真实 BGP EVPN 会话/Route Target/RT-2 MAC-IP/RT-5 IP Prefix/ARP 代理，数据面仍由 VXLAN 泛洪完成。EVPN 报文沿用广播 MAC 与 Underlay VLAN。

F5 LTM：新增 `DeviceKind::LoadBalancer`（复用 RouterState），Device 增加 `ltm`（pool/virtual；会话瞬态）。`ltm pool NAME member IP PORT`、`ltm virtual NAME vip IP PORT pool NAME`；TCP 连接命中 VIP 时按池成员轮询选择，DNAT 到成员并记录会话，回程按会话把源地址改回 VIP；VIP 由本端口 MAC 应答 ARP。不做健康检查、持久化/源地址保持、SSL 卸载、iRules、SNAT pool、F5 tmsh 方言；仅 TCP 负载均衡（复用 D26 的 TCP 状态机）。

### D32（M9 补充）同 VLAN 隔离、OSPFv3 与 SRv6 BE 的简化边界

**同 VLAN 隔离**：`Port` 增加 `isolate_group: Option<u16>` 与 `mux_role: Option<MuxRole>`（Principal/Separate/Group(u16)）。数据面在二层转发时检查同 VLAN 的源/目的端口：principal 与所有端口互通；separate 两两隔离且与 group 隔离；group 仅同组互通；同 `isolate_group` 的端口不能互通（思科 `switchport protected` 用 group 0 表达）。命令 `mux-vlan principal|separate|group N`、`port-isolate enable group N`。不做 MUX VLAN 的跨 VLAN primary/subordinate 二层桥接（各 subordinate VLAN 仍需三层网关），也不做 PVLAN 的 trunk 承载与 VACL。

**OSPFv3**：新增 `Ospfv3Packet`（Router ID/Area/源 IPv6/前缀数组）与独立 `Ospfv3Runtime`；接口 `ospfv3 1 area 0` 启用后周期发送本机前缀，邻居学习前缀并安装 `Route6`（`RouteSrc::Ospf`）。`lookup_route6` 将 OSPFv3 路由排在静态之后、直连之前。简化：不做完整 LSA 泛洪、DR/BDR、区域间汇总、LSA 老化与认证（认证交给 IPv6 IPsec）；前缀学习经 D33 增强后可穿越中间路由器（带水平分割的泛洪），仍无 DR/BDR 与区域间汇总。

**SRv6 BE**：`RouterState.srv6 { locator, sids, routes }`；命令 `srv6 locator`、`srv6 sid`、`srv6 route ... sid`。入口匹配业务前缀后封装 `Srv6Packet`（外层 IPv6 目的=SID，内层原始 IPv6），Underlay 用 `lookup_route6` 转发；目的 SID 属于本机 locator/SID 时解封装并按内层 IPv6 转发。不做 SRH、多段 Segment List、SRv6 Policy/TE、TI-LFA、切片与头压缩；BE 外层用广播 MAC 发送（同 IPsec 教学模型），依赖 Underlay 路由可达 SID。

**拓扑与 STP**：前端 `add_device`/`add_link`/`remove_link` 后调用 `Engine::topology_changed()` 重算 STP 并启动 hello 定时器；`lab_checks::run_checks` 先推进 45s 虚拟时间，等 STP 从 Listening/Learning 进入 Forwarding，避免判分时端口仍在 Learning 被误判为不通。

### D33（M9 补充）多节点组网的封装帧 Underlay 转发与 OSPFv3 泛洪

**动机**：SRv6/VXLAN/EVPN/IPsec 等实验升级为多节点组网（PE1—P—PE2、R1—ISP—R2、DSW1—SPINE—DSW2）后，暴露出引擎三个“两台直连拓扑掩盖了”的缺陷。

**IPv6 本机交付**：`ipv6_on_frame` 的本机交付判定从“入口端口地址”（`my_ipv6_on`）扩为 `my_ipv6_any`——本机任一端口的 IPv6 地址，路由器上含本地 SRv6 SID 与 Locator 内地址。此前路由器收到目的为自身其他接口地址的包会按转发表回环转发（Direct 命中自己口），ND 无应答而丢弃，ping6 路由器跨接口地址必失败。

**OSPFv3 泛洪**：Update 携带 learned 路由（`ospfv3_routes6`），带水平分割——不向学到该路由的接口发回（来源口由邻居表按 next-hop 反查）。此前泛洪只走一跳，三台链式时端点永远学不到对端网段。包内前缀上限 8 条。

**封装帧 Underlay 转发**：新增 `encap_fwd_over_underlay`——非 VTEP 路由器收到 `L3::Vxlan`/`L3::Evpn` 帧时按外层单播 IPv4 查表转发（广播 MAC 逐跳约定，VXLAN TTL 递减，出口≠入口）；此前路由器入口对这两类载荷直接丢弃（注释写明“仅交换机 VTEP 处理”），VTEP 隔着路由器无法互通。`l3_ingress` 与 `switch_ingress` 两路入口都接该 helper。

**封装帧发送 VLAN 解析**：`vxlan_encap`/`evpn_send` 的 peer 若经静态路由可达，路由表项无 VLAN（`out_vlan=None`），会落到 SVI 伪端口 `transmit_frame(port=0)` 从错误口发出——直连拓扑（peer 在直连段）从未暴露。修复：按下一跳所在直连网段二次解析出发送 VLAN（`lookup_route(nh)`），再走 `flood_in_vlan_except`。

**TCP over VXLAN**：`VxlanPayload` 增加 `Tcp(TcpPacket)` 变体，`vxlan_encap`/解封装处理 TCP 载荷（`L3::Tcp`，ETHERTYPE_IPV4）——此前只封装 Ipv4/Arp，tcpProbe 的 SYN 无法跨 VXLAN（数据中心综合实验的 VIP 探针场景暴露）。同时 M-LAG 组网的 DSW 侧也应 `stp disable`：`mlag domain` 的共享系统 MAC 使两台桥 ID 相同、STP 选举平局，peer-link 两侧全 DESI 转发会形成广播风暴，淹没交换机自身发起的 ARP。

**拓扑升级原则**：判分新增项只用于表达新增的学习点（如“Underlay 贯通”“过滤传导到下游”），不改引擎既有判分语义；任务文案必须与模拟器的 CLI 行为一致（VRP 接口 `quit` 落回系统视图、静态路由用前缀长度格式）。

## 后果

- 三方言首版交付慢一些（M0 即验证多方言架构），但 M1+ 平移成本递减。
- 逐包仿真在 M2+ 引入周期性协议流量（STP/OSPF Hello）时需要事件合并/限速，已在 D2 护栏内预留。
- 换 UI 框架成本被限制在 web/ 目录；引擎 crate 与 UI 无耦合。
