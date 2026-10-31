<script lang="ts">
  // 状态栏：引擎统计。
  import { onMount } from 'svelte';
  import { getSim, statsOf } from '../lib/sim';
  import { app } from '../lib/store.svelte';

  let display = $state({ virtualNs: 0, events: 0, inFlight: 0, dropped: 0 });

  function pull() {
    try {
      display = statsOf(getSim());
      app.stats = display;
    } catch {
      /* 引擎未就绪 */
    }
  }

  onMount(() => {
    pull();
    const t = setInterval(pull, 1000);
    return () => clearInterval(t);
  });

  const secs = $derived((display.virtualNs / 1e9).toFixed(2));
</script>

<div class="statusbar">
  <span>虚拟时间 <b class="mono">{secs}s</b></span>
  <span>已处理事件 <b class="mono">{display.events}</b></span>
  <span>在途报文 <b class="mono">{display.inFlight}</b></span>
  <span>丢弃 <b class="mono" class:warn={display.dropped > 0}>{display.dropped}</b></span>
  <span class="spacer"></span>
  <span>设备 <b class="mono">{app.devices.length}</b></span>
  <span>链路 <b class="mono">{app.links.length}</b></span>
  <span class="ver">M0 · 逐包仿真内核 + 三方言 CLI</span>
</div>

<style>
  .statusbar {
    display: flex;
    gap: 16px;
    align-items: center;
    padding: 3px 12px;
    font-size: 12px;
    color: var(--text-dim);
    border-top: 1px solid var(--border);
    background: var(--bg-panel-2);
  }
  b {
    color: var(--text);
    font-weight: 600;
  }
  b.warn {
    color: var(--bad);
  }
  .spacer {
    flex: 1;
  }
  .ver {
    opacity: 0.7;
  }
</style>
