// 三厂商命令对照表（单一数据源）。
// 每行都会在运行时经 wasm 的 verify_command 校验——保证表格与模拟器行为一致。

export type CmdMode =
  | 'Exec'
  | 'Priv'
  | 'Config'
  | 'If'
  | 'Pool'
  | 'Acl'
  | 'Line'
  | 'Aaa'
  | 'Mst'
  | 'Ospf'
  | 'OspfArea'
  | 'Bgp'
  | 'Isis'
  | 'RoutePolicy'
  | 'Zone'
  | 'SecurityPolicy'
  | 'SecurityRule'
  | 'Ipsec';

export interface CmdRow {
  /** 功能描述 */
  feature: string;
  topic: string;
  /** 模拟器支持的三方言写法（示例参数按常用拓扑填写） */
  ios: string;
  vrp: string;
  comware: string;
  /** 命令所在模式（三方言共用） */
  mode: CmdMode;
  /** 个别厂商模式不同时的覆盖 */
  modeIos?: CmdMode;
  modeVrp?: CmdMode;
  modeComware?: CmdMode;
}

export const CMDMAP: CmdRow[] = [
  // 基础
  { feature: '设置设备名称', topic: 'basics', ios: 'hostname R1', vrp: 'sysname R1', comware: 'sysname R1', mode: 'Config' },
  { feature: '查看接口摘要', topic: 'basics', ios: 'show ip interface brief', vrp: 'display ip interface brief', comware: 'display ip interface brief', mode: 'Exec' },
  { feature: 'ping 测试', topic: 'basics', ios: 'ping 10.0.0.2', vrp: 'ping 10.0.0.2', comware: 'ping 10.0.0.2', mode: 'Exec' },
  { feature: '配置接口 IP', topic: 'basics', ios: 'ip address 192.168.1.1 255.255.255.0', vrp: 'ip address 192.168.1.1 24', comware: 'ip address 192.168.1.1 24', mode: 'If' },
  { feature: '启用接口', topic: 'basics', ios: 'no shutdown', vrp: 'undo shutdown', comware: 'undo shutdown', mode: 'If' },
  { feature: '静态路由', topic: 'basics', ios: 'ip route 192.168.2.0 255.255.255.0 10.0.0.2', vrp: 'ip route-static 192.168.2.0 24 10.0.0.2', comware: 'ip route-static 192.168.2.0 24 10.0.0.2', mode: 'Config' },
  { feature: '查看路由表', topic: 'basics', ios: 'show ip route', vrp: 'display ip routing-table', comware: 'display ip routing-table', mode: 'Exec' },

  // VLAN
  { feature: '创建 VLAN', topic: 'vlan', ios: 'vlan 10', vrp: 'vlan batch 10', comware: 'vlan 10', mode: 'Config' },
  { feature: '进入接口', topic: 'vlan', ios: 'interface g0/0', vrp: 'interface g0/0/1', comware: 'interface g1/0/1', mode: 'Config' },
  { feature: 'Access 口划 VLAN', topic: 'vlan', ios: 'switchport access vlan 10', vrp: 'port default vlan 10', comware: 'port access vlan 10', mode: 'If' },
  { feature: 'Trunk 模式', topic: 'vlan', ios: 'switchport mode trunk', vrp: 'port link-type trunk', comware: 'port link-type trunk', mode: 'If' },
  { feature: 'Trunk 放行 VLAN', topic: 'vlan', ios: 'switchport trunk allowed vlan 10,20', vrp: 'port trunk allow-pass vlan 10 20', comware: 'port trunk permit vlan 10 20', mode: 'If' },
  { feature: 'SVI 网关', topic: 'vlan', ios: 'interface vlan 10', vrp: 'interface vlanif 10', comware: 'interface vlan-interface 10', mode: 'Config' },
  { feature: '查看 VLAN', topic: 'vlan', ios: 'show vlan brief', vrp: 'display vlan', comware: 'display vlan', mode: 'Exec' },

  // STP
  { feature: '设置 STP 优先级', topic: 'stp', ios: 'spanning-tree vlan 1 priority 4096', vrp: 'stp priority 4096', comware: 'stp priority 4096', mode: 'Config' },
  { feature: '关闭 STP', topic: 'stp', ios: 'no spanning-tree vlan 1', vrp: 'stp disable', comware: 'stp disable', mode: 'Config' },
  { feature: '接口成本', topic: 'stp', ios: 'spanning-tree vlan 1 cost 100', vrp: 'stp cost 100', comware: 'stp cost 100', mode: 'If' },
  { feature: '查看 STP', topic: 'stp', ios: 'show spanning-tree', vrp: 'display stp brief', comware: 'display stp brief', mode: 'Exec' },

  // DHCP
  { feature: '创建地址池', topic: 'dhcp', ios: 'ip dhcp pool LAN', vrp: 'ip pool LAN', comware: 'ip pool LAN', mode: 'Config' },
  { feature: '池网段', topic: 'dhcp', ios: 'network 192.168.1.0 255.255.255.0', vrp: 'network 192.168.1.0 mask 255.255.255.0', comware: 'network 192.168.1.0 mask 255.255.255.0', mode: 'Pool' },
  { feature: '池网关', topic: 'dhcp', ios: 'default-router 192.168.1.1', vrp: 'gateway-list 192.168.1.1', comware: 'gateway-list 192.168.1.1', mode: 'Pool' },

  // ACL
  { feature: '创建高级 ACL', topic: 'acl', ios: 'access-list 100 deny icmp 192.168.1.0 0.0.0.255 192.168.3.0 0.0.0.255', vrp: 'acl 3000', comware: 'acl advanced 3000', mode: 'Config' },
  { feature: 'ACL 加规则', topic: 'acl', ios: 'access-list 100 permit icmp 10.0.0.0 0.0.0.255 192.168.3.0 0.0.0.255', vrp: 'rule permit icmp source 10.0.0.0 0.0.0.255', comware: 'rule permit icmp source 10.0.0.0 0.0.0.255', mode: 'Acl', modeIos: 'Config' },
  { feature: '接口应用 ACL', topic: 'acl', ios: 'ip access-group 100 in', vrp: 'traffic-filter inbound acl 3000', comware: 'traffic-filter inbound acl 3000', mode: 'If' },
  { feature: '查看 ACL', topic: 'acl', ios: 'show access-lists', vrp: 'display acl all', comware: 'display acl all', mode: 'Exec' },

  // NAT
  { feature: '标记 NAT 内口', topic: 'nat', ios: 'ip nat inside', vrp: 'nat outbound', comware: 'nat outbound', mode: 'If' },
  { feature: '标记 NAT 外口', topic: 'nat', ios: 'ip nat outside', vrp: 'nat outbound', comware: 'nat outbound', mode: 'If' },
  { feature: '静态 NAT', topic: 'nat', ios: 'ip nat inside source static 192.168.1.100 202.100.1.50', vrp: 'nat static global 202.100.1.50 inside 192.168.1.100', comware: 'nat static global 202.100.1.50 inside 192.168.1.100', mode: 'Config' },
  { feature: '查看转换表', topic: 'nat', ios: 'show ip nat translations', vrp: 'display nat session', comware: 'display nat session', mode: 'Exec' },

  // OSPF / VRRP
  { feature: '启动 OSPF 进程', topic: 'ospf', ios: 'router ospf 1', vrp: 'ospf 1', comware: 'ospf 1', mode: 'Config' },
  {
    feature: '宣告网段（区域 0）',
    topic: 'ospf',
    ios: 'network 10.1.0.0 0.0.0.3 area 0',
    vrp: 'network 10.1.0.0 0.0.0.3',
    comware: 'network 10.1.0.0 0.0.0.3',
    mode: 'Ospf',
    modeVrp: 'OspfArea',
    modeComware: 'OspfArea',
  },
  { feature: '查看 OSPF 邻居', topic: 'ospf', ios: 'show ip ospf neighbor', vrp: 'display ospf peer', comware: 'display ospf peer', mode: 'Exec' },
  { feature: 'VRRP 虚拟 IP', topic: 'ospf', ios: 'vrrp 1 ip 10.0.0.1', vrp: 'vrrp vrid 1 virtual-ip 10.0.0.1', comware: 'vrrp vrid 1 virtual-ip 10.0.0.1', mode: 'If' },
  { feature: 'VRRP 优先级', topic: 'ospf', ios: 'vrrp 1 priority 200', vrp: 'vrrp vrid 1 priority 200', comware: 'vrrp vrid 1 priority 200', mode: 'If' },
  { feature: '查看 VRRP 状态', topic: 'ospf', ios: 'show vrrp', vrp: 'display vrrp', comware: 'display vrrp', mode: 'Exec' },

  // BGP / route-policy
  { feature: '启动 BGP 进程', topic: 'bgp', ios: 'router bgp 65001', vrp: 'bgp 65001', comware: 'bgp 65001', mode: 'Config' },
  { feature: '配置 BGP 邻居', topic: 'bgp', ios: 'neighbor 10.0.0.2 remote-as 65002', vrp: 'peer 10.0.0.2 as-number 65002', comware: 'peer 10.0.0.2 as-number 65002', mode: 'Bgp' },
  { feature: '宣告 BGP 网段', topic: 'bgp', ios: 'network 192.168.1.0 mask 255.255.255.0', vrp: 'network 192.168.1.0 255.255.255.0', comware: 'network 192.168.1.0 255.255.255.0', mode: 'Bgp' },
  { feature: '创建路由策略', topic: 'bgp', ios: 'route-map FILTER deny 10', vrp: 'route-policy FILTER deny node 10', comware: 'route-policy FILTER deny node 10', mode: 'Config' },
  { feature: '策略匹配 ACL', topic: 'bgp', ios: 'match ip address 2000', vrp: 'if-match acl 2000', comware: 'if-match acl 2000', mode: 'RoutePolicy' },
  { feature: '设置 Local-Pref', topic: 'bgp', ios: 'set local-preference 200', vrp: 'apply local-preference 200', comware: 'apply local-preference 200', mode: 'RoutePolicy' },
  {
    feature: '邻居应用策略（入）',
    topic: 'bgp',
    ios: 'neighbor 10.0.0.1 route-map FILTER in',
    vrp: 'peer 10.0.0.1 route-policy FILTER import',
    comware: 'peer 10.0.0.1 route-policy FILTER import',
    mode: 'Bgp',
  },
  { feature: '查看 BGP 路由', topic: 'bgp', ios: 'show ip bgp', vrp: 'display bgp routing-table', comware: 'display bgp routing-table', mode: 'Exec' },
  { feature: '查看路由策略', topic: 'bgp', ios: 'show route-map', vrp: 'display route-policy', comware: 'display route-policy', mode: 'Exec' },

  // VRF / MPLS
  { feature: '创建 VRF', topic: 'mpls', ios: 'vrf definition CUST1', vrp: 'ip vpn-instance CUST1', comware: 'ip vpn-instance CUST1', mode: 'Config' },
  { feature: '接口绑定 VRF', topic: 'mpls', ios: 'ip vrf forwarding CUST1', vrp: 'ip binding vpn-instance CUST1', comware: 'ip binding vpn-instance CUST1', mode: 'If' },
  {
    feature: '静态 LSP（入口）',
    topic: 'mpls',
    ios: 'mpls static-lsp ingress to-ce2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100',
    vrp: 'static-lsp ingress to-ce2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100',
    comware: 'static-lsp ingress to-ce2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100',
    mode: 'Config',
  },
  {
    feature: '静态 LSP（中转）',
    topic: 'mpls',
    ios: 'mpls static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200',
    vrp: 'static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200',
    comware: 'static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200',
    mode: 'Config',
  },
  { feature: '查看 LSP', topic: 'mpls', ios: 'show mpls lsp', vrp: 'display mpls lsp', comware: 'display mpls lsp', mode: 'Exec' },
  { feature: '查看 VRF 路由表', topic: 'mpls', ios: 'show ip route vrf CUST1', vrp: 'display ip routing-table vpn-instance CUST1', comware: 'display ip routing-table vpn-instance CUST1', mode: 'Exec' },

  // 防火墙
  { feature: '创建安全区域', topic: 'security', ios: 'zone security trust', vrp: 'firewall zone trust', comware: 'firewall zone trust', mode: 'Config' },
  {
    feature: '接口划入区域',
    topic: 'security',
    ios: 'zone-member security trust',
    vrp: 'add interface g0/0/0',
    comware: 'add interface g0/0/0',
    mode: 'If',
    modeVrp: 'Zone',
    modeComware: 'Zone',
  },
  { feature: '创建区域间策略', topic: 'security', ios: 'zone-pair security ZP', vrp: 'security-policy', comware: 'security-policy', mode: 'Config' },
  { feature: '策略源区域', topic: 'security', ios: 'source trust', vrp: 'source-zone trust', comware: 'source-zone trust', mode: 'SecurityRule' },
  { feature: '策略目的区域', topic: 'security', ios: 'destination untrust', vrp: 'destination-zone untrust', comware: 'destination-zone untrust', mode: 'SecurityRule' },
  { feature: '策略动作', topic: 'security', ios: 'action permit', vrp: 'action permit', comware: 'action permit', mode: 'SecurityRule' },
  { feature: '查看安全区域', topic: 'security', ios: 'show zone security', vrp: 'display firewall zone', comware: 'display security-zone', mode: 'Exec' },
  { feature: '查看安全策略', topic: 'security', ios: 'show zone-pair security', vrp: 'display security-policy', comware: 'display security-policy', mode: 'Exec' },

  // IPsec / SSL VPN
  {
    feature: '创建 IPsec 隧道',
    topic: 'ipsec',
    ios: 'ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2',
    vrp: 'ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2',
    comware: 'ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2',
    mode: 'Config',
  },
  { feature: '本地保护子网', topic: 'ipsec', ios: 'local-subnet 192.168.1.0 24', vrp: 'local-subnet 192.168.1.0 24', comware: 'local-subnet 192.168.1.0 24', mode: 'Ipsec' },
  { feature: '远端保护子网', topic: 'ipsec', ios: 'remote-subnet 192.168.2.0 24', vrp: 'remote-subnet 192.168.2.0 24', comware: 'remote-subnet 192.168.2.0 24', mode: 'Ipsec' },
  { feature: '查看 IPsec 隧道', topic: 'ipsec', ios: 'show ipsec tunnel', vrp: 'display ipsec tunnel', comware: 'display ipsec tunnel', mode: 'Exec' },
  { feature: 'SSL VPN 虚拟地址池', topic: 'sslvpn', ios: 'sslvpn virtual-ip 10.10.0.0 24', vrp: 'sslvpn virtual-ip 10.10.0.0 24', comware: 'sslvpn virtual-ip 10.10.0.0 24', mode: 'Config' },
  { feature: '创建本地用户', topic: 'sslvpn', ios: 'username admin privilege 15 secret Admin@123', vrp: 'local-user admin password cipher Admin@123', comware: 'local-user admin password cipher Admin@123', mode: 'Config' },
  { feature: '查看 SSL VPN 会话', topic: 'sslvpn', ios: 'show sslvpn session', vrp: 'display sslvpn session', comware: 'display sslvpn session', mode: 'Exec' },

  // WLAN
  { feature: '配置 SSID', topic: 'wlan', ios: 'wlan ssid CORP', vrp: 'wlan ssid CORP', comware: 'wlan ssid CORP', mode: 'Config' },
  { feature: 'WPA2 预共享密钥', topic: 'wlan', ios: 'wlan security wpa2 psk Admin@123', vrp: 'wlan security wpa2 psk Admin@123', comware: 'wlan security wpa2 psk Admin@123', mode: 'Config' },
  { feature: 'WPA3-SAE 口令', topic: 'wlan', ios: 'wlan security wpa3 sae Guest@123', vrp: 'wlan security wpa3 sae Guest@123', comware: 'wlan security wpa3 sae Guest@123', mode: 'Config' },
  { feature: 'SSID 业务 VLAN', topic: 'wlan', ios: 'wlan vlan 10', vrp: 'wlan vlan 10', comware: 'wlan vlan 10', mode: 'Config' },
  { feature: 'AP 注册 AC', topic: 'wlan', ios: 'capwap ac 10.0.0.2', vrp: 'capwap ac 10.0.0.2', comware: 'capwap ac 10.0.0.2', mode: 'Config' },
  { feature: '查看 WLAN', topic: 'wlan', ios: 'show wlan', vrp: 'display wlan', comware: 'display wlan', mode: 'Exec' },

  // VXLAN / EVPN
  { feature: '配置本地 VTEP', topic: 'vxlan', ios: 'vxlan local 10.0.0.1', vrp: 'vxlan local 10.0.0.1', comware: 'vxlan local 10.0.0.1', mode: 'Config' },
  { feature: 'VNI 映射 VLAN', topic: 'vxlan', ios: 'vxlan vni 10000 vlan 10', vrp: 'vxlan vni 10000 vlan 10', comware: 'vxlan vni 10000 vlan 10', mode: 'Config' },
  { feature: '静态远端 VTEP', topic: 'vxlan', ios: 'vxlan peer 10.0.0.2', vrp: 'vxlan peer 10.0.0.2', comware: 'vxlan peer 10.0.0.2', mode: 'Config' },
  { feature: '启用 EVPN', topic: 'vxlan', ios: 'evpn enable', vrp: 'evpn enable', comware: 'evpn enable', mode: 'Config' },
  { feature: '配置 EVPN 邻居', topic: 'vxlan', ios: 'evpn peer 10.0.0.2', vrp: 'evpn peer 10.0.0.2', comware: 'evpn peer 10.0.0.2', mode: 'Config' },
  { feature: '查看 VXLAN', topic: 'vxlan', ios: 'show vxlan', vrp: 'display vxlan', comware: 'display vxlan', mode: 'Exec' },
  { feature: '查看 EVPN', topic: 'vxlan', ios: 'show evpn', vrp: 'display evpn', comware: 'display evpn', mode: 'Exec' },

  // 堆叠 / M-LAG
  { feature: '配置堆叠域', topic: 'ha', ios: 'stack domain 10', vrp: 'stack domain 10', comware: 'stack domain 10', mode: 'Config' },
  { feature: '配置堆叠成员号', topic: 'ha', ios: 'stack member 1', vrp: 'stack member 1', comware: 'stack member 1', mode: 'Config' },
  { feature: '配置主设备优先级', topic: 'ha', ios: 'stack priority 200', vrp: 'stack priority 200', comware: 'stack priority 200', mode: 'Config' },
  { feature: '配置 M-LAG 域', topic: 'ha', ios: 'mlag domain 1', vrp: 'mlag domain 1', comware: 'mlag domain 1', mode: 'Config' },
  { feature: '指定 M-LAG peer-link', topic: 'ha', ios: 'mlag peer-link', vrp: 'mlag peer-link', comware: 'mlag peer-link', mode: 'If' },
  { feature: '查看堆叠', topic: 'ha', ios: 'show stack', vrp: 'display stack', comware: 'display stack', mode: 'Exec' },
  { feature: '查看 M-LAG', topic: 'ha', ios: 'show mlag', vrp: 'display mlag', comware: 'display mlag', mode: 'Exec' },

  // F5 LTM
  { feature: '添加池成员', topic: 'ltm', ios: 'ltm pool WEB member 10.0.1.10 80', vrp: 'ltm pool WEB member 10.0.1.10 80', comware: 'ltm pool WEB member 10.0.1.10 80', mode: 'Config' },
  { feature: '创建虚拟服务器', topic: 'ltm', ios: 'ltm virtual VS vip 10.0.0.100 80 pool WEB', vrp: 'ltm virtual VS vip 10.0.0.100 80 pool WEB', comware: 'ltm virtual VS vip 10.0.0.100 80 pool WEB', mode: 'Config' },
  { feature: '查看 LTM', topic: 'ltm', ios: 'show ltm', vrp: 'display ltm', comware: 'display ltm', mode: 'Exec' },

  // 园区综合
  { feature: 'Trunk 放行 VLAN', topic: 'campus', ios: 'switchport trunk allowed vlan 10,20', vrp: 'port trunk allow-pass vlan 10 20', comware: 'port trunk permit vlan 10 20', mode: 'If' },
  { feature: '配置默认路由', topic: 'campus', ios: 'ip route 0.0.0.0 0.0.0.0 10.0.0.1', vrp: 'ip route-static 0.0.0.0 0 10.0.0.1', comware: 'ip route-static 0.0.0.0 0 10.0.0.1', mode: 'Config' },
  { feature: '查看 NAT 会话', topic: 'campus', ios: 'show ip nat translations', vrp: 'display nat session', comware: 'display nat session', mode: 'Exec' },

  // 厂商对照：链路聚合
  { feature: '成员口加入聚合', topic: 'lag', ios: 'channel-group 1 mode active', vrp: 'eth-trunk 1', comware: 'port link-aggregation group 1', mode: 'If' },
  { feature: '查看聚合状态', topic: 'lag', ios: 'show etherchannel summary', vrp: 'display eth-trunk 1', comware: 'display link-aggregation verbose', mode: 'Exec' },

  // 厂商对照：堆叠
  { feature: '配置堆叠域', topic: 'stack', ios: 'stack domain 10', vrp: 'stack domain 10', comware: 'stack domain 10', mode: 'Config' },
  { feature: '配置成员编号', topic: 'stack', ios: 'stack member 1', vrp: 'stack member 1', comware: 'stack member 1', mode: 'Config' },
  { feature: '配置主设备优先级', topic: 'stack', ios: 'stack priority 200', vrp: 'stack priority 200', comware: 'stack priority 200', mode: 'Config' },
  { feature: '查看堆叠状态', topic: 'stack', ios: 'show stack', vrp: 'display stack', comware: 'display stack', mode: 'Exec' },

  // 厂商对照：跨设备聚合
  { feature: '配置 M-LAG 域', topic: 'mlag', ios: 'mlag domain 1', vrp: 'mlag domain 1', comware: 'mlag domain 1', mode: 'Config' },
  { feature: '指定 peer-link', topic: 'mlag', ios: 'mlag peer-link', vrp: 'mlag peer-link', comware: 'mlag peer-link', mode: 'If' },
  { feature: '查看 M-LAG 状态', topic: 'mlag', ios: 'show mlag', vrp: 'display mlag', comware: 'display mlag', mode: 'Exec' },

  // 厂商对照：首跳冗余（模拟器实现 VRRP）
  { feature: '配置虚拟网关 IP', topic: 'fhrp', ios: 'vrrp 1 ip 10.0.0.1', vrp: 'vrrp vrid 1 virtual-ip 10.0.0.1', comware: 'vrrp vrid 1 virtual-ip 10.0.0.1', mode: 'If' },
  { feature: '配置优先级', topic: 'fhrp', ios: 'vrrp 1 priority 200', vrp: 'vrrp vrid 1 priority 200', comware: 'vrrp vrid 1 priority 200', mode: 'If' },
  { feature: '开启抢占', topic: 'fhrp', ios: 'vrrp 1 preempt', vrp: 'vrrp vrid 1 preempt-mode', comware: 'vrrp vrid 1 preempt-mode', mode: 'If' },
  { feature: '查看主备状态', topic: 'fhrp', ios: 'show vrrp', vrp: 'display vrrp', comware: 'display vrrp', mode: 'Exec' },

  // IA：IPv4 地址
  { feature: '配置接口 IPv4 地址', topic: 'ipv4', ios: 'ip address 192.168.1.1 255.255.255.0', vrp: 'ip address 192.168.1.1 24', comware: 'ip address 192.168.1.1 24', mode: 'If' },
  { feature: '查看接口摘要', topic: 'ipv4', ios: 'show ip interface brief', vrp: 'display ip interface brief', comware: 'display ip interface brief', mode: 'Exec' },
  { feature: '连通性测试', topic: 'ipv4', ios: 'ping 192.168.1.2', vrp: 'ping 192.168.1.2', comware: 'ping 192.168.1.2', mode: 'Exec' },

  // IA：静态路由
  { feature: '配置静态路由', topic: 'routing', ios: 'ip route 192.168.2.0 255.255.255.0 10.0.0.2', vrp: 'ip route-static 192.168.2.0 24 10.0.0.2', comware: 'ip route-static 192.168.2.0 24 10.0.0.2', mode: 'Config' },
  { feature: '配置默认路由', topic: 'routing', ios: 'ip route 0.0.0.0 0.0.0.0 10.0.0.1', vrp: 'ip route-static 0.0.0.0 0 10.0.0.1', comware: 'ip route-static 0.0.0.0 0 10.0.0.1', mode: 'Config' },
  { feature: '查看路由表', topic: 'routing', ios: 'show ip route', vrp: 'display ip routing-table', comware: 'display ip routing-table', mode: 'Exec' },

  // IA：IPv6
  { feature: '配置 IPv6 地址', topic: 'ipv6', ios: 'ipv6 address 2001:db8:10::1/64', vrp: 'ipv6 address 2001:db8:10::1/64', comware: 'ipv6 address 2001:db8:10::1/64', mode: 'If' },
  { feature: 'IPv6 静态路由', topic: 'ipv6', ios: 'ipv6 route 2001:db8:20::/64 2001:db8:10::2', vrp: 'ipv6 route-static 2001:db8:20:: 64 2001:db8:10::2', comware: 'ipv6 route-static 2001:db8:20:: 64 2001:db8:10::2', mode: 'Config' },
  { feature: '查看 IPv6 接口', topic: 'ipv6', ios: 'show ipv6 interface brief', vrp: 'display ipv6 interface brief', comware: 'display ipv6 interface brief', mode: 'Exec' },
  { feature: '查看 IPv6 路由表', topic: 'ipv6', ios: 'show ipv6 route', vrp: 'display ipv6 routing-table', comware: 'display ipv6 routing-table', mode: 'Exec' },

  // IA：网络服务与管理
  { feature: '创建本地管理员', topic: 'services', ios: 'username admin privilege 15 secret Admin@123', vrp: 'local-user admin password cipher Admin@123', comware: 'local-user admin password cipher Admin@123', mode: 'Config' },
  { feature: 'VTY 只允许 SSH', topic: 'services', ios: 'transport input ssh', vrp: 'protocol inbound ssh', comware: 'protocol inbound ssh', mode: 'Line' },
  { feature: '查看接口状态', topic: 'services', ios: 'show ip interface brief', vrp: 'display ip interface brief', comware: 'display ip interface brief', mode: 'Exec' },

  // IE：IS-IS
  { feature: '启动 IS-IS 进程', topic: 'isis', ios: 'router isis', vrp: 'isis 1', comware: 'isis 1', mode: 'Config' },
  { feature: '配置 NET', topic: 'isis', ios: 'net 49.0001.0000.0000.0001.00', vrp: 'network-entity 49.0001.0000.0000.0001.00', comware: 'network-entity 49.0001.0000.0000.0001.00', mode: 'Isis' },
  { feature: '接口启用 IS-IS', topic: 'isis', ios: 'ip router isis', vrp: 'isis enable', comware: 'isis enable', mode: 'If' },
  { feature: '查看 IS-IS 邻居', topic: 'isis', ios: 'show isis neighbors', vrp: 'display isis peer', comware: 'display isis peer', mode: 'Exec' },

  // IE：QoS
  { feature: '广播风暴抑制', topic: 'qos', ios: 'storm-control broadcast level 100', vrp: 'broadcast-suppression 100', comware: 'broadcast-suppression 100', mode: 'If' },
  { feature: '入方向限速', topic: 'qos', ios: 'rate-limit input 1000', vrp: 'qos car inbound cir 1000', comware: 'qos lr inbound cir 1000', mode: 'If' },
  { feature: '查看 QoS 统计', topic: 'qos', ios: 'show storm-control', vrp: 'display qos', comware: 'display qos', mode: 'Exec' },
];
