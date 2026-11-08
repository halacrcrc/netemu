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
    | { type: 'wlanApJoined'; ac: string; ap: string }
    | { type: 'wlanSsid'; ac: string; ssid: string }
    | { type: 'wifiConnected'; dev: string }
    | { type: 'wifiOnAp'; dev: string; ap: string }
    | { type: 'vxlanVni'; dev: string; vlan: number; vni: number }
    | { type: 'stackMember'; dev: string; domain: number; member: number }
    | { type: 'mlagDomain'; dev: string; domain: number }
    | { type: 'evpnRoute'; dev: string; vtep: string; vni: number }
    | { type: 'ltmVirtual'; dev: string; name: string; vip: string; port: number }
    | { type: 'tcpProbe'; from: string; to: string; port: number; expect?: boolean }
    | { type: 'portIsolate'; dev: string; port: number; group: number }
    | { type: 'muxRole'; dev: string; port: number; role: 'principal' | 'separate' | 'group'; group?: number }
    | { type: 'ospfv3Neighbors'; dev: string; min: number }
    | { type: 'srv6Route'; dev: string; dst: string }
    | { type: 'ping6'; from: string; to: string; expect?: boolean }
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
      } else if (ck.kind.type === 'evpnRoute') {
        const r = JSON.parse(sim.check_evpn_route(need(ck.kind.dev), ck.kind.vtep, ck.kind.vni)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'ltmVirtual') {
        const r = JSON.parse(
          sim.check_ltm_virtual(need(ck.kind.dev), ck.kind.name, ck.kind.vip, ck.kind.port)
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'tcpProbe') {
        const r = JSON.parse(
          sim.check_tcp_probe(
            need(ck.kind.from),
            ck.kind.to,
            ck.kind.port,
            ck.kind.expect !== false
          )
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'portIsolate') {
        const r = JSON.parse(
          sim.check_port_isolate(need(ck.kind.dev), ck.kind.port, ck.kind.group)
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'muxRole') {
        const role = ck.kind.role === 'group' ? String(ck.kind.group ?? 0) : ck.kind.role;
        const r = JSON.parse(
          sim.check_mux_role(need(ck.kind.dev), ck.kind.port, role)
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'ospfv3Neighbors') {
        const r = JSON.parse(
          sim.check_ospfv3_neighbors(need(ck.kind.dev), ck.kind.min)
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'srv6Route') {
        const r = JSON.parse(
          sim.check_srv6_route(need(ck.kind.dev), ck.kind.dst)
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'ping6') {
        const r = JSON.parse(
          sim.check_ping6(need(ck.kind.from), ck.kind.to)
        ) as { passed: boolean; detail: string };
        out.push({
          label: ck.label,
          passed: r.passed === (ck.kind.expect !== false),
          detail: r.detail,
        });
      } else if (ck.kind.type === 'stackMember') {
        const r = JSON.parse(
          sim.check_stack_member(need(ck.kind.dev), ck.kind.domain, ck.kind.member)
        ) as { passed: boolean; detail: string };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'mlagDomain') {
        const r = JSON.parse(sim.check_mlag_domain(need(ck.kind.dev), ck.kind.domain)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'vxlanVni') {
        const r = JSON.parse(sim.check_vxlan_vni(need(ck.kind.dev), ck.kind.vlan, ck.kind.vni)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'wifiOnAp') {
        const r = JSON.parse(sim.check_wifi_on_ap(need(ck.kind.dev), need(ck.kind.ap))) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'wlanApJoined') {
        const r = JSON.parse(sim.check_wlan_ap_joined(need(ck.kind.ac), need(ck.kind.ap))) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'wlanSsid') {
        const r = JSON.parse(sim.check_wlan_ssid(need(ck.kind.ac), ck.kind.ssid)) as {
          passed: boolean;
          detail: string;
        };
        out.push({ label: ck.label, passed: r.passed, detail: r.detail });
      } else if (ck.kind.type === 'wifiConnected') {
        const r = JSON.parse(sim.check_wifi_connected(need(ck.kind.dev))) as {
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

// ---------- 实验十三：无线园区（AC + AP + WPA2） ----------

export const lab13: LabDef = {
  id: 'wlan-capwap',
  title: '实验十三 · 无线园区（AC + AP + CAPWAP + WPA2）',
  brief:
    'AC 通过 CAPWAP 管理 AP：AC 下发 SSID/安全策略/业务 VLAN，AP 注册后把无线终端桥接到有线 VLAN。只有通过 WPA2 口令关联的终端才能访问有线服务器。',
  tasks: [
    'AC1：interface g0/0/0 → ip address 10.0.0.2 24 → undo shutdown → quit',
    'AC1：wlan ssid CORP → wlan security wpa2 psk Admin@123 → wlan vlan 10',
    'SW1：vlan batch 10；G0/0/2 接 AP 配 trunk 并 allow-pass vlan 10；G0/0/3 接 SRV 配 access VLAN10',
    'AP1：G0/0/1 配 trunk 并 allow-pass vlan 10；interface vlanif 1 → ip address 10.0.0.10 24 → quit',
    'AP1：capwap ac 10.0.0.2 —— display wlan 应显示 CORP / Joined',
    'PC1：wifi connect CORP Admin@123 → wifi status；PC2 用错误口令尝试（应失败）',
    'PC1 ping 192.168.10.10 应通；PC2 未关联，ping 应不通',
  ],
  checks: [
    { label: 'AP1 已注册到 AC1', kind: { type: 'wlanApJoined', ac: 'AC1', ap: 'AP1' } },
    { label: 'AC1 已配置 SSID CORP', kind: { type: 'wlanSsid', ac: 'AC1', ssid: 'CORP' } },
    { label: 'PC1 已完成 Wi-Fi 关联', kind: { type: 'wifiConnected', dev: 'PC1' } },
    { label: 'PC1 → 有线服务器连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.10' } },
    {
      label: 'PC2（未关联）无法访问服务器',
      kind: { type: 'ping', from: 'PC2', to: '192.168.10.10', expect: false },
    },
  ],
  build: (sim) => {
    const ac = sim.add_device('WlanController', 'Vrp');
    const sw = sim.add_device('Switch', 'Vrp');
    const ap = sim.add_device('AccessPoint', 'Vrp');
    const pc1 = sim.add_device('Host', 'Ios');
    const pc2 = sim.add_device('Host', 'Ios');
    const srv = sim.add_device('Host', 'Ios');
    sim.rename_device(ac, 'AC1');
    sim.rename_device(sw, 'SW1');
    sim.rename_device(ap, 'AP1');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(pc2, 'PC2');
    sim.rename_device(srv, 'SRV');
    sim.add_link(ac, sw); // SW1 p0 VLAN1
    sim.add_link(sw, ap); // SW1 p1 trunk / AP1 p0
    sim.add_link(ap, pc1); // AP1 p1
    sim.add_link(ap, pc2); // AP1 p2
    sim.add_link(sw, srv); // SW1 p2 access VLAN10
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.10.12', 24, '');
    sim.set_host_config(srv, '192.168.10.10', 24, '');
    setPosition(ac, { x: 60, y: 60 });
    setPosition(sw, { x: 300, y: 150 });
    setPosition(ap, { x: 520, y: 150 });
    setPosition(pc1, { x: 450, y: 300 });
    setPosition(pc2, { x: 620, y: 300 });
    setPosition(srv, { x: 120, y: 300 });
  },
};

// ---------- 实验十四：多 SSID / WPA3 / 漫游 ----------

export const lab14: LabDef = {
  id: 'wlan-advanced',
  title: '实验十四 · 多 SSID / WPA3 / 无线漫游',
  brief:
    'AC 下发两个 SSID：CORP（WPA2、VLAN10）与 GUEST（WPA3-SAE、VLAN20）。两台 AP 都承载这两个 WLAN，客户端按 AP 优先级关联；用 wifi roam 在 AP 间漫游，业务不中断且不同 SSID 保持隔离。',
  tasks: [
    'AC1：interface g0/0/0 → ip address 10.0.0.2 24 → undo shutdown → quit',
    'AC1：wlan ssid CORP → wlan security wpa2 psk Admin@123 → wlan vlan 10',
    'AC1：wlan ssid GUEST → wlan security wpa3 sae Guest@123 → wlan vlan 20',
    'SW1：vlan batch 10 20；G0/0/2、G0/0/3 接 AP1/AP2 配 trunk allow-pass vlan 10 20；G0/0/4 接 SRV10 access VLAN10；G0/0/5 接 SRV20 access VLAN20',
    'AP1/AP2：G0/0/1 配 trunk allow-pass vlan 10 20；Vlanif1 分别 10.0.0.10/24、10.0.0.11/24；wlan priority 200/100；capwap ac 10.0.0.2',
    'PC1：wifi connect CORP Admin@123；PC2：wifi connect GUEST Guest@123（错误口令应失败）',
    'PC1 ping 192.168.10.10、PC2 ping 192.168.20.10 应通；PC1 ping 192.168.20.10 应不通（SSID/VLAN 隔离）',
    'PC1：wifi roam —— 应漫游到 AP2，再次 ping 192.168.10.10 仍通',
  ],
  checks: [
    { label: 'AP1 已注册到 AC1', kind: { type: 'wlanApJoined', ac: 'AC1', ap: 'AP1' } },
    { label: 'AP2 已注册到 AC1', kind: { type: 'wlanApJoined', ac: 'AC1', ap: 'AP2' } },
    { label: 'AC1 同时配置 CORP 与 GUEST', kind: { type: 'wlanSsid', ac: 'AC1', ssid: 'GUEST' } },
    { label: 'PC1 已关联 Wi-Fi', kind: { type: 'wifiConnected', dev: 'PC1' } },
    { label: 'PC2 已关联 Wi-Fi', kind: { type: 'wifiConnected', dev: 'PC2' } },
    { label: 'PC1 漫游到 AP2', kind: { type: 'wifiOnAp', dev: 'PC1', ap: 'AP2' } },
    { label: 'PC1 → CORP 服务器连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.10' } },
    { label: 'PC2 → GUEST 服务器连通', kind: { type: 'ping', from: 'PC2', to: '192.168.20.10' } },
    {
      label: 'CORP → GUEST 跨 SSID 隔离',
      kind: { type: 'ping', from: 'PC1', to: '192.168.20.10', expect: false },
    },
  ],
  build: (sim) => {
    const ac = sim.add_device('WlanController', 'Vrp');
    const sw = sim.add_device('Switch', 'Vrp');
    const ap1 = sim.add_device('AccessPoint', 'Vrp');
    const ap2 = sim.add_device('AccessPoint', 'Vrp');
    const pc1 = sim.add_device('Host', 'Ios');
    const pc2 = sim.add_device('Host', 'Ios');
    const srv10 = sim.add_device('Host', 'Ios');
    const srv20 = sim.add_device('Host', 'Ios');
    sim.rename_device(ac, 'AC1');
    sim.rename_device(sw, 'SW1');
    sim.rename_device(ap1, 'AP1');
    sim.rename_device(ap2, 'AP2');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(pc2, 'PC2');
    sim.rename_device(srv10, 'SRV10');
    sim.rename_device(srv20, 'SRV20');
    sim.add_link(ac, sw); // SW1 p0
    sim.add_link(ap1, sw); // SW1 p1
    sim.add_link(ap2, sw); // SW1 p2
    sim.add_link(sw, srv10); // SW1 p3
    sim.add_link(sw, srv20); // SW1 p4
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.20.22', 24, '');
    sim.set_host_config(srv10, '192.168.10.10', 24, '');
    sim.set_host_config(srv20, '192.168.20.10', 24, '');
    setPosition(ac, { x: 40, y: 40 });
    setPosition(sw, { x: 280, y: 160 });
    setPosition(ap1, { x: 480, y: 60 });
    setPosition(ap2, { x: 480, y: 260 });
    setPosition(pc1, { x: 680, y: 40 });
    setPosition(pc2, { x: 680, y: 280 });
    setPosition(srv10, { x: 100, y: 320 });
    setPosition(srv20, { x: 280, y: 380 });
  },
};

// ---------- 实验十五：VXLAN 数据中心跨 DC 二层互通 ----------

export const lab15: LabDef = {
  id: 'vxlan-dc',
  title: '实验十五 · VXLAN 数据中心跨 DC 二层互通',
  brief:
    '两台数据中心交换机作为 VTEP，通过 VXLAN 把本地 VLAN10 映射到 VNI 10000，在 L3 Underlay 上实现跨 DC 的二层延伸。H1/H2 同网段但位于不同机房，仍可直接互通。',
  tasks: [
    'DSW1：vlan batch 10 100；G0/0/1 接 H1 access VLAN10；G0/0/2 接 DSW2 access VLAN100',
    'DSW1：interface vlanif 100 → ip address 10.0.0.1 24 → undo shutdown',
    'DSW1：vxlan local 10.0.0.1 → vxlan vni 10000 vlan 10 → vxlan peer 10.0.0.2',
    'DSW2：vlan batch 10 100；G0/0/1 接 DSW1 access VLAN100；G0/0/2 接 H2 access VLAN10',
    'DSW2：interface vlanif 100 → ip address 10.0.0.2 24 → undo shutdown',
    'DSW2：vxlan local 10.0.0.2 → vxlan vni 10000 vlan 10 → vxlan peer 10.0.0.1',
    'display vxlan 应看到 VNI 10000 <-> VLAN10 与对端 VTEP；H1 ping 192.168.10.20 应通',
    '（抓包可见 VXLAN 外层报文，内层为原始以太帧）',
  ],
  checks: [
    { label: 'DSW1 配置 VNI 10000 ↔ VLAN10', kind: { type: 'vxlanVni', dev: 'DSW1', vlan: 10, vni: 10000 } },
    { label: 'DSW2 配置 VNI 10000 ↔ VLAN10', kind: { type: 'vxlanVni', dev: 'DSW2', vlan: 10, vni: 10000 } },
    { label: 'H1 → H2 跨 DC 二层互通', kind: { type: 'ping', from: 'H1', to: '192.168.10.20' } },
  ],
  build: (sim) => {
    const h1 = sim.add_device('Host', 'Ios');
    const d1 = sim.add_device('Switch', 'Vrp');
    const d2 = sim.add_device('Switch', 'Vrp');
    const h2 = sim.add_device('Host', 'Ios');
    sim.rename_device(h1, 'H1');
    sim.rename_device(d1, 'DSW1');
    sim.rename_device(d2, 'DSW2');
    sim.rename_device(h2, 'H2');
    sim.add_link(h1, d1); // DSW1 p0
    sim.add_link(d1, d2); // DSW1 p1 / DSW2 p0
    sim.add_link(d2, h2); // DSW2 p1
    sim.set_host_config(h1, '192.168.10.10', 24, '');
    sim.set_host_config(h2, '192.168.10.20', 24, '');
    setPosition(h1, { x: 60, y: 90 });
    setPosition(d1, { x: 270, y: 90 });
    setPosition(d2, { x: 500, y: 90 });
    setPosition(h2, { x: 710, y: 90 });
  },
};

// ---------- 实验十六：堆叠 / IRF ----------

export const lab16: LabDef = {
  id: 'stack-irf',
  title: '实验十六 · 堆叠 / IRF（多交换机一台逻辑设备）',
  brief:
    '两台交换机配置同一 stack domain，成为一台逻辑交换机：成员间链路视为内部 fabric，不参与 STP 阻塞；两台成员下的 PC 在同一 VLAN 内互通。',
  tasks: [
    'SW1：stack domain 10 → stack member 1 → stack priority 200',
    'SW2：stack domain 10 → stack member 2 → stack priority 100',
    'SW1/SW2：vlan batch 10；各自接 PC 的口配 access VLAN10',
    'SW1/SW2：互联口配 trunk 并 allow-pass vlan 10',
    'display stack：应看到 domain 10、两个成员，SW1 为 Master',
    'PC1 ping 192.168.10.12 应通（跨堆叠成员二层转发）',
  ],
  checks: [
    { label: 'SW1 是 stack domain 10 的成员 1', kind: { type: 'stackMember', dev: 'SW1', domain: 10, member: 1 } },
    { label: 'SW2 是 stack domain 10 的成员 2', kind: { type: 'stackMember', dev: 'SW2', domain: 10, member: 2 } },
    { label: 'PC1 → PC2 跨堆叠成员连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.12' } },
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
    sim.add_link(sw1, sw2); // SW1 p1 / SW2 p0 stack link
    sim.add_link(sw2, pc2); // SW2 p1
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.10.12', 24, '');
    setPosition(pc1, { x: 60, y: 90 });
    setPosition(sw1, { x: 260, y: 90 });
    setPosition(sw2, { x: 480, y: 90 });
    setPosition(pc2, { x: 680, y: 90 });
  },
};

// ---------- 实验十七：M-LAG 跨设备链路聚合 ----------

export const lab17: LabDef = {
  id: 'mlag-dc',
  title: '实验十七 · M-LAG 跨设备链路聚合',
  brief:
    '两台 DSW 组成 M-LAG 对：共享同一系统 MAC，接入交换机 SW1 用 LACP 双归到两台 DSW，两条链路同时转发；DSW 间 peer-link 负责跨设备转发。',
  tasks: [
    'SW1：vlan batch 10 → stp disable（M-LAG 双归不依赖 STP）',
    'SW1：G0/0/1 接 PC1 access VLAN10；G0/0/2、G0/0/3 接 DSW1/DSW2 配 trunk allow-pass vlan 10',
    'SW1：interface eth-trunk 1 → mode lacp-static → G0/0/2、G0/0/3 分别 eth-trunk 1',
    'DSW1/DSW2：vlan batch 10；mlag domain 1；G0/0/1 接 SW1 trunk allow-pass vlan 10；G0/0/2 互连 trunk allow-pass vlan 10 并 mlag peer-link',
    'DSW1/DSW2：interface eth-trunk 1 → mode lacp-static → G0/0/1 加入 eth-trunk 1',
    'DSW2：G0/0/3 接 PC2 access VLAN10',
    'display mlag 应看到 domain 1 与共享 System MAC；SW1 display eth-trunk 1 应选中 2 个成员',
    'PC1 ping 192.168.10.12 应通（双归链路同时转发）',
  ],
  checks: [
    { label: 'DSW1 配置 M-LAG domain 1', kind: { type: 'mlagDomain', dev: 'DSW1', domain: 1 } },
    { label: 'DSW2 配置 M-LAG domain 1', kind: { type: 'mlagDomain', dev: 'DSW2', domain: 1 } },
    { label: 'SW1 双归 LACP 选中 2 个成员', kind: { type: 'lagSelected', dev: 'SW1', group: 1, count: 2 } },
    { label: 'PC1 → PC2 经 M-LAG 连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.12' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const sw1 = sim.add_device('Switch', 'Vrp');
    const d1 = sim.add_device('Switch', 'Vrp');
    const d2 = sim.add_device('Switch', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(sw1, 'SW1');
    sim.rename_device(d1, 'DSW1');
    sim.rename_device(d2, 'DSW2');
    sim.rename_device(pc2, 'PC2');
    sim.add_link(pc1, sw1); // SW1 p0
    sim.add_link(sw1, d1); // SW1 p1 / DSW1 p0
    sim.add_link(sw1, d2); // SW1 p2 / DSW2 p0
    sim.add_link(d1, d2); // DSW1 p1 / DSW2 p1 peer-link
    sim.add_link(d2, pc2); // DSW2 p2
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.10.12', 24, '');
    setPosition(pc1, { x: 40, y: 60 });
    setPosition(sw1, { x: 230, y: 140 });
    setPosition(d1, { x: 450, y: 50 });
    setPosition(d2, { x: 450, y: 230 });
    setPosition(pc2, { x: 660, y: 230 });
  },
};

// ---------- 实验十八：EVPN 控制面自动发现 VTEP ----------

export const lab18: LabDef = {
  id: 'evpn-dc',
  title: '实验十八 · EVPN 控制面（自动发现 VTEP）',
  brief:
    '两台 VTEP 不再手工指定 vxlan peer，而是通过 EVPN 路由通告（简化 Type-3）互相学习远端 VTEP 与 VNI，数据面自动建立 VXLAN 隧道。',
  tasks: [
    'DSW1/DSW2：按实验十五配置 VLAN10/100、Underlay SVI 10.0.0.1/2、VXLAN local 与 VNI 10000 ↔ VLAN10',
    'DSW1：evpn enable → evpn peer 10.0.0.2（不要再配静态 vxlan peer）',
    'DSW2：evpn enable → evpn peer 10.0.0.1',
    'display evpn：应看到对端 VTEP 与 VNI 10000 路由（由 EVPN 自动学习）',
    'display vxlan：Peers 中应自动出现远端 VTEP',
    'H1 ping 192.168.10.20 应通（数据面自动使用学习到的 VTEP）',
  ],
  checks: [
    { label: 'DSW1 学到 DSW2 的 EVPN 路由', kind: { type: 'evpnRoute', dev: 'DSW1', vtep: '10.0.0.2', vni: 10000 } },
    { label: 'DSW2 学到 DSW1 的 EVPN 路由', kind: { type: 'evpnRoute', dev: 'DSW2', vtep: '10.0.0.1', vni: 10000 } },
    { label: 'H1 → H2 经 EVPN 自动 VTEP 连通', kind: { type: 'ping', from: 'H1', to: '192.168.10.20' } },
  ],
  build: (sim) => {
    const h1 = sim.add_device('Host', 'Ios');
    const d1 = sim.add_device('Switch', 'Vrp');
    const d2 = sim.add_device('Switch', 'Vrp');
    const h2 = sim.add_device('Host', 'Ios');
    sim.rename_device(h1, 'H1');
    sim.rename_device(d1, 'DSW1');
    sim.rename_device(d2, 'DSW2');
    sim.rename_device(h2, 'H2');
    sim.add_link(h1, d1);
    sim.add_link(d1, d2);
    sim.add_link(d2, h2);
    sim.set_host_config(h1, '192.168.10.10', 24, '');
    sim.set_host_config(h2, '192.168.10.20', 24, '');
    setPosition(h1, { x: 60, y: 90 });
    setPosition(d1, { x: 270, y: 90 });
    setPosition(d2, { x: 500, y: 90 });
    setPosition(h2, { x: 710, y: 90 });
  },
};

// ---------- 实验十九：F5 LTM 负载均衡 ----------

export const lab19: LabDef = {
  id: 'f5-ltm',
  title: '实验十九 · F5 LTM 负载均衡',
  brief:
    'F5 负载均衡器对外提供 VIP 10.0.0.100:80，后端池有两台服务器。客户端 TCP 连接 VIP，LTM 轮询选择成员并做目的/源地址转换，回程由 VIP 返回。',
  tasks: [
    'LB1：G0/0/0 接客户端 10.0.0.1/24；G0/0/1 接 SRV1 10.0.1.1/24；G0/0/2 接 SRV2 10.0.2.1/24，均 undo shutdown',
    'LB1：ltm pool WEB member 10.0.1.10 80',
    'LB1：ltm pool WEB member 10.0.2.11 80',
    'LB1：ltm virtual VS vip 10.0.0.100 80 pool WEB',
    'display ltm：应看到 VS → WEB，池内两个成员',
    'PC1：tcp 10.0.0.100 80 连续执行两次，两次都应“连接成功”',
    'display ltm：Sessions 中应出现两条会话，分别指向不同成员（轮询）',
  ],
  checks: [
    {
      label: 'LB1 虚拟服务器 VS = 10.0.0.100:80',
      kind: { type: 'ltmVirtual', dev: 'LB1', name: 'VS', vip: '10.0.0.100', port: 80 },
    },
    { label: 'PC1 第一次连接 VIP 成功', kind: { type: 'tcpProbe', from: 'PC1', to: '10.0.0.100', port: 80 } },
    { label: 'PC1 第二次连接 VIP 成功', kind: { type: 'tcpProbe', from: 'PC1', to: '10.0.0.100', port: 80 } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const lb = sim.add_device('LoadBalancer', 'Vrp');
    const s1 = sim.add_device('Host', 'Ios');
    const s2 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(lb, 'LB1');
    sim.rename_device(s1, 'SRV1');
    sim.rename_device(s2, 'SRV2');
    sim.add_link(pc1, lb); // LB1 p0
    sim.add_link(lb, s1); // LB1 p1
    sim.add_link(lb, s2); // LB1 p2
    sim.set_host_config(pc1, '10.0.0.10', 24, '10.0.0.1');
    sim.set_host_config(s1, '10.0.1.10', 24, '10.0.1.1');
    sim.set_host_config(s2, '10.0.2.11', 24, '10.0.2.1');
    setPosition(pc1, { x: 60, y: 90 });
    setPosition(lb, { x: 300, y: 90 });
    setPosition(s1, { x: 560, y: 30 });
    setPosition(s2, { x: 560, y: 180 });
  },
};

// ---------- 实验二十：MUX VLAN / 端口隔离 ----------

export const lab20: LabDef = {
  id: 'port-isolation',
  title: '实验二十 · MUX VLAN 与端口隔离',
  brief:
    '同一 VLAN 里再做二层隔离：PC1（separate）不能访问 PC2（group），但两者都能访问 PC3（principal）。这就是 MUX VLAN / 私有 VLAN 的核心行为。',
  tasks: [
    'SW1：vlan batch 10；G0/0/1 接 PC1、G0/0/2 接 PC2、G0/0/3 接 PC3，全部 access VLAN10',
    'SW1：G0/0/1 → mux-vlan separate；G0/0/2 → mux-vlan group 1；G0/0/3 → mux-vlan principal',
    '（选做）对 PC1/PC2 接口执行 port-isolate enable group 1，观察同样的隔离效果',
    'display mux-vlan / display port-isolate 检查端口角色',
    'PC1 ping PC2 应不通；PC1 ping PC3、PC2 ping PC3 应通',
  ],
  checks: [
    { label: 'G0/0/1 为 separate 端口', kind: { type: 'muxRole', dev: 'SW1', port: 0, role: 'separate' } },
    { label: 'G0/0/2 为 group 1 端口', kind: { type: 'muxRole', dev: 'SW1', port: 1, role: 'group', group: 1 } },
    { label: 'G0/0/3 为 principal 端口', kind: { type: 'muxRole', dev: 'SW1', port: 2, role: 'principal' } },
    { label: 'PC1 → PC2 被隔离', kind: { type: 'ping', from: 'PC1', to: '192.168.10.12', expect: false } },
    { label: 'PC1 → PC3（principal）连通', kind: { type: 'ping', from: 'PC1', to: '192.168.10.13' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const pc2 = sim.add_device('Host', 'Ios');
    const pc3 = sim.add_device('Host', 'Ios');
    const sw1 = sim.add_device('Switch', 'Vrp');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(pc2, 'PC2');
    sim.rename_device(pc3, 'PC3');
    sim.rename_device(sw1, 'SW1');
    sim.add_link(pc1, sw1); // SW1 p0
    sim.add_link(pc2, sw1); // SW1 p1
    sim.add_link(pc3, sw1); // SW1 p2
    sim.set_host_config(pc1, '192.168.10.11', 24, '');
    sim.set_host_config(pc2, '192.168.10.12', 24, '');
    sim.set_host_config(pc3, '192.168.10.13', 24, '');
    setPosition(pc1, { x: 60, y: 40 });
    setPosition(pc2, { x: 60, y: 160 });
    setPosition(pc3, { x: 60, y: 280 });
    setPosition(sw1, { x: 330, y: 160 });
  },
};

// ---------- 实验二十一：OSPFv3 ----------

export const lab21: LabDef = {
  id: 'ospfv3',
  title: '实验二十一 · OSPFv3 动态路由',
  brief:
    '两台路由器通过 IPv6 链路互联，各自连接一个 IPv6 网段。启用 OSPFv3 后，两端自动学习对端前缀，PC1 与 PC2 可以跨路由器互 ping6。',
  tasks: [
    'R1：g0/0/0 配 2001:db8:1::1/64（接 PC1），g0/0/1 配 2001:db8:12::1/64（接 R2），undo shutdown',
    'R2：g0/0/0 配 2001:db8:12::2/64（接 R1），g0/0/1 配 2001:db8:2::1/64（接 PC2），undo shutdown',
    'R1/R2：ospfv3 1 → router-id 1.1.1.1 / 2.2.2.2',
    'R1/R2：互联口与业务口执行 ospfv3 1 area 0',
    'display ospfv3 peer 应看到邻居；display ipv6 routing-table 应看到对端网段',
    'PC1：ping6 2001:db8:2::10 应通',
  ],
  checks: [
    { label: 'R1 建立 OSPFv3 邻居', kind: { type: 'ospfv3Neighbors', dev: 'R1', min: 1 } },
    { label: 'R2 建立 OSPFv3 邻居', kind: { type: 'ospfv3Neighbors', dev: 'R2', min: 1 } },
    { label: 'PC1 → PC2 跨路由器 ping6 连通', kind: { type: 'ping6', from: 'PC1', to: '2001:db8:2::10' } },
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
    sim.add_link(pc1, r1); // R1 p0
    sim.add_link(r1, r2); // R1 p1 / R2 p0
    sim.add_link(r2, pc2); // R2 p1
    sim.set_host_v6(pc1, '2001:db8:1::10', 64, '2001:db8:1::1');
    sim.set_host_v6(pc2, '2001:db8:2::10', 64, '2001:db8:2::1');
    setPosition(pc1, { x: 40, y: 100 });
    setPosition(r1, { x: 260, y: 100 });
    setPosition(r2, { x: 500, y: 100 });
    setPosition(pc2, { x: 720, y: 100 });
  },
};

// ---------- 实验二十二：SRv6 BE ----------

export const lab22: LabDef = {
  id: 'srv6-be',
  title: '实验二十二 · SRv6 BE 跨网段转发',
  brief:
    'PE1—P—PE2 三台路由器组成 IPv6 骨干（Underlay），两端各接业务网段。PE 上配置 Locator 与本地 SID，把去往对端业务网段的报文封装到远端 SID，报文逐跳穿过骨干，出口 PE 解封装后交付。注意中间的 P 只跑普通 IPv6 转发——不配任何 SRv6，这正是 SRv6 BE 的精髓。',
  tasks: [
    '接口地址（全部 undo shutdown）：PE1 g0/0/0 2001:db8:1::1/64、g0/0/1 2001:db8:12::1/64；P g0/0/0 2001:db8:12::2/64、g0/0/1 2001:db8:23::1/64；PE2 g0/0/0 2001:db8:23::2/64、g0/0/1 2001:db8:2::1/64',
    'PE1：srv6 locator 2001:db8:100::/64；srv6 sid 2001:db8:100::1。PE2：srv6 locator 2001:db8:200::/64；srv6 sid 2001:db8:200::1',
    'SRv6 路由：PE1 srv6 route 2001:db8:2::/64 sid 2001:db8:200::1；PE2 srv6 route 2001:db8:1::/64 sid 2001:db8:100::1',
    'Underlay 静态路由（模拟 IGP 全网贯通）：PE1 ipv6 route-static 2001:db8:200:: 64 2001:db8:12::2 与 2001:db8:23:: 64 2001:db8:12::2；P 2001:db8:200:: 64 2001:db8:23::2 与 2001:db8:100:: 64 2001:db8:12::1；PE2 ipv6 route-static 2001:db8:100:: 64 2001:db8:23::1 与 2001:db8:12:: 64 2001:db8:23::1',
    'P 上不要配置任何 srv6 命令——它只做 IPv6 转发。选中 PE1—P 链路抓包，能看到封装后的 SRv6 报文逐跳前进',
    'display srv6 应看到 Locator、本地 SID 与路由；PC1 ping6 2001:db8:2::10 应通',
  ],
  checks: [
    { label: 'PE1 配置去往业务网段的 SRv6 路由', kind: { type: 'srv6Route', dev: 'PE1', dst: '2001:db8:2::/64' } },
    { label: 'PE2 配置回程 SRv6 路由', kind: { type: 'srv6Route', dev: 'PE2', dst: '2001:db8:1::/64' } },
    { label: 'Underlay IPv6 贯通（PE1 → P）', kind: { type: 'ping6', from: 'PE1', to: '2001:db8:23::1' } },
    { label: 'PC1 → PC2 经 SRv6 穿过骨干连通', kind: { type: 'ping6', from: 'PC1', to: '2001:db8:2::10' } },
  ],
  build: (sim) => {
    const pc1 = sim.add_device('Host', 'Ios');
    const pe1 = sim.add_device('Router', 'Vrp');
    const p = sim.add_device('Router', 'Vrp');
    const pe2 = sim.add_device('Router', 'Vrp');
    const pc2 = sim.add_device('Host', 'Ios');
    sim.rename_device(pc1, 'PC1');
    sim.rename_device(pe1, 'PE1');
    sim.rename_device(p, 'P');
    sim.rename_device(pe2, 'PE2');
    sim.rename_device(pc2, 'PC2');
    sim.add_link(pc1, pe1);
    sim.add_link(pe1, p);
    sim.add_link(p, pe2);
    sim.add_link(pe2, pc2);
    sim.set_host_v6(pc1, '2001:db8:1::10', 64, '2001:db8:1::1');
    sim.set_host_v6(pc2, '2001:db8:2::10', 64, '2001:db8:2::1');
    setPosition(pc1, { x: 40, y: 120 });
    setPosition(pe1, { x: 230, y: 120 });
    setPosition(p, { x: 430, y: 60 });
    setPosition(pe2, { x: 630, y: 120 });
    setPosition(pc2, { x: 820, y: 120 });
  },
};

export const LABS: LabDef[] = [lab1, lab2, lab3, lab4, lab5, lab6, lab7, lab8, lab9, lab10, lab11, lab12, lab13, lab14, lab15, lab16, lab17, lab18, lab19, lab20, lab21, lab22];

// 合并英文文案
for (const l of LABS) {
  l.en = LABS_EN[l.id];
}
