// M1 实验定义：声明式拓扑 + 任务 + 校验项。（显示状态在 store.svelte.ts 中）

import type { Sim } from '../wasm/net_sim';
import { getSim, dumpSummary } from './sim';
import { refreshFrom, resetPositions, clearTabs, setSelected, setView, setPosition } from './store.svelte';
import { markDirty } from './persistence';

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
    | { type: 'rootBridge'; dev: string }
    | { type: 'portRole'; dev: string; port: number; role: 'Designated' | 'Root' | 'Alternate' }
    | {
        type: 'portRoleEither';
        dev: string;
        portA: number;
        roleA: 'Designated' | 'Root' | 'Alternate';
        portB: number;
        roleB: 'Designated' | 'Root' | 'Alternate';
      };
}

export interface LabDef {
  id: string;
  title: string;
  brief: string;
  tasks: string[];
  checks: LabCheckDef[];
  build: (sim: Sim) => void;
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
  return out;
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

export const LABS: LabDef[] = [lab1, lab2, lab3, lab4];
