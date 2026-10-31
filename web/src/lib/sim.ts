// WASM 引擎加载与类型桥（与 Rust 侧 NetSummary 对应，字段为 snake_case）。

import init, { Sim } from '../wasm/net_sim';

export interface PortSummary {
  idx: number;
  admin_up: boolean;
  ip: string | null;
  prefix: number;
  linked: boolean;
}

export interface StaticRouteSummary {
  dst: string;
  len: number;
  nexthop: string;
}

export type DeviceKind = 'Router' | 'Switch' | 'Host';
export type Dialect = 'Ios' | 'Vrp' | 'Comware';

export interface DevSummary {
  id: number;
  name: string;
  kind: DeviceKind;
  dialect: Dialect;
  ports: PortSummary[];
  gw: string | null;
  static_routes: StaticRouteSummary[];
  dhcp_enable?: boolean;
}

export interface LinkSummary {
  id: number;
  up: boolean;
  a: [number, number];
  b: [number, number];
}

export interface NetSummary {
  devices: DevSummary[];
  links: LinkSummary[];
}

export interface EngineStats {
  virtualNs: number;
  events: number;
  inFlight: number;
  dropped: number;
}

let simInstance: Sim | null = null;

export async function loadSim(): Promise<Sim> {
  await init();
  simInstance = new Sim();
  return simInstance;
}

export function getSim(): Sim {
  if (!simInstance) throw new Error('引擎未初始化');
  return simInstance;
}

// ---------- 封装常用调用 ----------

export function dumpSummary(sim: Sim): NetSummary {
  return JSON.parse(sim.dump()) as NetSummary;
}

export function execCommand(sim: Sim, dev: number, line: string): string {
  return sim.exec_command(dev, line);
}

export function promptOf(sim: Sim, dev: number): string {
  return sim.prompt(dev);
}

export function completeOf(sim: Sim, dev: number, line: string): { line: string; options: string[] } {
  return JSON.parse(sim.complete(dev, line)) as { line: string; options: string[] };
}

export function statsOf(sim: Sim): EngineStats {
  return JSON.parse(sim.stats()) as EngineStats;
}

export function portOper(sim: Sim, dev: number, port: number): boolean {
  const s = dumpSummary(sim);
  const d = s.devices.find((x) => x.id === dev);
  if (!d) return false;
  const p = d.ports[port];
  if (!p) return false;
  return p.admin_up && p.linked;
}
