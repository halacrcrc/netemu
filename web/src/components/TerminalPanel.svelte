<script lang="ts">
  // 底部终端区：每台设备一个 xterm 标签，行编辑（历史/补全/?帮助/Ctrl+C）。
  import { onMount } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import '@xterm/xterm/css/xterm.css';
  import { getSim, execCommand, promptOf, completeOf, dumpSummary } from '../lib/sim';
  import { app, deviceName, closeTerminal, refreshFrom } from '../lib/store.svelte';
  import { markDirty } from '../lib/persistence';
  import { theme } from '../lib/theme.svelte';
  import { t } from '../lib/i18n.svelte';

  interface Editor {
    term: Terminal;
    fit: FitAddon;
    buffer: string;
    history: string[];
    histIdx: number;
    dev: number;
  }

  const editors = new Map<number, Editor>();
  let bodyEl: HTMLDivElement | undefined = $state();

  // 终端配色随主题：暗「机房夜航」/ 亮「打印纸」
  const TERM_THEMES = {
    dark: {
      background: '#0b1020',
      foreground: '#dbe4ff',
      cursor: '#38bdf8',
      selectionBackground: '#264f78',
      black: '#0b1020',
      red: '#f87171',
      green: '#34d399',
      yellow: '#fbbf24',
      blue: '#60a5fa',
      magenta: '#c084fc',
      cyan: '#38bdf8',
      white: '#dbe4ff',
    },
    light: {
      background: '#f7f6f2',
      foreground: '#33322e',
      cursor: '#1e5fbf',
      selectionBackground: '#cddcf5',
      black: '#3a3730',
      red: '#c0392b',
      green: '#2f7d46',
      yellow: '#a16207',
      blue: '#1d4ed8',
      magenta: '#a21caf',
      cyan: '#0e7490',
      white: '#57534a',
    },
  } as const;

  function refreshSim() {
    const s = dumpSummary(getSim());
    refreshFrom(s.devices, s.links);
    markDirty();
  }

  function writePrompt(ed: Editor) {
    ed.term.write('\r\n' + promptOf(getSim(), ed.dev));
    ed.buffer = '';
  }

  function clearLine(ed: Editor) {
    // 用退格清掉当前输入
    for (let i = 0; i < ed.buffer.length; i++) ed.term.write('\b \b');
    ed.buffer = '';
  }

  function rewrite(ed: Editor, text: string) {
    clearLine(ed);
    ed.buffer = text;
    ed.term.write(text);
  }

  async function submit(ed: Editor) {
    const line = ed.buffer;
    ed.term.write('\r\n');
    ed.buffer = '';
    if (line.trim()) {
      ed.history.push(line);
      if (ed.history.length > 200) ed.history.shift();
    }
    ed.histIdx = ed.history.length;
    if (line.trim()) {
      const out = execCommand(getSim(), ed.dev, line);
      if (out) ed.term.write(out + '\r\n');
      refreshSim();
    }
    ed.term.write(promptOf(getSim(), ed.dev));
  }

  function handleData(ed: Editor, data: string) {
    // 粘贴多字符
    if (data.length > 1 && !data.startsWith('\x1b')) {
      for (const ch of data) {
        if (ch === '\r') {
          void submit(ed);
          return;
        }
        handleChar(ed, ch);
      }
      return;
    }
    handleChar(ed, data);
  }

  function handleChar(ed: Editor, ch: string) {
    switch (ch) {
      case '\r': {
        void submit(ed);
        return;
      }
      case '\x7f': {
        if (ed.buffer.length) {
          ed.buffer = ed.buffer.slice(0, -1);
          ed.term.write('\b \b');
        }
        return;
      }
      case '\x03': {
        ed.term.write('^C');
        writePrompt(ed);
        return;
      }
      case '\t': {
        const r = completeOf(getSim(), ed.dev, ed.buffer);
        if (r.line !== ed.buffer) {
          rewrite(ed, r.line);
        } else if (r.options.length) {
          ed.term.write('\r\n' + r.options.join('   ') + '\r\n');
          ed.term.write(promptOf(getSim(), ed.dev) + ed.buffer);
        }
        return;
      }
      case '?': {
        ed.term.write('?');
        const out = execCommand(getSim(), ed.dev, ed.buffer);
        if (out) ed.term.write('\r\n' + out);
        ed.buffer = ed.buffer.replace(/[?]/g, '').trimEnd();
        ed.term.write('\r\n' + promptOf(getSim(), ed.dev) + ed.buffer);
        return;
      }
      case '\x1b[A': {
        // 上
        if (ed.histIdx > 0) {
          ed.histIdx -= 1;
          rewrite(ed, ed.history[ed.histIdx] ?? '');
        }
        return;
      }
      case '\x1b[B': {
        // 下
        if (ed.histIdx < ed.history.length) {
          ed.histIdx += 1;
          rewrite(ed, ed.history[ed.histIdx] ?? '');
        }
        return;
      }
      default: {
        if (ch.startsWith('\x1b')) return; // 其余控制序列忽略
        if (ch >= ' ') {
          ed.buffer += ch;
          ed.term.write(ch);
        }
      }
    }
  }

  function mountEditor(container: HTMLDivElement, dev: number): Editor {
    const term = new Terminal({
      fontSize: 13,
      fontFamily: "'Maple Mono', Consolas, 'Cascadia Mono', monospace",
      theme: TERM_THEMES[theme.name],
      cursorBlink: true,
      scrollback: 5000,
      convertEol: true,    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(container);
    term.onData((data) => {
      const ed = editors.get(dev);
      if (ed) handleData(ed, data);
    });
    term.write(t('term.welcome'));
    const ed: Editor = { term, fit, buffer: '', history: [], histIdx: 0, dev };
    term.write(promptOf(getSim(), dev));
    return ed;
  }

  $effect(() => {
    // 为每个打开的 tab 挂载终端
    const open = new Set(app.tabs);
    for (const dev of app.tabs) {
      if (!editors.has(dev) && bodyEl) {
        const holder = document.createElement('div');
        holder.className = 'term-holder';
        holder.dataset.dev = String(dev);
        bodyEl.appendChild(holder);
        const ed = mountEditor(holder, dev);
        editors.set(dev, ed);
        queueMicrotask(() => ed.fit.fit());
      }
    }
    // 清理已关闭/已删除的 tab
    for (const [dev, ed] of editors) {
      if (!open.has(dev)) {
        ed.term.dispose();
        ed.term.element?.remove();
        editors.delete(dev);
      }
    }
    // 显示/隐藏
    for (const [dev, ed] of editors) {
      const holder = ed.term.element?.parentElement;
      if (holder) holder.style.display = dev === app.activeTab ? 'block' : 'none';
      if (dev === app.activeTab) queueMicrotask(() => ed.fit.fit());
    }
  });

  $effect(() => {
    // 跟随主题切换已打开终端的配色
    const t = TERM_THEMES[theme.name];
    for (const ed of editors.values()) {
      ed.term.options.theme = t;
    }
  });

  onMount(() => {
    const ro = new ResizeObserver(() => {
      for (const ed of editors.values()) {
        if (ed.dev === app.activeTab) ed.fit.fit();
      }
    });
    if (bodyEl) ro.observe(bodyEl);
    return () => ro.disconnect();
  });
</script>

<div class="terminal-panel">
  <div class="tabbar">
    {#each app.tabs as dev (dev)}
      <button
        class="tab"
        class:active={dev === app.activeTab}
        onclick={() => (app.activeTab = dev)}
      >
        {deviceName(dev)}
        <span class="x" role="button" tabindex="-1" onclick={(e) => { e.stopPropagation(); closeTerminal(dev); }}>×</span>
      </button>
    {/each}
    {#if !app.tabs.length}
      <span class="empty">{t('term.empty')}</span>
    {/if}
  </div>
  <div class="term-body" bind:this={bodyEl}></div>
</div>

<style>
  .terminal-panel {
    height: 260px;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
    background: var(--bg-panel-2);
  }
  .tabbar {
    display: flex;
    gap: 4px;
    padding: 4px 8px 0;
    align-items: center;
    min-height: 32px;
    border-bottom: 1px solid var(--border);
  }
  .tab {
    border-radius: 6px 6px 0 0;
    border-bottom: none;
    padding: 4px 10px;
  }
  .tab.active {
    background: var(--bg);
    border-color: var(--accent);
    color: var(--accent);
  }
  .tab .x {
    margin-left: 6px;
    opacity: 0.6;
  }
  .tab .x:hover {
    opacity: 1;
    color: var(--bad);
  }
  .empty {
    color: var(--text-dim);
    font-size: 12px;
    padding: 4px;
  }
  .term-body {
    flex: 1;
    position: relative;
  }
  :global(.term-holder) {
    position: absolute;
    inset: 0;
    padding: 4px 8px;
  }
</style>
