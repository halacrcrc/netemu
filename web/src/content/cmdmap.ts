// 三厂商命令对照表（单一数据源）。
// 每行都会在运行时经 wasm 的 verify_command 校验——保证表格与模拟器行为一致。

import type { TopicId } from './questions';

export type CmdMode = 'Exec' | 'Priv' | 'Config' | 'If' | 'Pool' | 'Acl';

export interface CmdRow {
  /** 功能描述 */
  feature: string;
  topic: TopicId;
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
];
