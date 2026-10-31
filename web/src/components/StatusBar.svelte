<script lang="ts">
  // 状态栏：引擎统计。
  import { onMount } from 'svelte';
  import { getSim, statsOf } from '../lib/sim';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';

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
  <span>{t('status.time')} <b class="mono">{secs}s</b></span>
  <span>{t('status.events')} <b class="mono">{display.events}</b></span>
  <span>{t('status.flight')} <b class="mono">{display.inFlight}</b></span>
  <span>{t('status.dropped')} <b class="mono" class:warn={display.dropped > 0}>{display.dropped}</b></span>
  <span class="spacer"></span>
  <span>{t('status.devices')} <b class="mono">{app.devices.length}</b></span>
  <span>{t('status.links')} <b class="mono">{app.links.length}</b></span>
  <span class="ver">{t('status.version')}</span>
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
