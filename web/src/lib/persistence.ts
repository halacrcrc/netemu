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

export function loadWorkspace(): Workspace | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const ws = JSON.parse(raw) as Workspace;
    if (!ws.net || !Array.isArray(ws.net.devices)) return null;
    return ws;
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
        resolve(ws);
      } catch (e) {
        reject(e);
      }
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}
