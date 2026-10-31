// 入口：加载 WASM 引擎 → 恢复工作区 → 挂载应用。

import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { loadSim, dumpSummary } from './lib/sim';
import { app, refreshFrom } from './lib/store.svelte';
import { loadWorkspace, markDirty } from './lib/persistence';

declare global {
  interface Window {
    __netemu?: {
      exec: (dev: number, line: string) => string;
      sim: unknown;
      dump: () => string;
    };
  }
}

async function boot() {
  const sim = await loadSim();
  const ws = loadWorkspace();
  if (ws) {
    try {
      sim.load(JSON.stringify(ws.net));
    } catch (e) {
      // 旧版本工作区格式不兼容：忽略并重新开始
      console.warn('工作区恢复失败，已重置', e);
      localStorage.removeItem('netemu.workspace.v1');
      sim.reset();
    }
    for (const [id, pos] of Object.entries(ws.editor.positions ?? {})) {
      app.positions[Number(id)] = pos;
    }
    app.pan = ws.editor.pan ?? app.pan;
    app.zoom = ws.editor.zoom ?? 1;
    app.tabs = (ws.editor.tabs ?? []).filter((id) =>
      ws.net.devices.some((d) => d.id === id)
    );
    app.activeTab = app.tabs[0] ?? null;
  }
  const s = dumpSummary(sim);
  refreshFrom(s.devices, s.links);

  // 状态变化自动保存（画布拖动/编辑器输入都会触发 markDirty；命令执行在终端里触发）
  setInterval(markDirty, 3000);

  // 测试钩子：与终端 submit 相同的调用链（exec → 引擎静默 → 界面刷新）
  window.__netemu = {
    sim,
    dump: () => JSON.stringify(dumpSummary(sim)),
    exec: (dev: number, line: string) => {
      const out = sim.exec_command(dev, line);
      const s2 = dumpSummary(sim);
      refreshFrom(s2.devices, s2.links);
      markDirty();
      return out;
    },
  };

  mount(App, { target: document.getElementById('app')! });
}

void boot();
