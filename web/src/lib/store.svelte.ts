// 全局应用状态（Svelte 5 runes 模块）。

import type { DevSummary, LinkSummary, EngineStats } from './sim';

export interface NodePos {
  x: number;
  y: number;
}

export const app = $state({
  devices: [] as DevSummary[],
  links: [] as LinkSummary[],
  positions: {} as Record<number, NodePos>,
  /** 当前选中：设备或链路 */
  selected: null as null | { kind: 'dev' | 'link'; id: number },
  /** 连线模式开启 */
  linkMode: false,
  /** 连线模式：已选定出发设备 */
  linkFrom: null as number | null,
  /** 连线时鼠标位置（画布坐标） */
  cursor: null as NodePos | null,
  pan: { x: 60, y: 50 } as NodePos,
  zoom: 1,
  /** 打开的终端标签（设备 id） */
  tabs: [] as number[],
  activeTab: null as number | null,
  stats: { virtualNs: 0, events: 0, inFlight: 0, dropped: 0 } as EngineStats,
  /** 当前激活的实验标题（null = 未在实验模式） */
  labTitle: null as string | null,
  /** 最近一次实验检查结果 */
  labChecks: null as null | { label: string; passed: boolean; detail: string }[],
  /** 实验完成记录：labId → 是否曾全过（持久化于 localStorage） */
  labPassed: {} as Record<string, boolean>,
  /** 顶层视图 */
  viewName: 'lab' as 'lab' | 'quiz' | 'course' | 'notes',
});

const LAB_KEY = 'netemu.labs.v1';
try {
  const raw = localStorage.getItem(LAB_KEY);
  if (raw) app.labPassed = JSON.parse(raw) as Record<string, boolean>;
} catch {
  /* 忽略 */
}

export function markLabPassed(labId: string, allPassed: boolean) {
  if (allPassed) app.labPassed[labId] = true;
  try {
    localStorage.setItem(LAB_KEY, JSON.stringify(app.labPassed));
  } catch {
    /* 忽略 */
  }
}

export function deviceById(id: number): DevSummary | undefined {
  return app.devices.find((d) => d.id === id);
}

export function deviceName(id: number): string {
  return deviceById(id)?.name ?? '?';
}

/** 设备是否至少有一个端口 up（管理开启 + 有链路） */
export function deviceAlive(d: DevSummary): boolean {
  return d.ports.some((p) => p.admin_up && p.linked);
}

/** 端口显示序号（1 起），用于标注。 */
export function portLabel(p: { linked: boolean; ip: string | null }): string {
  if (p.ip) return p.ip;
  return p.linked ? '—' : '';
}

let nextPlace = 0;
export function nextPosition(): NodePos {
  const col = nextPlace % 4;
  const row = Math.floor(nextPlace / 4) % 3;
  nextPlace += 1;
  return { x: 140 + col * 170, y: 110 + row * 130 };
}

/** 用引擎摘要刷新状态；为新增设备安排位置，清理已删除设备的位置/终端/选中。 */
export function refreshFrom(devices: DevSummary[], links: LinkSummary[]) {
  const ids = new Set(devices.map((d) => d.id));
  for (const d of devices) {
    if (!app.positions[d.id]) {
      app.positions[d.id] = nextPosition();
    }
  }
  for (const id of Object.keys(app.positions)) {
    if (!ids.has(Number(id))) delete app.positions[Number(id)];
  }
  app.devices = devices;
  app.links = links;
  app.tabs = app.tabs.filter((id) => ids.has(id));
  if (app.activeTab !== null && !ids.has(app.activeTab)) {
    app.activeTab = app.tabs.length ? app.tabs[app.tabs.length - 1] : null;
  }
  if (app.selected) {
    const ok =
      app.selected.kind === 'dev'
        ? ids.has(app.selected.id)
        : links.some((l) => l.id === app.selected!.id);
    if (!ok) app.selected = null;
  }
}

// ---------- 跨模块修改请使用以下函数（闭包内改动保证响应性） ----------

export function setPosition(id: number, pos: NodePos) {
  app.positions[id] = pos;
}

export function resetPositions() {
  for (const k of Object.keys(app.positions)) delete app.positions[k];
}

export function clearTabs() {
  app.tabs = [];
  app.activeTab = null;
}

export function setView(pan: NodePos, zoom: number) {
  app.pan = pan;
  app.zoom = zoom;
}

export function setSelected(s: null | { kind: 'dev' | 'link'; id: number }) {
  app.selected = s;
}

export function setLabTitle(t: string | null) {
  app.labTitle = t;
}

export function setLabChecks(c: null | { label: string; passed: boolean; detail: string }[]) {
  app.labChecks = c;
}

/** 顶层视图切换：练习场 / 题库 / 课程 / 笔记 */
export function setViewName(name: 'lab' | 'quiz' | 'course' | 'notes') {
  app.viewName = name;
}

export function openTerminal(dev: number) {
  if (!app.tabs.includes(dev)) app.tabs.push(dev);
  app.activeTab = dev;
}

export function closeTerminal(dev: number) {
  const i = app.tabs.indexOf(dev);
  if (i >= 0) app.tabs.splice(i, 1);
  if (app.activeTab === dev) {
    app.activeTab = app.tabs.length ? app.tabs[Math.max(0, i - 1)] : null;
  }
}
