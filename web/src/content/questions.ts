// 题库 v1：IA 级别单选题（原创题面与解析）。
// 题目内容为原创，仅作描述性使用各厂商命令语法。

export type TopicId = 'basics' | 'vlan' | 'stp' | 'dhcp' | 'acl' | 'nat';

export const TOPICS: { id: TopicId; name: string }[] = [
  { id: 'basics', name: '网络基础' },
  { id: 'vlan', name: 'VLAN' },
  { id: 'stp', name: 'STP' },
  { id: 'dhcp', name: 'DHCP' },
  { id: 'acl', name: 'ACL' },
  { id: 'nat', name: 'NAT' },
];

export interface Question {
  id: string;
  topic: TopicId;
  q: string;
  options: string[];
  /** 正确选项下标 */
  answer: number;
  explain: string;
}

export const QUESTIONS: Question[] = [
  // ---------- 网络基础 ----------
  {
    id: 'b1',
    topic: 'basics',
    q: '交换机根据什么转发以太网帧？',
    options: ['目的 IP 地址', '目的 MAC 地址', '端口号', 'TTL 值'],
    answer: 1,
    explain: '以太网交换机是二层设备，依据帧头中的目的 MAC 地址查 MAC 地址表转发。',
  },
  {
    id: 'b2',
    topic: 'basics',
    q: 'ARP 协议的作用是？',
    options: [
      '把域名解析为 IP 地址',
      '把 IP 地址解析为 MAC 地址',
      '把 MAC 地址解析为 IP 地址',
      '动态分配 IP 地址',
    ],
    answer: 1,
    explain: 'ARP（地址解析协议）通过广播询问"谁持有某 IP"，获得对应的 MAC 地址。',
  },
  {
    id: 'b3',
    topic: 'basics',
    q: 'ICMP echo request 的 TTL 字段每经过一台路由器会怎样？',
    options: ['加 1', '减 1', '不变', '清零'],
    answer: 1,
    explain: '每台路由器转发时将 TTL 减 1，减到 0 则丢弃并发送超时报文——这是 traceroute 的原理。',
  },
  {
    id: 'b4',
    topic: 'basics',
    q: '192.168.1.100/26 的子网掩码和可用主机数是？',
    options: [
      '255.255.255.192，62 台',
      '255.255.255.224，30 台',
      '255.255.255.192，64 台',
      '255.255.255.240，14 台',
    ],
    answer: 0,
    explain: '/26 即掩码 255.255.255.192，主机位 6 位，2^6-2 = 62 台可用（去掉网络地址和广播地址）。',
  },
  {
    id: 'b5',
    topic: 'basics',
    q: '交换机收到目的 MAC 未知的数据帧时会？',
    options: ['丢弃', '向除入端口外的所有端口洪泛', '回送错误报文', '缓存等待 ARP'],
    answer: 1,
    explain: '未知单播洪泛（unknown unicast flooding）是交换机的正常行为；MAC 表学习到目的后再恢复单播转发。',
  },
  {
    id: 'b6',
    topic: 'basics',
    q: 'ping 命令使用的是哪两种 ICMP 报文？',
    options: [
      'Redirect 与 Unreachable',
      'Echo Request 与 Echo Reply',
      'Source Quench 与 Time Exceeded',
      'Router Advertisement 与 Solicitation',
    ],
    answer: 1,
    explain: 'ping 通过 Echo Request（类型 8）与 Echo Reply（类型 0）测量往返时延与连通性。',
  },

  // ---------- VLAN ----------
  {
    id: 'v1',
    topic: 'vlan',
    q: '同一 VLAN 内的两台主机分别接在同一交换机的两个 Access 口（VLAN 10），它们通信时帧是否携带 802.1Q 标签？',
    options: ['带标签 VLAN 10', '不带标签', '带标签 VLAN 1', '双标签'],
    answer: 1,
    explain: 'Access 口只在交换机内部区分 VLAN，向主机发送时剥离标签；主机看到的都是普通以太网帧。',
  },
  {
    id: 'v2',
    topic: 'vlan',
    q: '两台交换机之间的 Trunk 链路默认放行 VLAN 1，要让它承载 VLAN 10 的流量，华为设备需要？',
    options: [
      'port default vlan 10',
      'port trunk allow-pass vlan 10',
      'port link-type access',
      'undo port trunk pvid',
    ],
    answer: 1,
    explain: '华为/华三 Trunk 默认仅放行 VLAN 1，必须用 allow-pass（Comware 用 permit）显式放行；思科默认放行全部 VLAN。',
  },
  {
    id: 'v3',
    topic: 'vlan',
    q: '三层交换机上实现 VLAN 间路由，通常使用？',
    options: [
      '每个 VLAN 一个物理口',
      'SVI（VLANIF 虚接口）',
      'Router-on-a-stick 子接口',
      '新增一台路由器全互联',
    ],
    answer: 1,
    explain: '三层交换机为每个 VLAN 创建 SVI（如 Vlanif10）作为该 VLAN 的网关，报文在 VLAN 间经内部路由转发。',
  },
  {
    id: 'v4',
    topic: 'vlan',
    q: '跨 VLAN 通信（VLAN10 → VLAN20，经 SVI 路由）后，对端收到回包的 TTL 相比直连同网段通信会？',
    options: ['不变', '减 1', '减 2', '归零'],
    answer: 1,
    explain: '每经过一次三层转发（SVI 间路由）TTL 减 1，所以跨 VLAN ping 的回包 TTL 比同网段小 1。',
  },
  {
    id: 'v5',
    topic: 'vlan',
    q: 'Trunk 口的原生 VLAN（PVID/Native VLAN）流量在链路上是？',
    options: ['带原生 VLAN 标签', '不带标签', '加密传输', '被丢弃'],
    answer: 1,
    explain: 'Native VLAN（华为称 PVID）的帧在 Trunk 上不加 802.1Q 标签；两端 PVID 不一致会导致 VLAN 串扰。',
  },

  // ---------- STP ----------
  {
    id: 's1',
    topic: 'stp',
    q: 'STP 根桥的选举依据是？',
    options: [
      '桥 MAC 最小的胜出',
      '桥优先级最小的胜出，相同则比 MAC',
      'IP 地址最小的胜出',
      '端口数最多的胜出',
    ],
    answer: 1,
    explain: '比较桥 ID（优先级 + MAC）：先比优先级（默认 32768，步长 4096），相同再比 MAC，越小越优。',
  },
  {
    id: 's2',
    topic: 'stp',
    q: '非根桥交换机上，到根桥累计路径成本最小的端口被称为？',
    options: ['指定端口（DP）', '根端口（RP）', '替换端口（Alternate）', '边缘端口'],
    answer: 1,
    explain: '每台非根桥有且仅有一个根端口——去往根桥总成本最小的口；其余与根桥冲突的口成为替换端口并阻塞。',
  },
  {
    id: 's3',
    topic: 'stp',
    q: '802.1D STP 中，端口从 Listening 到 Forwarding 需要经过 Learning，默认每段耗时？',
    options: ['2 秒', '15 秒', '20 秒', '50 秒'],
    answer: 1,
    explain: 'Forward Delay 默认 15 秒，Listening→Learning→Forwarding 各 15 秒，加上 Max Age 20 秒构成经典收敛时间。',
  },
  {
    id: 's4',
    topic: 'stp',
    q: 'STP 阻塞端口（Alternate）收到数据帧时会？',
    options: ['正常转发', '丢弃，但仍接收 BPDU', '转发并洪泛', '回送拒绝报文'],
    answer: 1,
    explain: '阻塞端口不转发数据帧（防环路），但持续监听 BPDU 以便拓扑变化时快速接替；BPDU 老化时间为 Max Age 20s。',
  },
  {
    id: 's5',
    topic: 'stp',
    q: '把某台交换机设为根桥，最直接的做法是？',
    options: [
      '增大它的桥优先级',
      '减小它的桥优先级（如 4096）',
      '关闭它的 STP',
      '提高它的端口成本',
    ],
    answer: 1,
    explain: '优先级越小越优先成为根桥；华为/华三命令 stp priority，思科 spanning-tree vlan 1 priority（步长 4096）。',
  },

  // ---------- DHCP ----------
  {
    id: 'd1',
    topic: 'dhcp',
    q: 'DHCP 的四步交互顺序是？',
    options: [
      'Offer → Discover → Ack → Request',
      'Discover → Offer → Request → Ack',
      'Request → Discover → Ack → Offer',
      'Discover → Request → Offer → Ack',
    ],
    answer: 1,
    explain: 'DORA：客户端广播 Discover，服务器回应 Offer，客户端广播 Request 确认，服务器下发 Ack 完成租约。',
  },
  {
    id: 'd2',
    topic: 'dhcp',
    q: 'DHCP Discover 报文的源 IP 和目的 IP 是？',
    options: [
      '0.0.0.0 → 255.255.255.255',
      '127.0.0.1 → 127.0.0.1',
      '客户端 MAC → 服务器 MAC',
      '169.254.x.x → 224.0.0.1',
    ],
    answer: 0,
    explain: '客户端此时没有地址，源为 0.0.0.0；以受限广播 255.255.255.255 发送——这也是 DHCP 广播穿过三层网络需要中继的原因。',
  },
  {
    id: 'd3',
    topic: 'dhcp',
    q: 'DHCP 服务器选择地址池下发地址的依据通常是？',
    options: [
      '客户端的主机名',
      '接收请求的接口所在网段',
      '报文到达的先后顺序',
      '客户端指定的 VLAN',
    ],
    answer: 1,
    explain: '路由器从哪个接口收到 Discover，就用与该接口同网段的地址池回应——这也是本模拟器池匹配的实现方式。',
  },
  {
    id: 'd4',
    topic: 'dhcp',
    q: '除了 IP 地址和掩码，DHCP 通常还会下发？',
    options: [
      '默认网关与 DNS 服务器',
      'MAC 地址与端口号',
      'VLAN ID 与 Trunk 配置',
      'STP 优先级',
    ],
    answer: 0,
    explain: '常用选项：option 3 默认网关、option 6 DNS、option 1 掩码——没有网关，客户端就无法跨网段通信。',
  },
  {
    id: 'd5',
    topic: 'dhcp',
    q: '两台 PC 先后从同一个地址池获取地址，第二台 PC 得到的地址？',
    options: [
      '与第一台相同',
      '池中的下一个空闲地址',
      '永远是广播地址',
      '随机任意地址',
    ],
    answer: 1,
    explain: '服务器维护租约表：已分配的地址不再下发，第二台拿到下一个空闲地址（本模拟器从池起点顺序分配）。',
  },

  // ---------- ACL ----------
  {
    id: 'a1',
    topic: 'acl',
    q: 'ACL 匹配规则"首条命中即生效"，如果数据包没有命中任何规则，结果是？',
    options: ['默认允许', '默认拒绝（隐式 deny）', '继续匹配下一条 ACL', '回送报错'],
    answer: 1,
    explain: 'ACL 末尾存在不可见的隐式 deny all——写 ACL 时"先放行需要的，其余自然被拒"。',
  },
  {
    id: 'a2',
    topic: 'acl',
    q: 'VRP 中 ACL 编号 3000~3999 属于哪类？',
    options: ['基础 ACL', '高级 ACL', '二层 ACL', '用户 ACL'],
    answer: 1,
    explain: '华为/华三：2000~2999 基础（只匹配源），3000~3999 高级（源+目的+协议）；思科对应 1~99 标准与 100~199 扩展。',
  },
  {
    id: 'a3',
    topic: 'acl',
    q: '通配符掩码 0.0.0.255 表示匹配时？',
    options: [
      '只匹配一个主机',
      '最后一个字节任意',
      '前三个字节任意',
      '全部 32 位精确匹配',
    ],
    answer: 1,
    explain: '通配符中 0 表示必须匹配、1 表示忽略：0.0.0.255 即前 24 位精确、后 8 位任意，对应一个 /24 网段；0 表示精确匹配主机。',
  },
  {
    id: 'a4',
    topic: 'acl',
    q: '在接口入方向应用 ACL 过滤流量，判定发生在？',
    options: [
      '路由查表之后',
      '路由查表之前',
      '仅在数据帧出接口时',
      '仅在回包时',
    ],
    answer: 1,
    explain: '入方向 ACL 在报文刚进入接口时判定（先于路由）；出方向 ACL 在路由之后、离开接口之前判定。',
  },

  // ---------- NAT ----------
  {
    id: 'n1',
    topic: 'nat',
    q: 'Easy-IP（PAT）方式下，内网多台主机共享上网时，路由器改写的是？',
    options: [
      '目的 IP 改写为公网地址',
      '源 IP 改写为出口的公网 IP，并按会话区分回包',
      'TTL 改写为 128',
      'MAC 地址改写为网关 MAC',
    ],
    answer: 1,
    explain: 'PAT 把内网源地址+端口转换为出口公网地址+端口并记录映射表，回包按表还原——这正是"多台共享一个公网地址"的原理。',
  },
  {
    id: 'n2',
    topic: 'nat',
    q: 'NAT 路由器的 inside / outside 接口标记的作用是？',
    options: [
      '划分 VLAN',
      '确定报文在哪个方向做地址转换',
      '指定 DNS 服务器',
      '启用 DHCP 服务',
    ],
    answer: 1,
    explain: '从 inside 去往 outside 的报文做源转换（出向），回来的报文在 outside 口做目的还原（入向）。',
  },
  {
    id: 'n3',
    topic: 'nat',
    q: '静态 NAT 与 Easy-IP 的区别是？',
    options: [
      '静态 NAT 是一对一固定映射，Easy-IP 动态复用出口地址',
      '静态 NAT 只支持 TCP',
      'Easy-IP 不需要路由',
      '没有区别',
    ],
    answer: 0,
    explain: '静态 NAT 让外网可以主动访问该内网服务器（1:1 固定）；Easy-IP 由内网发起、动态建立会话，外网无法主动进入。',
  },
  {
    id: 'n4',
    topic: 'nat',
    q: '内网 PC 经 NAT ping 外网服务器，转换表里会记录什么信息用于还原回包？',
    options: [
      '内网源 IP 与 ICMP 会话 ID 的映射',
      '目的服务器的 MAC 地址',
      'TTL 的原始值',
      '服务器的公网 MAC',
    ],
    answer: 0,
    explain: 'ICMP echo 的 (内网源 IP, id) 组合被记录；回包目的为出口 IP 且 id 匹配时，还原为内网 IP 和原 id 送回发起者。',
  },
  {
    id: 'n5',
    topic: 'nat',
    q: '下列哪种情况**不**需要 NAT？',
    options: [
      '公司内网多台 PC 访问互联网',
      '内网服务器对公网提供 Web 服务',
      '同一 VLAN 内两台 PC 互访',
      '办公网与合作伙伴网络地址重叠时互访',
    ],
    answer: 2,
    explain: '同一 VLAN 内互访是纯二层交换，不涉及三层地址转换；其余场景都是 NAT 的典型用途。',
  },
];
