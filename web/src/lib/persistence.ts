// 工作区持久化：localStorage 自动保存 + 文件导出/导入。

import { getSim, dumpSummary, type NetSummary } from './sim';
import { app } from './store.svelte';

const KEY = 'netemu.workspace.v1';

export interface Workspace {
  net: NetSummary;
  editor: {
    positions: Record<number, { x: number; y: number }>;
    pan: { x: number; y: number };
    zoom: number;
    tabs: number[];
  };
}

let saveTimer: ReturnType<typeof setTimeout> | null = null;

/** 收集当前拓扑 + 编辑器状态（保存/导出/分享共用）。 */
export function currentWorkspace(): Workspace {
  const sim = getSim();
  return {
    net: dumpSummary(sim),
    editor: {
      positions: JSON.parse(JSON.stringify(app.positions)),
      pan: { ...app.pan },
      zoom: app.zoom,
      tabs: [...app.tabs],
    },
  };
}

/** 任何状态变化后调用；防抖落盘。 */
export function markDirty() {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try {
      localStorage.setItem(KEY, JSON.stringify(currentWorkspace()));
    } catch (e) {
      console.warn('自动保存失败', e);
    }
  }, 400);
}

/** 归一化解析出的工作区：畸形/缺失的 editor 字段补默认（旧版本数据、损坏存档、手工篡改）。 */
export function normalizeWorkspace(ws: Workspace): Workspace {
  // 运行时数据不保证形状（JSON.parse 的返回没有编辑器保证），防御性读取
  const e = (ws.editor ?? {}) as Partial<Workspace['editor']>;
  const positions: Record<number, { x: number; y: number }> = {};
  if (e.positions && typeof e.positions === 'object') {
    for (const [k, v] of Object.entries(e.positions)) {
      if (
        v &&
        typeof v === 'object' &&
        Number.isFinite((v as { x?: unknown }).x as number) &&
        Number.isFinite((v as { y?: unknown }).y as number)
      ) {
        positions[Number(k)] = {
          x: (v as { x: number }).x,
          y: (v as { y: number }).y,
        };
      }
    }
  }
  const pan =
    e.pan && Number.isFinite(e.pan.x) && Number.isFinite(e.pan.y)
      ? { x: e.pan.x, y: e.pan.y }
      : { x: 60, y: 50 };
  const zoom = Number.isFinite(e.zoom) && (e.zoom as number) > 0 ? (e.zoom as number) : 1;
  const tabs = Array.isArray(e.tabs) ? e.tabs.filter((n) => Number.isFinite(n)) : [];
  return { net: ws.net, editor: { positions, pan, zoom, tabs } };
}

export function loadWorkspace(): Workspace | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const ws = JSON.parse(raw) as Workspace;
    if (!ws.net || !Array.isArray(ws.net.devices)) return null;
    return normalizeWorkspace(ws);
  } catch {
    return null;
  }
}

export function clearWorkspace() {
  localStorage.removeItem(KEY);
}

export function exportFile() {
  const ws = currentWorkspace();
  const blob = new Blob([JSON.stringify(ws, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `拓扑-${new Date().toISOString().slice(0, 10)}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

export function importFile(file: File): Promise<Workspace> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const ws = JSON.parse(String(reader.result)) as Workspace;
        if (!ws.net || !Array.isArray(ws.net.devices)) throw new Error('格式不对');
        resolve(normalizeWorkspace(ws));
      } catch (e) {
        reject(e);
      }
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}
