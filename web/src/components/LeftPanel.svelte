<script lang="ts">
  // 左侧面板：设备库、属性编辑、链路抓包、实验卡、示例任务卡。
  import { getSim, dumpSummary } from '../lib/sim';
  import { app, deviceById, refreshFrom, openTerminal, setLabTitle, setLabChecks, markLabPassed, resetPositions, setPosition, setView } from '../lib/store.svelte';
  import { markDirty, currentWorkspace, exportFile, importFile } from '../lib/persistence';
  import { buildShareUrl } from '../lib/share';
  import { LABS, applyLab, runLabChecks, labTitle, labBrief, labTasks, type LabDef } from '../lib/labs';
  import { t } from '../lib/i18n.svelte';

  (window as unknown as { __appComp?: unknown }).__appComp = app;
  let newKind = $state<
    'Router' | 'Switch' | 'Host' | 'Firewall' | 'AccessPoint' | 'WlanController'
  >('Router');
  let newDialect = $state<'Ios' | 'Vrp' | 'Comware'>('Ios');

  // 属性编辑的本地副本
  let editName = $state('');
  let editDialect = $state<'Ios' | 'Vrp' | 'Comware'>('Ios');
  let editIp = $state('');
  let editPrefix = $state(24);
  let editGw = $state('');
  let editIp6 = $state('');
  let editPrefix6 = $state(64);
  let editGw6 = $state('');
  let lastEditedId = $state<number | null>(null);

  const selectedDev = $derived(
    app.selected?.kind === 'dev' ? deviceById(app.selected.id) : undefined
  );
  const selectedLink = $derived(
    app.selected?.kind === 'link' ? app.links.find((l) => l.id === app.selected!.id) : undefined
  );

  // 选中设备变化时同步编辑副本
  $effect(() => {
    const d = selectedDev;
    if (!d) return;
    if (lastEditedId !== d.id) {
      editName = d.name;
      editDialect = d.dialect;
      editIp = d.ports[0]?.ip ?? '';
      editPrefix = d.ports[0]?.prefix ?? 24;
      editGw = d.gw ?? '';
      editIp6 = d.ports[0]?.ipv6 ?? '';
      editPrefix6 = d.ports[0]?.prefix6 ?? 64;
      editGw6 = d.gw6 ?? '';
      lastEditedId = d.id;
    }
  });

  function refreshFromSim() {
    const s = dumpSummary(getSim());
    refreshFrom(s.devices, s.links);
    markDirty();
  }

  function addDevice() {
    const sim = getSim();
    const id = sim.add_device(newKind, newDialect);
    app.selected = { kind: 'dev', id };
    refreshFromSim();
  }

  function applyRename() {
    const d = selectedDev;
    if (!d) return;
    const name = editName.trim();
    if (name && name !== d.name) {
      getSim().rename_device(d.id, name);
      refreshFromSim();
    }
  }

  function applyDialect() {
    const d = selectedDev;
    if (!d || d.kind === 'Host') return;
    if (editDialect !== d.dialect) {
      getSim().set_dialect(d.id, editDialect);
      refreshFromSim();
    }
  }

  function applyHost() {
    const d = selectedDev;
    if (!d || d.kind !== 'Host') return;
    try {
      getSim().set_host_config(d.id, editIp.trim(), editPrefix, editGw.trim());
      refreshFromSim();
    } catch (err) {
      alert(String(err));
    }
  }

  function applyHostV6() {
    const d = selectedDev;
    if (!d || d.kind !== 'Host') return;
    try {
      getSim().set_host_v6(d.id, editIp6.trim(), editPrefix6, editGw6.trim());
      refreshFromSim();
    } catch (err) {
      alert(String(err));
    }
  }

  // DHCP 开关
  let dhcpOn = $state(false);
  $effect(() => {
    const d = selectedDev;
    if (d && lastEditedId === d.id) {
      dhcpOn = d.dhcp_enable;
    }
  });

  function toggleDhcp() {
    const d = selectedDev;
    if (!d || d.kind !== 'Host') return;
    try {
      const r = JSON.parse(getSim().set_host_dhcp(d.id, !dhcpOn)) as { ip: string | null };
      dhcpOn = !dhcpOn;
      editIp = r.ip ?? '';
      refreshFromSim();
    } catch (err) {
      alert(String(err));
    }
  }

  function removeSelected() {
    const sim = getSim();
    if (app.selected?.kind === 'dev') {
      sim.remove_device(app.selected.id);
    } else if (app.selected?.kind === 'link') {
      stopCapture();
      sim.remove_link(app.selected.id);
    }
    app.selected = null;
    refreshFromSim();
  }

  // ---------- 抓包 ----------
  interface CaptureRow {
    seq: number;
    ms: number;
    dir: number;
    srcMac: string;
    dstMac: string;
    kind: string;
    vlan: number | null;
    info: string;
  }
  let captureOn = $state(false);
  let captureRows = $state<CaptureRow[]>([]);
  let captureSince = 0;
  let captureTimer: ReturnType<typeof setInterval> | null = null;

  function startCapture() {
    const l = selectedLink;
    if (!l) return;
    getSim().set_capture(l.id, true);
    captureSince = 0;
    captureRows = [];
    captureOn = true;
    if (captureTimer) clearInterval(captureTimer);
    captureTimer = setInterval(pullCapture, 600);
  }

  function stopCapture() {
    const l = selectedLink;
    if (l) getSim().set_capture(l.id, false);
    captureOn = false;
    if (captureTimer) {
      clearInterval(captureTimer);
      captureTimer = null;
    }
  }

  function pullCapture() {
    const l = selectedLink;
    if (!l || !captureOn) return;
    const r = JSON.parse(getSim().capture_rows(l.id, captureSince)) as {
      next: number;
      rows: CaptureRow[];
    };
    captureSince = r.next;
    if (r.rows.length) {
      captureRows = [...captureRows, ...r.rows].slice(-200);
    }
  }

  // 选中链路变化时停止抓包
  $effect(() => {
    void selectedLink?.id;
    if (captureOn) {
      getSim().set_capture(selectedLink?.id ?? 0, false);
      captureOn = false;
      if (captureTimer) {
        clearInterval(captureTimer);
        captureTimer = null;
      }
    }
  });

  // ---------- 示例任务（M0） ----------
  let checkResult = $state<null | { ping: boolean; route: boolean }>(null);

  const demoReady = $derived(app.devices.some((d) => d.name === 'PC1' && d.kind === 'Host'));

  function demoTopology() {
    const sim = getSim();
    sim.reset();
    // PC1 — R1 — R2 — PC2
    const pc1 = sim.add_device('Host', 'Ios');
    const r1 = sim.add_device('Router', 'Ios');
    const r2 = sim.add_device('Router', 'Ios');
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
    // 画布位置
    app.positions[pc1] = { x: 60, y: 90 };
    app.positions[r1] = { x: 260, y: 90 };
    app.positions[r2] = { x: 460, y: 90 };
    app.positions[pc2] = { x: 660, y: 90 };
    app.pan = { x: 20, y: 30 };
    app.zoom = 1;
    app.selected = null;
    checkResult = null;
    refreshFromSim();
  }

  function runChecks() {
    const sim = getSim();
    const pc1 = app.devices.find((d) => d.name === 'PC1');
    const r1 = app.devices.find((d) => d.name === 'R1');
    if (!pc1 || !r1) return;
    const ping = JSON.parse(sim.check_ping(pc1.id, '192.168.2.10')) as { passed: boolean };
    const route = JSON.parse(sim.check_static_route(r1.id, '192.168.2.0/24')) as {
      passed: boolean;
    };
    checkResult = { ping: ping.passed, route: route.passed };
  }

  // ---------- M1 实验 ----------
  let activeLab = $state<LabDef | null>(null);

  function startLab(lab: LabDef) {
    applyLab(lab);
    activeLab = lab;
    setLabTitle(lab.title);
    setLabChecks(null);
  }

  // 云恢复后重读实验进度徽章
  $effect(() => {
    const handler = () => {
      try {
        const raw = localStorage.getItem('netemu.labs.v1');
        if (raw) app.labPassed = JSON.parse(raw) as Record<string, boolean>;
      } catch {
        /* 忽略 */
      }
    };
    window.addEventListener('netemu:backup-restored', handler);
    return () => window.removeEventListener('netemu:backup-restored', handler);
  });

  // 课程页"打开对应实验"事件
  $effect(() => {
    const handler = (e: Event) => {
      const lab = LABS.find((x) => x.id === (e as CustomEvent<string>).detail);
      if (lab) startLab(lab);
    };
    window.addEventListener('netemu:open-lab', handler);
    return () => window.removeEventListener('netemu:open-lab', handler);
  });

  function exitLab() {
    activeLab = null;
    setLabTitle(null);
    setLabChecks(null);
  }

  // ---------- 拓扑导入 / 分享 ----------
  let fileInput = $state<HTMLInputElement | null>(null);
  let topoMsg = $state('');

  async function copyShare() {
    try {
      const url = await buildShareUrl(currentWorkspace());
      try {
        await navigator.clipboard.writeText(url);
        topoMsg = `${t('left.shareCopied')} (${url.length})`;
      } catch {
        window.prompt(t('left.promptShare'), url);
        topoMsg = t('left.shareManual');
      }
    } catch (e) {
      topoMsg = `${t('left.shareFail')}: ${String(e)}`;
    }
  }

  async function doImport(ev: Event) {
    const input = ev.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try {
      const ws = await importFile(file);
      getSim().load(JSON.stringify(ws.net));
      resetPositions();
      for (const [id, pos] of Object.entries(ws.editor.positions ?? {})) {
        setPosition(Number(id), pos);
      }
      setView(ws.editor.pan ?? { x: 20, y: 30 }, ws.editor.zoom ?? 1);
      const s = dumpSummary(getSim());
      refreshFrom(s.devices, s.links);
      markDirty();
      topoMsg = t('left.importOk');
    } catch (e) {
      topoMsg = `${t('left.importFail')}: ${String(e)}`;
    }
  }
</script>

<div class="left-panel">
  <section class="panel">
    <h3>{t('left.devices')}</h3>
    <div class="pad add-row">
      <select bind:value={newKind}>
        <option value="Router">{t('left.router')}</option>
        <option value="Switch">{t('left.switch')}</option>
        <option value="Host">{t('left.pc')}</option>
        <option value="Firewall">{t('left.firewall')}</option>
        <option value="AccessPoint">{t('left.ap')}</option>
        <option value="WlanController">{t('left.ac')}</option>
      </select>
      {#if newKind !== 'Host'}
        <select bind:value={newDialect}>
          <option value="Ios">{t('course.ios')}</option>
          <option value="Vrp">{t('course.vrp')}</option>
          <option value="Comware">{t('course.comware')}</option>
        </select>
      {/if}
      <button class="primary" onclick={addDevice}>{t('left.add')}</button>
    </div>
    <div class="pad hint">{t('left.hint')}</div>
  </section>

  <section class="panel">
    <h3>{t('left.props')}</h3>
    {#if selectedDev}
      <div class="pad props">
        <label>{t('left.name')}</label>
        <div class="row">
          <input bind:value={editName} onblur={applyRename} onkeydown={(e) => e.key === 'Enter' && applyRename()} />
        </div>
        {#if selectedDev.kind !== 'Host'}
          <label>{t('left.dialect')}</label>
          <div class="row">
            <select bind:value={editDialect} onchange={applyDialect}>
              <option value="Ios">{t('course.ios')}</option>
              <option value="Vrp">{t('course.vrp')}</option>
              <option value="Comware">{t('course.comware')}</option>
            </select>
          </div>
          <div class="ports mono">
            {#each selectedDev.ports as p}
              <div>
                {t('left.port')}{p.idx}: {p.admin_up ? t('left.on') : t('left.off')}
                {p.linked ? '·' + t('left.linked') : ''} {p.ip ?? ''}
              </div>
            {/each}
          </div>
        {:else}
          <button class="dhcp-btn" class:on={dhcpOn} onclick={toggleDhcp}>
            {dhcpOn ? t('left.dhcpOn') : t('left.dhcpOff')}
          </button>
          {#if !dhcpOn}
            <label>{t('left.ip')}</label>
            <div class="row">
              <input class="mono" bind:value={editIp} placeholder="192.168.1.10" />
              <input class="mono" type="number" min="0" max="32" bind:value={editPrefix} style="width:64px" />
            </div>
            <label>{t('left.gw')}</label>
            <div class="row">
              <input class="mono" bind:value={editGw} placeholder="192.168.1.1" />
              <button onclick={applyHost}>{t('left.apply')}</button>
            </div>
          {/if}
          <label>{t('left.ipv6')}</label>
          <div class="row">
            <input class="mono" bind:value={editIp6} placeholder="2001:db8::10" />
            <input class="mono" type="number" min="0" max="128" bind:value={editPrefix6} style="width:64px" />
          </div>
          <label>{t('left.gw6')}</label>
          <div class="row">
            <input class="mono" bind:value={editGw6} placeholder="2001:db8::1" />
            <button onclick={applyHostV6}>{t('left.apply')}</button>
          </div>
          <div class="hint">{t('left.hostHint')}</div>
        {/if}
        <button class="danger" onclick={removeSelected}>{t('left.deleteDev')}</button>
      </div>
    {:else if selectedLink}
      <div class="pad props">
        <div class="hint">{t('left.linkHint')}</div>
        {#if captureOn}
          <button onclick={stopCapture}>{t('left.stopCapture')}</button>
          <div class="cap-list mono">
            {#each captureRows as r (r.seq)}
              <div class="cap-row">
                <span class="cap-seq">#{r.seq}</span>
                <span class="cap-kind" class:arp={r.kind === 'ARP'}>{r.kind}</span>
                {#if r.vlan !== null}<span class="cap-vlan">VLAN{r.vlan}</span>{/if}
                <div class="cap-info">{r.info}</div>
              </div>
            {:else}
              <div class="hint">{t('left.waiting')}</div>
            {/each}
          </div>
        {:else}
          <button onclick={startCapture}>{t('left.startCapture')}</button>
        {/if}
        <button class="danger" onclick={removeSelected}>{t('left.deleteLink')}</button>
      </div>
    {:else}
      <div class="pad hint">{t('left.noSelection')}</div>
    {/if}
  </section>

  <section class="panel">
    <h3>{t('left.labs')}</h3>
    <div class="pad">
      {#if activeLab}
        <div class="lab-title">{labTitle(activeLab)}</div>
        <div class="hint">{labBrief(activeLab)}</div>
        <ol class="task">
          {#each labTasks(activeLab) as task}
            <li>{@html task}</li>
          {/each}
        </ol>
        <button
          class="primary"
          onclick={() => {
            const r = runLabChecks(activeLab!);
            setLabChecks(r);
            markLabPassed(activeLab!.id, r.every((x) => x.passed));
          }}
        >
          {t('left.checkBtn')}
        </button>
        {#if app.labChecks}
          <div class="check mono">
            {#each app.labChecks as r}
              <div class={r.passed ? 'good' : 'bad'}>
                {r.passed ? '✓' : '✗'} {r.label}
                <span class="detail">{r.detail}</span>
              </div>
            {/each}
          </div>
        {/if}
        <button onclick={() => startLab(activeLab!)}>{t('left.resetLab')}</button>
        <button class="danger" onclick={exitLab}>{t('left.exitLab')}</button>
      {:else}
        {#each LABS as lab (lab.id)}
          <button class="lab-btn" onclick={() => startLab(lab)}>
            {labTitle(lab)}
            {#if app.labPassed[lab.id]}<span class="pass-mark">✓</span>{/if}
          </button>
          <div class="hint">{labBrief(lab)}</div>
        {/each}
      {/if}
    </div>
  </section>

  <section class="panel">
    <h3>{t('left.demoTitle')}</h3>
    <div class="pad">
      <button onclick={demoTopology}>{t('left.loadDemo')}</button>
      {#if demoReady}
        <ol class="task">
          <li>R1：g0/0 配 <code class="mono">192.168.1.1/24</code>、g0/1 配 <code class="mono">10.0.0.1/24</code>，并 <code class="mono">no shutdown</code></li>
          <li>R2：g0/0 配 <code class="mono">10.0.0.2/24</code>、g0/1 配 <code class="mono">192.168.2.1/24</code>，并 <code class="mono">no shutdown</code></li>
          <li>R1/R2 各配静态路由到对方网段</li>
          <li>从 PC1 终端 <code class="mono">ping 192.168.2.10</code></li>
        </ol>
        <button class="primary" onclick={runChecks}>{t('left.checkBtn')}</button>
        {#if checkResult}
          <div class="check mono">
            <div class={checkResult.ping ? 'good' : 'bad'}>
              {checkResult.ping ? '✓' : '✗'} {t('left.demoPing')}
            </div>
            <div class={checkResult.route ? 'good' : 'bad'}>
              {checkResult.route ? '✓' : '✗'} {t('left.demoRoute')}
            </div>
          </div>
        {/if}
      {/if}
    </div>
  </section>

  <section class="panel">
    <h3>{t('left.topoTitle')}</h3>
    <div class="pad">
      <div class="row">
        <button onclick={exportFile}>{t('left.export')}</button>
        <button onclick={() => fileInput?.click()}>{t('left.import')}</button>
      </div>
      <div class="row">
        <button class="primary" onclick={copyShare}>{t('left.share')}</button>
      </div>
      <input
        class="hidden-file"
        type="file"
        accept="application/json,.json"
        bind:this={fileInput}
        onchange={doImport}
      />
      {#if topoMsg}<div class="hint">{topoMsg}</div>{/if}
    </div>
  </section>
</div>

<style>
  .left-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px;    overflow-y: auto;
    height: 100%;
  }
  .pad {
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .add-row {
    flex-direction: row;
    flex-wrap: wrap;
  }
  .add-row select {
    flex: 1;
    min-width: 100px;
  }
  label {
    font-size: 12px;
    color: var(--text-dim);
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .hidden-file {
    display: none;
  }
  .row input {
    flex: 1;
    min-width: 0;
  }
  .hint {
    color: var(--text-dim);
    font-size: 12px;
    line-height: 1.5;
  }
  .ports {
    font-size: 11px;
    color: var(--text-dim);
    line-height: 1.6;
  }
  .danger {
    margin-top: 4px;
  }
  .task {
    margin: 0;
    padding-left: 18px;
    font-size: 12px;
    line-height: 1.7;
    color: var(--text);
  }
  .task code {
    background: var(--bg-panel-2);
    padding: 0 4px;
    border-radius: 3px;
  }
  .check {
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .check .detail {
    color: var(--text-dim);
    margin-left: 4px;
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
  .cap-list {
    max-height: 180px;
    overflow-y: auto;
    font-size: 11px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .cap-row {
    border-bottom: 1px solid var(--border);
    padding-bottom: 3px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: baseline;
  }
  .cap-seq {
    color: var(--text-dim);
  }
  .cap-kind {
    color: var(--accent);
    font-weight: 600;
  }
  .cap-kind.arp {
    color: var(--accent-2);
  }
  .cap-vlan {
    color: var(--host);
  }
  .cap-info {
    width: 100%;
    color: var(--text-dim);
    word-break: break-all;
  }
  .lab-btn {
    text-align: left;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .pass-mark {
    color: var(--good);
    font-weight: 700;
  }
  .dhcp-btn {
    text-align: left;
  }
  .dhcp-btn.on {
    border-color: var(--good);
    color: var(--good);
  }
  .lab-title {
    font-weight: 600;
    font-size: 13px;
  }
</style>
