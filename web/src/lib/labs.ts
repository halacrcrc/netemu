// M1 实验定义：声明式拓扑 + 任务 + 校验项。（显示状态在 store.svelte.ts 中）

import type { Sim } from '../wasm/net_sim';
import { getSim, dumpSummary } from './sim';
import { refreshFrom, resetPositions, clearTabs, setSelected, setView, setPosition } from './store.svelte';
import { markDirty } from './persistence';
import { LABS_EN } from '../content/labs-en';
import { lang } from './i18n.svelte';

export interface LabCheckDef {
  label: string;
  kind:
    | { type: 'ping'; from: string; to: string; expect?: boolean }
    | { type: 'sviIp'; dev: string; vlan: number; ip: string }
    | { type: 'accessVlan'; dev: string; port: number; vlan: number }
    | { type: 'trunkAllows'; dev: string; port: number; vlan: number }
    | { type: 'vlanExists'; dev: string; vlan: number }
    | { type: 'staticRoute'; dev: string; net: string }
    | { type: 'portIp'; dev: string; port: number; ip: string }
    | { type: 'routeExists'; dev: string; net: string }
    | { type: 'routeAbsent'; dev: string; net: string }
    | { type: 'mplsLsp'; dev: string; name: string }
    | { type: 'vrrpMaster'; dev: string; vrid: number }
    | { type: 'ospfNeighbors'; dev: string; min: number }
    | { type: 'securityZone'; dev: string; port: number; zone: string }
    | { type: 'ipsecTunnel'; dev: string; name: string }
    | { type: 'vpnConnected'; dev: string }
    | {
        type: 'securityPolicy';
        dev: string;
        name: string;
        srcZone: string;
        dstZone: string;
        permit: boolean;
      }
    | { type: 'rootBridge'; dev: string }
    | { type: 'portRole'; dev: string; port: number; role: 'Designated' | 'Root' | 'Alternate' }
    | {
        type: 'portRoleEither';
        dev: string;
        portA: number;
        roleA: 'Designated' | 'Root' | 'Alternate';
        portB: number;
        roleB: 'Designated' | 'Root' | 'Alternate';
      }
    | { type: 'lagMember'; dev: string; group: number; port: number }
    | { type: 'lagSelected'; dev: string; group: number; count: number }
    | { type: 'mgmtUser'; dev: string; name: string }
    | { type: 'vtyTransport'; dev: string; transport: 'all' | 'ssh' | 'telnet' | 'none' }
    | {
        type: 'login';
        from: string;
        to: string;
        proto: 'telnet' | 'ssh';
        user: string;
        pass: string;
        expect?: boolean;
      };
}

export interface LabDef {
  id: string;
  title: string;
  brief: string;
  tasks: string[];
  checks: LabCheckDef[];
  build: (sim: Sim) => void;
  /** 英文界面文案（运行时合并） */
  en?: { title: string; brief: string; tasks: string[]; checkLabels: string[] };
}

/** 本地化实验文案（EN 缺省回退中文）。 */
export function labTitle(lab: LabDef): string {
  return lang() === 'en' && lab.en ? lab.en.title : lab.title;
}
export function labBrief(lab: LabDef): string {
  return lang() === 'en' && lab.en ? lab.en.brief : lab.brief;
}
export function labTasks(lab: LabDef): string[] {
  return lang() === 'en' && lab.en ? lab.en.tasks : lab.tasks;
}

/** 重置并搭建实验拓扑。 */
export function applyLab(lab: LabDef): void {
  const sim = getSim();
  sim.reset();
  resetPositions();
  clearTabs();
  setSelected(null);
  lab.build(sim);
  setView({ x: 20, y: 30 }, 1);
  const s = dumpSummary(sim);
  refreshFrom(s.devices, s.links);
  markDirty();
}

/** 运行实验检查项，返回逐项结果（调用方负责写入响应式状态）。 */
export function runLabChecks(lab: LabDef): { label: string; passed: boolean; detail: string }[] {
  const sim = getSim();
  const s = JSON.parse(sim.dump()) as { devices: { id: number; name: string }[] };
  const byName = new Map(s.devices.map((d) => [d.name, d.id]));
  const out: { label: string; passed: boolean; detail: string }[] = [];
  const need = (name: string): number => {
    const id = byName.get(name);
    if (id === undefined) throw new Error(`找不到设备 ${name}`);
    return id;
  };
  for (const ck of lab.checks) {
    try {
      if (ck.kind.type === 'ping') {
        const from = need(ck.kind.from);
        const r = JSON.parse(sim.check_ping(from, ck.kind.to)) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed === (ck.kind.expect !== false), detail: r.detail });
      } else if (ck.kind.type === 'sviIp') {
        const r = JSON.parse(sim.check_svi_ip(need(ck.kind.dev), ck.kind.vlan, ck.kind.ip)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'accessVlan') {
        const r = JSON.parse(sim.check_access_vlan(need(ck.kind.dev), ck.kind.port, ck.kind.vlan)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'trunkAllows') {
        const r = JSON.parse(sim.check_trunk_allows(need(ck.kind.dev), ck.kind.port, ck.kind.vlan)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'vlanExists') {
        const r = JSON.parse(sim.check_vlan(need(ck.kind.dev), ck.kind.vlan)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'rootBridge') {
        const r = JSON.parse(sim.check_root_bridge(need(ck.kind.dev))) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'portRoleEither') {
        const r1 = JSON.parse(
          sim.check_port_role(need(ck.kind.dev), ck.kind.portA, ck.kind.roleA)
        ) as { passed: boolean; detail: string };
        const r2 = JSON.parse(
          sim.check_port_role(need(ck.kind.dev), ck.kind.portB, ck.kind.roleB)
        ) as { passed: boolean; detail: string };
        out.push({
          label: ck.label,
          passed: r1.passed || r2.passed,
          detail: `${r1.detail} / ${r2.detail}`,
        });
      } else if (ck.kind.type === 'lagMember') {
        const r = JSON.parse(sim.check_lag_member(need(ck.kind.dev), ck.kind.group, ck.kind.port)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'lagSelected') {
        const r = JSON.parse(sim.check_lag_selected(need(ck.kind.dev), ck.kind.group, ck.kind.count)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'mgmtUser') {
        const r = JSON.parse(sim.check_mgmt_user(need(ck.kind.dev), ck.kind.name)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'vtyTransport') {
        const r = JSON.parse(sim.check_vty_transport(need(ck.kind.dev), ck.kind.transport)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'login') {
        const r = JSON.parse(
          sim.check_login(
            need(ck.kind.from),
            ck.kind.to,
            ck.kind.proto,
            ck.kind.user,
            ck.kind.pass,
            ck.kind.expect !== false
          )
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'portRole') {
        const r = JSON.parse(sim.check_port_role(need(ck.kind.dev), ck.kind.port, ck.kind.role)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'portIp') {
        const r = JSON.parse(sim.check_port_ip(need(ck.kind.dev), ck.kind.port, ck.kind.ip)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'routeExists') {
        const r = JSON.parse(sim.check_route(need(ck.kind.dev), ck.kind.net)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'ipsecTunnel') {
        const r = JSON.parse(sim.check_ipsec_tunnel(need(ck.kind.dev), ck.kind.name)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'vpnConnected') {
        const r = JSON.parse(sim.check_vpn_connected(need(ck.kind.dev))) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'securityZone') {
        const r = JSON.parse(
          sim.check_security_zone(need(ck.kind.dev), ck.kind.port, ck.kind.zone)
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'securityPolicy') {
        const r = JSON.parse(
          sim.check_security_policy(
            need(ck.kind.dev),
            ck.kind.name,
            ck.kind.srcZone,
            ck.kind.dstZone,
            ck.kind.permit
          )
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'vrrpMaster') {
        const r = JSON.parse(sim.check_vrrp_master(need(ck.kind.dev), ck.kind.vrid)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'ospfNeighbors') {
        const r = JSON.parse(sim.check_ospf_neighbors(need(ck.kind.dev), ck.kind.min)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'mplsLsp') {
        const r = JSON.parse(sim.check_mpls_lsp(need(ck.kind.dev), ck.kind.name)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'routeAbsent') {
        const r = JSON.parse(sim.check_route_absent(need(ck.kind.dev), ck.kind.net)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'staticRoute') {
        const r = JSON.parse(sim.check_static_route(need(ck.kind.dev), ck.kind.net)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      }
    } catch (e) {
      out.push({ label: ck.label, passed: false, detail: String(e) });
    }
  }
  return out.map((r, i) => ({
    ...r,
    label: lang() === 'en' && lab.en?.checkLabels[i] ? lab.en.checkLabels[i] : r.label,
  }));
}

// ---------- 实验一：二层办公网（VLAN 隔离） ----------

export const lab1: LabDef = {
  id: 'office-l2',
  title: '实验一 · 二层办公网（VLAN 隔离）',
  brief:
    '两台交换机 Trunk 互联。财务部（VLAN10）两台 PC 跨交换机互通；行政部（VLAN20）与之隔离。',
  tasks: [
    'SW1/SW2：vlan batch 10 20（IOS：vlan 10、vlan 20）',
    'SW1 G0/0/1 接 PC1 → access VLAN10；SW1 G0/0/3 接 PC3 → access VLAN20',
    'SW2 G0/0/2 接 PC2 → access VLAN10',
    '两台交换机的互联口配 Trunk 并放行 VLAN10（华为/华三记得 allow-pass / permit）',
    '从 PC1 ping 192.168.10.12（应通）、ping 192.168.10.13（应不通）',
  ],
  checks: [
    {
      label: 'PC1 → PC2（同 VLAN 跨交换机）连通',
      kind: { type: 'ping', from: 'PC1', to: '192.168.10.12' },
    },
    {
      label: 'PC1 → PC3（跨 VLAN）被隔离',
      kind: { type: 'ping', from: 'PC1', to: '192.168.10.13', expect: false },
    },
    { label: 'SW1 互联口 Trunk 已放行 VLAN10', kind: { type: 'trunkAllows', dev: 'SW1', port: 1, vlan: 10 } },
    { label: 'SW2 互联口 Trunk 已放行 VLAN10', kind: { type: 'trunkAllows', dev: 'SW2', port: 0, vlan: 10 } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const sw1 = sim.add_device('Switch', 'Vrp');
    const sw2 = sim.add_device('Switch', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    const pc3 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(sw1, 'SW1');
    sim.rename_device(sw2, 'SW2');
    sim.rename_device(pc2, 'PC2');
    sim.rename_device(pc3, 'PC3');
    sim.add_link(pc1, sw1); // SW1 port0
    sim.add_link(sw1, sw2); // SW1 port1 — SW2 port0
    sim.add_link(sw2, pc2); // SW2 port1
    sim.add_link(sw1, pc3); // SW1 port2
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.10.12', 24, '');
    sim.set_host_config(pc3, '192.168.10.13', 24, '');
    setPosition(pc1, { x: 40, y: 60 });
    setPosition(sw1, { x: 240, y: 60 });
    setPosition(sw2, { x: 470, y: 60 });
    setPosition(pc2, { x: 670, y: 60 });
    setPosition(pc3, { x: 240, y: 210 });
  },
};

// ---------- 实验二：三层交换机 VLAN 间路由（SVI） ----------

export const lab2: LabDef = {
  id: 'office-l3',
  title: '实验二 · 三层交换机 VLAN 间路由（SVI）',
  brief:
    '一台三层交换机划两个 VLAN，配 Vlanif10 / Vlanif20 网关，实现 VLAN 间路由。体会"VLAN 隔离 + 三层互通"。',
  tasks: [
    'SW1：vlan batch 10 20',
    'G0/0/1 → access VLAN10（接 PC1）；G0/0/2 → access VLAN20（接 PC2）',
    'interface Vlanif10 配 192.168.10.1/24；interface Vlanif20 配 192.168.20.1/24（IOS：interface vlan 10）',
    'PC1 网关 192.168.10.1、PC2 网关 192.168.20.1（在属性面板填）',
    '从 PC1 ping 192.168.20.12 —— 这次应该通了！',
  ],
  checks: [
    { label: 'Vlanif10 = 192.168.10.1', kind: { type: 'sviIp', dev: 'SW1', vlan: 10, ip: '192.168.10.1' } },
    { label: 'Vlanif20 = 192.168.20.1', kind: { type: 'sviIp', dev: 'SW1', vlan: 20, ip: '192.168.20.1' } },
    { label: 'PC1 → PC2 跨 VLAN 连通', kind: { type: 'ping', from: 'PC1', to: '192.168.20.12' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const sw1 = sim.add_device('Switch', 'Comware');
    const pc2 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(sw1, 'SW1');
    sim.rename_device(pc2, 'PC2');
    sim.add_link(pc1, sw1);
    sim.add_link(sw1, pc2);
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.20.12', 24, '');
    setPosition(pc1, { x: 80, y: 90 });
    setPosition(sw1, { x: 320, y: 90 });
    setPosition(pc2, { x: 560, y: 90 });
  },
};

// ---------- 实验三：冗余链路与 STP ----------

export const lab3: LabDef = {
  id: 'stp-redundancy',
  title: '实验三 · 冗余链路与 STP',
  brief:
    '两台交换机之间拉两条链路（成环）。观察 STP 如何自动阻塞一条、故障后如何收敛。抓包里能看到 BPDU。',
  tasks: [
    '加载拓扑后等 30 秒（虚拟时间）让 STP 收敛，在 SW1 执行 show spanning-tree 看角色',
    '两台交换机互联口配 Trunk 并放行 VLAN10（华为：port link-type trunk + allow-pass）',
    '把 SW1 设为根桥：system-view → stp priority 4096（IOS：spanning-tree vlan 1 priority 4096）',
    '选中一条互联链路开抓包，观察 BPDU 报文（每 2 秒一次）',
    '在 SW1 上 shutdown 主链路口，等收敛后 PC1 仍能 ping 通 PC2 —— 冗余生效！',
    '（选做）两台都 stp disable，感受广播风暴：状态栏丢弃数飙升',
  ],
  checks: [
    { label: 'SW1 是根桥（优先级 4096）', kind: { type: 'rootBridge', dev: 'SW1' } },
    {
      label: 'SW2 已收敛：根端口位于某一条互联链路上',
      kind: {
        type: 'portRoleEither',
        dev: 'SW2',
        portA: 0,
        roleA: 'Root',
        portB: 1,
        roleB: 'Root',
      },
    },
    { label: 'PC1 → PC2 经冗余路径连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.12' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const sw1 = sim.add_device('Switch', 'Vrp');
    const sw2 = sim.add_device('Switch', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(sw1, 'SW1');
    sim.rename_device(sw2, 'SW2');
    sim.rename_device(pc2, 'PC2');
    sim.add_link(pc1, sw1); // SW1 p0
    sim.add_link(sw1, sw2); // SW1 p1 — SW2 p0（主链路）
    sim.add_link(sw1, sw2); // SW1 p2 — SW2 p1（备份链路）
    sim.add_link(sw2, pc2); // SW2 p2
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.10.12', 24, '');
    // 拉开两条平行链路的视觉弧度：位置错开
    setPosition(pc1, { x: 40, y: 40 });
    setPosition(sw1, { x: 250, y: 40 });
    setPosition(sw2, { x: 460, y: 40 });
    setPosition(pc2, { x: 670, y: 40 });
  },
};

// ---------- 实验四：企业上网三件套（DHCP + ACL + NAT） ----------

export const lab4: LabDef = {
  id: 'enterprise',
  title: '实验四 · 企业上网（DHCP+ACL+NAT）',
  brief:
    '内网 PC 通过 DHCP 自动获取地址；路由器配置 NAT 上外网；用 ACL 禁止办公网 ping 财务服务器。IA 综合大实验。',
  tasks: [
    'R1：g0/0（内网口）配 192.168.1.1/24，g0/1（外网口）配 202.100.1.1/24，均 no shutdown',
    'R1：ip dhcp pool LAN → network 192.168.1.0 255.255.255.0 → default-router 192.168.1.1',
    'PC1 属性面板开启「DHCP 自动获取」→ 应拿到 192.168.1.2',
    'R1：g0/0 配 ip nat inside、g0/1 配 ip nat outside；show ip nat translations 看转换',
    '（选做）access-list 100 deny icmp 192.168.1.0 0.0.0.255 202.100.1.0 0.0.0.255 + 接口应用，体验拦截',
    'PC1 ping 202.100.1.100 —— 通了！你的流量已经 NAT 成公网地址',
  ],
  checks: [
    {
      label: 'PC1 通过 DHCP 获取到 192.168.1.2',
      kind: { type: 'portIp', dev: 'PC1', port: 0, ip: '192.168.1.2' },
    },
    { label: 'PC1 → 外网服务器连通（NAT 生效）', kind: { type: 'ping', from: 'PC1', to: '202.100.1.100' } },
    { label: 'R1 直连外网网段', kind: { type: 'routeExists', dev: 'R1', net: '202.100.1.0/24' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const r1 = sim.add_device('Router', 'Ios');
    const srv = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(r1, 'R1');
    sim.rename_device(srv, 'SRV');
    sim.add_link(pc1, r1); // R1 p0 inside
    sim.add_link(r1, srv); // R1 p1 outside
    sim.set_host_config(srv, '202.100.1.100', 24, '');
    setPosition(pc1, { x: 60, y: 60 });
    setPosition(r1, { x: 300, y: 60 });
    setPosition(srv, { x: 560, y: 60 });
  },
};

// ---------- 实验五：链路聚合与带宽冗余 ----------

export const lab5: LabDef = {
  id: 'lag-redundancy',
  title: '实验五 · 链路聚合与带宽冗余',
  brief:
    '两台交换机之间拉两条链路，用 LACP 聚合成一个逻辑链路 Eth-Trunk1。STP 只看到一个端口，两条链路同时转发；断一条业务不中断。',
  tasks: [
    'SW1/SW2：system-view → vlan batch 10',
    'SW1 G0/0/1 接 PC1 → port link-type access + port default vlan 10（SW2 的 G0/0/3 接 PC2 同理）',
    '互联口（SW1 G0/0/2、G0/0/3；SW2 G0/0/1、G0/0/2）→ port link-type trunk + port trunk allow-pass vlan 10',
    'SW1/SW2：interface Eth-Trunk 1 创建聚合口；再进两个成员口执行 eth-trunk 1',
    'display eth-trunk 1：应看到 2 个成员状态 Up；PC1 ping 192.168.10.12',
    '选做：成员口 shutdown 一个，再 ping —— 业务不中断，这就是带宽冗余',
  ],
  checks: [
    { label: 'SW1 成员口 G0/0/2 已加入聚合组 1', kind: { type: 'lagMember', dev: 'SW1', group: 1, port: 1 } },
    { label: 'SW1 成员口 G0/0/3 已加入聚合组 1', kind: { type: 'lagMember', dev: 'SW1', group: 1, port: 2 } },
    { label: 'SW1 聚合组 1 两个成员均已选中（LACP Up）', kind: { type: 'lagSelected', dev: 'SW1', group: 1, count: 2 } },
    { label: 'SW2 成员口 G0/0/1 已加入聚合组 1', kind: { type: 'lagMember', dev: 'SW2', group: 1, port: 0 } },
    { label: 'SW2 成员口 G0/0/2 已加入聚合组 1', kind: { type: 'lagMember', dev: 'SW2', group: 1, port: 1 } },
    { label: 'SW2 聚合组 1 两个成员均已选中（LACP Up）', kind: { type: 'lagSelected', dev: 'SW2', group: 1, count: 2 } },
    { label: 'PC1 → PC2 经聚合链路连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.12' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const sw1 = sim.add_device('Switch', 'Vrp');
    const sw2 = sim.add_device('Switch', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(sw1, 'SW1');
    sim.rename_device(sw2, 'SW2');
    sim.rename_device(pc2, 'PC2');
    sim.add_link(pc1, sw1); // SW1 p0
    sim.add_link(sw1, sw2); // SW1 p1 — SW2 p0
    sim.add_link(sw1, sw2); // SW1 p2 — SW2 p1
    sim.add_link(sw2, pc2); // SW2 p2
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.10.12', 24, '');
    setPosition(pc1, { x: 40, y: 40 });
    setPosition(sw1, { x: 250, y: 40 });
    setPosition(sw2, { x: 460, y: 40 });
    setPosition(pc2, { x: 670, y: 40 });
  },
};

// ---------- 实验六：设备管理面加固（SSH / Telnet） ----------

export const lab6: LabDef = {
  id: 'mgmt-ssh',
  title: '实验六 · 设备管理面加固（SSH/Telnet）',
  brief:
    '给路由器配置本地管理员与 VTY 登录，只允许 SSH 接入。从 PC 用 SSH 登录成功；Telnet 被拒绝——管理面加固的第一课。',
  tasks: [
    'R1：enable → configure terminal → interface g0/0 → ip address 192.168.1.1 255.255.255.0 → no shutdown → exit',
    'R1：username admin privilege 15 secret Cisco@123',
    'R1：line vty 0 4 → login local → transport input ssh → exit',
    'PC1 终端：ssh 192.168.1.1 → 用户名 admin、口令 Cisco@123 → 看到 R1# 说明登录成功',
    '登录后执行 show ip interface brief 查看 R1 接口，再 exit 断开',
    'PC1 终端：telnet 192.168.1.1 —— 应提示连接被拒绝（VTY 只开了 SSH）',
  ],
  checks: [
    { label: 'R1 存在本地用户 admin', kind: { type: 'mgmtUser', dev: 'R1', name: 'admin' } },
    { label: 'R1 VTY 仅允许 SSH 接入', kind: { type: 'vtyTransport', dev: 'R1', transport: 'ssh' } },
    {
      label: 'PC1 用 SSH 登录 R1 成功',
      kind: { type: 'login', from: 'PC1', to: '192.168.1.1', proto: 'ssh', user: 'admin', pass: 'Cisco@123' },
    },
    {
      label: 'PC1 的 Telnet 登录被拒绝',
      kind: {
        type: 'login',
        from: 'PC1',
        to: '192.168.1.1',
        proto: 'telnet',
        user: 'admin',
        pass: 'Cisco@123',
        expect: false,
      },
    },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const r1 = sim.add_device('Router', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(r1, 'R1');
    sim.add_link(pc1, r1); // R1 g0/0
    sim.set_host_config(pc1, '192.168.1.10', 24, '192.168.1.1');
    setPosition(pc1, { x: 80, y: 90 });
    setPosition(r1, { x: 400, y: 90 });
  },
};

// ---------- 实验七：BGP 路由策略（route-policy） ----------

export const lab7: LabDef = {
  id: 'bgp-route-policy',
  title: '实验七 · BGP 路由策略（route-policy）',
  brief:
    'R1 通过 BGP 向 R2 宣告两个网段，其中 172.16.9.0/24 是受控业务。用 route-policy + ACL 在 R2 的入方向只过滤这一个前缀，其余路由照常放行。',
  tasks: [
    'R1：G0/0/0 192.168.1.1/24、G0/0/1 10.0.0.1/24、G0/0/2 172.16.9.1/24，全部 undo shutdown',
    'R2：G0/0/0 10.0.0.2/24、G0/0/1 192.168.2.1/24，全部 undo shutdown',
    'R1：bgp 65001 → peer 10.0.0.2 as-number 65002 → network 192.168.1.0 255.255.255.0 → network 172.16.9.0 255.255.255.0',
    'R2：bgp 65002 → peer 10.0.0.1 as-number 65001 → network 192.168.2.0 255.255.255.0 → quit',
    'R2：acl 2000 → rule permit source 172.16.9.0 0.0.0.255 → quit（只匹配受控网段）',
    'R2：route-policy FILTER deny node 10 → if-match acl 2000 → quit',
    'R2：route-policy FILTER permit node 20 → quit',
    'R2：bgp 65002 → peer 10.0.0.1 route-policy FILTER import → quit',
    'R2：display bgp routing-table —— 应只有 192.168.1.0/24，没有 172.16.9.0/24（IOS：route-map + match ip address + neighbor ... route-map ... in）',
    'PC2 ping 192.168.1.10 应通；ping 172.16.9.10 应不通（被策略过滤）',
  ],
  checks: [
    {
      label: 'R2 学到 R1 的 192.168.1.0/24（BGP 路由）',
      kind: { type: 'routeExists', dev: 'R2', net: '192.168.1.0/24' },
    },
    {
      label: 'R2 未学到受控网段 172.16.9.0/24（策略过滤）',
      kind: { type: 'routeAbsent', dev: 'R2', net: '172.16.9.0/24' },
    },
    {
      label: 'R1 学到 R2 的 192.168.2.0/24（回程路由存在）',
      kind: { type: 'routeExists', dev: 'R1', net: '192.168.2.0/24' },
    },
    { label: 'PC2 → PC1（192.168.1.10）连通', kind: { type: 'ping', from: 'PC2', to: '192.168.1.10' } },
    {
      label: 'PC2 → 受控服务器（172.16.9.10）不通',
      kind: { type: 'ping', from: 'PC2', to: '172.16.9.10', expect: false },
    },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const r1 = sim.add_device('Router', 'Vrp');
    const r2 = sim.add_device('Router', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    const srv = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(r1, 'R1');
    sim.rename_device(r2, 'R2');
    sim.rename_device(pc2, 'PC2');
    sim.rename_device(srv, 'SRV');
    sim.add_link(pc1, r1); // R1 G0/0/0
    sim.add_link(r1, r2); // R1 G0/0/1 — R2 G0/0/0
    sim.add_link(r2, pc2); // R2 G0/0/1
    sim.add_link(r1, srv); // R1 G0/0/2
    sim.set_host_config(pc1, '192.168.1.10', 24, '192.168.1.1');
    sim.set_host_config(pc2, '192.168.2.10', 24, '192.168.2.1');
    sim.set_host_config(srv, '172.16.9.10', 24, '172.16.9.1');
    setPosition(pc1, { x: 40, y: 40 });
    setPosition(r1, { x: 250, y: 110 });
    setPosition(srv, { x: 250, y: 250 });
    setPosition(r2, { x: 480, y: 110 });
    setPosition(pc2, { x: 690, y: 110 });
  },
};

// ---------- 实验八：MPLS L3VPN（VRF + 静态 LSP） ----------

export const lab8: LabDef = {
  id: 'mpls-l3vpn',
  title: '实验八 · MPLS L3VPN（VRF + 静态 LSP）',
  brief:
    '两家客户站点通过运营商 MPLS 骨干互联。PE 上用 VRF 隔离客户路由，入口压入“传输标签 + VPN 标签”，P 只换外层标签，出口 PE 按 VPN 标签把报文投递回客户 VRF。',
  tasks: [
    'PE1：system-view → ip vpn-instance CUST1（注意不要 quit）→ interface g0/0/0 → ip binding vpn-instance CUST1 → ip address 192.168.1.1 24 → undo shutdown → quit',
    'PE1：interface g0/0/1 → ip address 10.0.0.1 24 → undo shutdown → quit',
    'PE1：static-lsp ingress to-ce2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100 vrf CUST1 vpn-label 1001',
    'PE1 回程：static-lsp egress trans400 incoming-label 400 → static-lsp egress vpn1002 incoming-label 1002 vrf CUST1',
    'P：G0/0/0 10.0.0.2/24、G0/0/1 10.0.1.1/24；static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200',
    'P：static-lsp transit t2 incoming-label 300 nexthop 10.0.0.1 out-label 400',
    'PE2：ip vpn-instance CUST1 → g0/0/0 10.0.1.2/24、g0/0/1 绑定 CUST1 + 192.168.2.1/24',
    'PE2：static-lsp egress trans200 incoming-label 200 → static-lsp egress vpn1001 incoming-label 1001 vrf CUST1',
    'PE2 回程：static-lsp ingress to-ce1 destination 192.168.1.0 24 nexthop 10.0.1.1 out-label 300 vrf CUST1 vpn-label 1002',
    'display mpls lsp 应看到 VRF=CUST1 与 VPN 标签；CE1 ping 192.168.2.10（IOS：mpls static-lsp … 同结构）',
  ],
  checks: [
    { label: 'PE1 已配置去程 VPN LSP to-ce2', kind: { type: 'mplsLsp', dev: 'PE1', name: 'to-ce2' } },
    { label: 'PE2 已配置 VPN 标签 1001 的 VRF 映射', kind: { type: 'mplsLsp', dev: 'PE2', name: 'vpn1001' } },
    { label: 'CE1 → CE2 跨 MPLS 骨干连通', kind: { type: 'ping', from: 'CE1', to: '192.168.2.10' } },
    { label: 'CE2 → CE1 回程连通', kind: { type: 'ping', from: 'CE2', to: '192.168.1.10' } },
  ],
  build: (sim) => {
    const ce1 = sim.add_device('Host', 'Ios');
    const pe1 = sim.add_device('Router', 'Vrp');
    const p = sim.add_device('Router', 'Vrp');
    const pe2 = sim.add_device('Router', 'Vrp');
    const ce2 = sim.add_device('Host', 'Ios');
    sim.rename_device(ce1, 'CE1');
    sim.rename_device(pe1, 'PE1');
    sim.rename_device(p, 'P');
    sim.rename_device(pe2, 'PE2');
    sim.rename_device(ce2, 'CE2');
    sim.add_link(ce1, pe1); // PE1 g0/0/0
    sim.add_link(pe1, p); // PE1 g0/0/1 — P g0/0/0
    sim.add_link(p, pe2); // P g0/0/1 — PE2 g0/0/0
    sim.add_link(pe2, ce2); // PE2 g0/0/1
    sim.set_host_config(ce1, '192.168.1.10', 24, '192.168.1.1');
    sim.set_host_config(ce2, '192.168.2.10', 24, '192.168.2.1');
    setPosition(ce1, { x: 40, y: 100 });
    setPosition(pe1, { x: 230, y: 100 });
    setPosition(p, { x: 440, y: 100 });
    setPosition(pe2, { x: 650, y: 100 });
    setPosition(ce2, { x: 840, y: 100 });
  },
};

// ---------- 实验九：大型园区综合实验 ----------

export const lab9: LabDef = {
  id: 'campus-capstone',
  title: '实验九 · 大型园区综合实验（VLAN/LACP/OSPF/VRRP/NAT）',
  brief:
    '两栋楼各一台接入交换机，双链路 LACP 互联；核心交换机 DSW 做 VLAN 间路由；两台核心路由器做 VRRP 网关与 OSPF，CORE1 负责 NAT 上外网。这是一次把 M2~M5 串起来的园区收官实验。',
  tasks: [
    'SW1：vlan batch 10 20；G0/0/1 access VLAN10（PC1）、G0/0/2 access VLAN20（PC3）；G0/0/3、G0/0/4、G0/0/5 配 trunk 并 allow-pass vlan 10 20',
    'SW2：vlan batch 10 20；G0/0/1、G0/0/2 配 trunk allow-pass vlan 10 20；G0/0/3 access VLAN10（PC2）、G0/0/4 access VLAN20（PC4）',
    'SW1/SW2：成员口（SW1 G0/0/3、G0/0/4；SW2 G0/0/1、G0/0/2）执行 eth-trunk 1 加入聚合组；成员口已配 trunk，聚合口本身无需再配（display eth-trunk 1 应看到 2 个 Up）',
    'DSW：vlan batch 10 20 99；G0/0/1 trunk allow-pass vlan 10 20（接 SW1）；G0/0/2、G0/0/3 access VLAN99（接核心）',
    'DSW：Vlanif10 192.168.10.1/24、Vlanif20 192.168.20.1/24、Vlanif99 10.0.0.2/24；ip route-static 0.0.0.0 0 10.0.0.1（网关指向 VRRP VIP）',
    'CORE1（IOS）：g0/0 10.0.0.10/24 + vrrp 1 ip 10.0.0.1 + priority 200 + preempt + ip nat inside；g0/1 10.1.0.1/30；g0/2 203.0.113.1/24 + ip nat outside',
    'CORE1：ip route 192.168.0.0 255.255.0.0 10.0.0.2；router ospf 1 → network 10.1.0.0 0.0.0.3 area 0',
    'CORE2（IOS）：g0/0 10.0.0.11/24 + vrrp 1 ip 10.0.0.1；g0/1 10.1.0.2/30；router ospf 1 → network 10.1.0.0 0.0.0.3 area 0',
    '验证：display vrrp（CORE1 Master）、show ip ospf neighbor（FULL）、display eth-trunk 1（2 Up）；PC1 ping PC2/PC3/203.0.113.10',
  ],
  checks: [
    { label: 'SW1 聚合组 1 两个成员已选中', kind: { type: 'lagSelected', dev: 'SW1', group: 1, count: 2 } },
    { label: 'DSW Vlanif10 = 192.168.10.1', kind: { type: 'sviIp', dev: 'DSW', vlan: 10, ip: '192.168.10.1' } },
    { label: 'DSW Vlanif20 = 192.168.20.1', kind: { type: 'sviIp', dev: 'DSW', vlan: 20, ip: '192.168.20.1' } },
    { label: 'CORE1 是 VRRP 组 1 的 Master', kind: { type: 'vrrpMaster', dev: 'CORE1', vrid: 1 } },
    { label: 'CORE1 与 CORE2 建立 OSPF 邻居', kind: { type: 'ospfNeighbors', dev: 'CORE1', min: 1 } },
    { label: 'PC1 → PC2（同 VLAN 跨楼）连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.12' } },
    { label: 'PC1 → PC3（跨 VLAN 经 SVI）连通', kind: { type: 'ping', from: 'PC1', to: '192.168.20.21' } },
    { label: 'PC1 → 外网服务器（NAT）连通', kind: { type: 'ping', from: 'PC1', to: '203.0.113.10' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const pc3 = sim.add_device('Host', 'Ios');
    const sw1 = sim.add_device('Switch', 'Vrp');
    const sw2 = sim.add_device('Switch', 'Vrp');
    const dsw = sim.add_device('Switch', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    const pc4 = sim.add_device('Host', 'Ios');
    const core1 = sim.add_device('Router', 'Ios');
    const core2 = sim.add_device('Router', 'Ios');
    const srv = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(pc3, 'PC3');
    sim.rename_device(sw1, 'SW1');
    sim.rename_device(sw2, 'SW2');
    sim.rename_device(dsw, 'DSW');
    sim.rename_device(pc2, 'PC2');
    sim.rename_device(pc4, 'PC4');
    sim.rename_device(core1, 'CORE1');
    sim.rename_device(core2, 'CORE2');
    sim.rename_device(srv, 'SRV');
    sim.add_link(pc1, sw1); // SW1 p0
    sim.add_link(pc3, sw1); // SW1 p1
    sim.add_link(sw1, sw2); // SW1 p2 / SW2 p0
    sim.add_link(sw1, sw2); // SW1 p3 / SW2 p1
    sim.add_link(sw1, dsw); // SW1 p4 / DSW p0
    sim.add_link(sw2, pc2); // SW2 p2
    sim.add_link(sw2, pc4); // SW2 p3
    sim.add_link(dsw, core1); // DSW p1 / CORE1 p0
    sim.add_link(dsw, core2); // DSW p2 / CORE2 p0
    sim.add_link(core1, core2); // CORE1 p1 / CORE2 p1
    sim.add_link(core1, srv); // CORE1 p2
    sim.set_host_config(pc1, '192.168.10.11', 24, '192.168.10.1');
    sim.set_host_config(pc2, '192.168.10.12', 24, '192.168.10.1');
    sim.set_host_config(pc3, '192.168.20.21', 24, '192.168.20.1');
    sim.set_host_config(pc4, '192.168.20.22', 24, '192.168.20.1');
    sim.set_host_config(srv, '203.0.113.10', 24, '');
    setPosition(pc1, { x: 40, y: 40 });
    setPosition(pc3, { x: 40, y: 160 });
    setPosition(sw1, { x: 210, y: 100 });
    setPosition(sw2, { x: 450, y: 100 });
    setPosition(pc2, { x: 620, y: 40 });
    setPosition(pc4, { x: 620, y: 160 });
    setPosition(dsw, { x: 330, y: 240 });
    setPosition(core1, { x: 230, y: 370 });
    setPosition(core2, { x: 470, y: 370 });
    setPosition(srv, { x: 70, y: 470 });
  },
};

// ---------- 实验十：防火墙安全区域与策略 ----------

export const lab10: LabDef = {
  id: 'firewall-zone',
  title: '实验十 · 防火墙安全区域与策略',
  brief:
    '内网 PC 通过防火墙访问外网服务器。把两个接口分别划入 trust / untrust 区域，只放行 trust→untrust 的流量；untrust 主动访问内网被默认拒绝，回包由状态会话放行。',
  tasks: [
    'FW1：system-view → interface g0/0/0 → ip address 192.168.1.1 24 → undo shutdown → quit',
    'FW1：interface g0/0/1 → ip address 203.0.113.1 24 → undo shutdown → quit',
    'FW1：firewall zone trust → add interface g0/0/0 → quit（IOS：zone security INSIDE + 接口 zone-member security INSIDE）',
    'FW1：firewall zone untrust → add interface g0/0/1 → quit',
    'FW1：security-policy → rule name allow-out → source-zone trust → destination-zone untrust → action permit → quit → quit',
    'FW1：display firewall zone、display security-policy 检查区域与规则',
    'SRV → PC1 先 ping 应不通（untrust 主动入内网被默认拒绝）；PC1 → SRV ping 应通，回包由状态会话放行',
  ],
  checks: [
    { label: 'FW1 G0/0/0 已划入 trust 区域', kind: { type: 'securityZone', dev: 'FW1', port: 0, zone: 'trust' } },
    { label: 'FW1 G0/0/1 已划入 untrust 区域', kind: { type: 'securityZone', dev: 'FW1', port: 1, zone: 'untrust' } },
    {
      label: '存在 trust → untrust permit 策略',
      kind: {
        type: 'securityPolicy',
        dev: 'FW1',
        name: 'allow-out',
        srcZone: 'trust',
        dstZone: 'untrust',
        permit: true,
      },
    },
    { label: 'SRV → PC1 被默认拒绝', kind: { type: 'ping', from: 'SRV', to: '192.168.1.10', expect: false } },
    { label: 'PC1 → SRV 放行（状态化回包）', kind: { type: 'ping', from: 'PC1', to: '203.0.113.10' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const fw1 = sim.add_device('Firewall', 'Vrp');
    const srv = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(fw1, 'FW1');
    sim.rename_device(srv, 'SRV');
    sim.add_link(pc1, fw1); // FW1 p0
    sim.add_link(fw1, srv); // FW1 p1
    sim.set_host_config(pc1, '192.168.1.10', 24, '192.168.1.1');
    sim.set_host_config(srv, '203.0.113.10', 24, '203.0.113.1');
    setPosition(pc1, { x: 60, y: 100 });
    setPosition(fw1, { x: 330, y: 100 });
    setPosition(srv, { x: 620, y: 100 });
  },
};

// ---------- 实验十一：IPsec 站点到站点 VPN ----------

export const lab11: LabDef = {
  id: 'ipsec-site',
  title: '实验十一 · IPsec 站点到站点 VPN',
  brief:
    '两个分支机构各有一台路由器，通过公网互联。配置 IPsec 隧道把 192.168.1.0/24 与 192.168.2.0/24 的流量封装到 10.0.0.0/30 公网链路上，实现站点私网互通。',
  tasks: [
    'R1：G0/0/0 192.168.1.1/24、G0/0/1 10.0.0.1/30，均 undo shutdown',
    'R2：G0/0/0 10.0.0.2/30、G0/0/1 192.168.2.1/24，均 undo shutdown',
    'R1：ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2 → local-subnet 192.168.1.0 24 → remote-subnet 192.168.2.0 24 → quit',
    'R2：ipsec tunnel T1 local 10.0.0.2 remote 10.0.0.1 → local-subnet 192.168.2.0 24 → remote-subnet 192.168.1.0 24 → quit',
    'display ipsec tunnel 应看到两端保护子网；PC1 ping 192.168.2.10 应通',
    '（IOS：ipsec tunnel T1 local … remote … 同一套简化命令）',
  ],
  checks: [
    { label: 'R1 已配置 IPsec 隧道 T1', kind: { type: 'ipsecTunnel', dev: 'R1', name: 'T1' } },
    { label: 'R2 已配置 IPsec 隧道 T1', kind: { type: 'ipsecTunnel', dev: 'R2', name: 'T1' } },
    { label: 'PC1 → PC2 经 IPsec 隧道连通', kind: { type: 'ping', from: 'PC1', to: '192.168.2.10' } },
    { label: 'PC2 → PC1 回程连通', kind: { type: 'ping', from: 'PC2', to: '192.168.1.10' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const r1 = sim.add_device('Router', 'Vrp');
    const r2 = sim.add_device('Router', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(r1, 'R1');
    sim.rename_device(r2, 'R2');
    sim.rename_device(pc2, 'PC2');
    sim.add_link(pc1, r1);
    sim.add_link(r1, r2);
    sim.add_link(r2, pc2);
    sim.set_host_config(pc1, '192.168.1.10', 24, '192.168.1.1');
    sim.set_host_config(pc2, '192.168.2.10', 24, '192.168.2.1');
    setPosition(pc1, { x: 50, y: 100 });
    setPosition(r1, { x: 250, y: 100 });
    setPosition(r2, { x: 470, y: 100 });
    setPosition(pc2, { x: 680, y: 100 });
  },
};

// ---------- 实验十二：SSL VPN 远程接入 ----------

export const lab12: LabDef = {
  id: 'sslvpn-remote',
  title: '实验十二 · SSL VPN 远程接入',
  brief:
    '外网 PC 通过 SSL VPN 网关认证后获得虚拟 IP，再访问内网服务器。防火墙默认拒绝外网主动访问内网，SSL VPN 隧道是受控的远程接入通道。',
  tasks: [
    'FW1：G0/0/0（外网）203.0.113.1/24、G0/0/1（内网）192.168.1.1/24，均 undo shutdown',
    'FW1：firewall zone untrust → add interface g0/0/0 → quit；firewall zone trust → add interface g0/0/1 → quit',
    'FW1：aaa → local-user admin password cipher Admin@123 → quit',
    'FW1：sslvpn virtual-ip 10.10.0.0 24',
    'PC1：sslvpn connect 203.0.113.1 admin Admin@123 —— 应获得虚拟 IP 10.10.0.2',
    'FW1：display sslvpn session 应看到虚拟 IP 与客户端真实 IP',
    'PC1 ping 192.168.1.10 —— 经 SSL VPN 隧道应通（未连接前不通）',
  ],
  checks: [
    { label: 'PC1 已建立 SSL VPN 会话', kind: { type: 'vpnConnected', dev: 'PC1' } },
    { label: 'PC1 → 内网服务器经 VPN 连通', kind: { type: 'ping', from: 'PC1', to: '192.168.1.10' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const fw1 = sim.add_device('Firewall', 'Vrp');
    const srv = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(fw1, 'FW1');
    sim.rename_device(srv, 'SRV');
    sim.add_link(pc1, fw1); // FW1 p0 outside
    sim.add_link(fw1, srv); // FW1 p1 inside
    sim.set_host_config(pc1, '203.0.113.10', 24, '203.0.113.1');
    sim.set_host_config(srv, '192.168.1.10', 24, '192.168.1.1');
    setPosition(pc1, { x: 60, y: 100 });
    setPosition(fw1, { x: 330, y: 100 });
    setPosition(srv, { x: 620, y: 100 });
  },
};

export const LABS: LabDef[] = [lab1, lab2, lab3, lab4, lab5, lab6, lab7, lab8, lab9, lab10, lab11, lab12];

// 合并英文文案
for (const l of LABS) {
  l.en = LABS_EN[l.id];
}
