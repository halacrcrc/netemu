// 入口：加载 WASM 引擎 → 恢复工作区 → 挂载应用。

import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { loadSim, dumpSummary } from './lib/sim';
import { app, refreshFrom } from './lib/store.svelte';
import { currentWorkspace, loadWorkspace, markDirty, normalizeWorkspace, type Workspace } from './lib/persistence';
import { buildShareUrl, decodeWorkspace } from './lib/share';

function applyEditor(ws: Workspace) {
  for (const [id, pos] of Object.entries(ws.editor.positions ?? {})) {
    app.positions[Number(id)] = pos;
  }
  app.pan = ws.editor.pan ?? app.pan;
  app.zoom = ws.editor.zoom ?? 1;
  app.tabs = (ws.editor.tabs ?? []).filter((id) => ws.net.devices.some((d) => d.id === id));
  app.activeTab = app.tabs[0] ?? null;
}

declare global {
  interface Window {
    __netemu?: {
      exec: (dev: number, line: string) => string;
      sim: unknown;
      dump: () => string;
      shareUrl: () => Promise<string>;
    };
  }
}

async function boot() {
  const sim = await loadSim();
  let loaded = false;
  // 分享链接优先：URL hash 中的拓扑覆盖本地工作区
  const hash = location.hash;
  if (hash.startsWith('#topo=')) {
    try {
      const ws = normalizeWorkspace(await decodeWorkspace(hash.slice('#topo='.length)));
      sim.load(JSON.stringify(ws.net));
      applyEditor(ws);
      loaded = true;
    } catch (e) {
      console.warn('分享链接解析失败，已忽略', e);
    }
  }
  const ws = loaded ? null : loadWorkspace();
  if (ws) {
    try {
      sim.load(JSON.stringify(ws.net));
    } catch (e) {
      // 旧版本工作区格式不兼容：忽略并重新开始
      console.warn('工作区恢复失败，已重置', e);
      localStorage.removeItem('netemu.workspace.v1');
      sim.reset();
    }
    applyEditor(ws);
  }
  const s = dumpSummary(sim);
  refreshFrom(s.devices, s.links);

  // 状态变化自动保存（画布拖动/编辑器输入都会触发 markDirty；命令执行在终端里触发）
  setInterval(markDirty, 3000);

  // 测试钩子：与终端 submit 相同的调用链（exec → 引擎静默 → 界面刷新）
  window.__netemu = {
    sim,
    dump: () => JSON.stringify(dumpSummary(sim)),
    shareUrl: () => buildShareUrl(currentWorkspace()),
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

void boot().catch((e) => {
  // 兜底：任何启动失败都给出可见错误，而不是永久白屏
  console.error('启动失败', e);
  const el = document.getElementById('app');
  if (el) el.textContent = '应用启动失败，请刷新页面；若反复出现，请清除本站 localStorage 数据。';
});
