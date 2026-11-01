<script lang="ts">
  // 拓扑编辑器画布：拖拽节点、连线模式、平移缩放、选中。
  import { getSim, dumpSummary } from '../lib/sim';
  import { app, deviceById, deviceAlive, refreshFrom, openTerminal } from '../lib/store.svelte';
  import { markDirty } from '../lib/persistence';
  import { t } from '../lib/i18n.svelte';

  let svgEl: SVGSVGElement | undefined = $state();

  const NODE_W = 92;
  const NODE_H = 64;

  function toCanvas(e: MouseEvent): { x: number; y: number } {
    const rect = svgEl!.getBoundingClientRect();
    return {
      x: (e.clientX - rect.left - app.pan.x) / app.zoom,
      y: (e.clientY - rect.top - app.pan.y) / app.zoom,
    };
  }

  function refreshFromSim() {
    const s = dumpSummary(getSim());
    refreshFrom(s.devices, s.links);
    markDirty();
  }

  // ---------- 节点拖拽 ----------
  let dragging: { id: number; dx: number; dy: number } | null = null;

  function nodePointerDown(e: PointerEvent, id: number) {
    e.stopPropagation();
    if (app.linkMode) {
      if (app.linkFrom === null) {
        app.linkFrom = id;
      } else {
        finishLink(id);
      }
      return;
    }
    const p = toCanvas(e);
    const pos = app.positions[id];
    dragging = { id, dx: p.x - pos.x, dy: p.y - pos.y };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    app.selected = { kind: 'dev', id };
  }

  function nodePointerMove(e: PointerEvent) {
    if (!dragging) return;
    const p = toCanvas(e);
    app.positions[dragging.id] = {
      x: Math.round(p.x - dragging.dx),
      y: Math.round(p.y - dragging.dy),
    };
  }

  function nodePointerUp() {
    if (dragging) {
      dragging = null;
      markDirty();
    }
  }

  // ---------- 画布平移 / 缩放 ----------
  let panning: { x: number; y: number; px: number; py: number } | null = null;

  function bgPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    if (app.linkMode) {
      app.linkMode = false;
      app.linkFrom = null;
      return;
    }
    panning = { x: e.clientX, y: e.clientY, px: app.pan.x, py: app.pan.y };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    app.selected = null;
  }

  function bgPointerMove(e: PointerEvent) {
    if (app.linkMode && app.linkFrom !== null) {
      app.cursor = toCanvas(e);
      return;
    }
    if (!panning) return;
    app.pan = {
      x: panning.px + (e.clientX - panning.x),
      y: panning.py + (e.clientY - panning.y),
    };
  }

  function bgPointerUp() {
    panning = null;
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const rect = svgEl!.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;
    const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
    const nz = Math.min(2.5, Math.max(0.35, app.zoom * factor));
    app.pan = {
      x: cx - ((cx - app.pan.x) * nz) / app.zoom,
      y: cy - ((cy - app.pan.y) * nz) / app.zoom,
    };
    app.zoom = nz;
    markDirty();
  }

  // ---------- 连线 ----------
  function finishLink(toId: number) {
    const from = app.linkFrom;
    app.linkMode = false;
    app.linkFrom = null;
    if (from === null || from === toId) return;
    try {
      getSim().add_link(from, toId);
      refreshFromSim();
    } catch (err) {
      alert(String(err));
    }
  }

  // ---------- 键盘 ----------
  function onKeydown(e: KeyboardEvent) {
    if ((e.target as HTMLElement)?.tagName === 'INPUT') return;
    if (e.key === 'Escape') {
      app.linkMode = false;
      app.linkFrom = null;
      app.selected = null;
      return;
    }
    if (!app.selected) return;
    if (e.key !== 'Delete' && e.key !== 'Backspace') return;
    e.preventDefault();
    const sim = getSim();
    if (app.selected.kind === 'dev') {
      sim.remove_device(app.selected.id);
    } else {
      sim.remove_link(app.selected.id);
    }
    app.selected = null;
    refreshFromSim();
  }

  function edgeUp(linkId: number): boolean {
    const link = app.links.find((l) => l.id === linkId);
    if (!link) return false;
    const endUp = (dev: number, port: number) => {
      const p = deviceById(dev)?.ports[port];
      return !!p && p.admin_up && p.linked;
    };
    return endUp(link.a[0], link.a[1]) && endUp(link.b[0], link.b[1]);
  }

  const KIND_COLOR: Record<string, string> = {
    Router: 'var(--router)',
    Switch: 'var(--switch)',
    Host: 'var(--host)',
    Firewall: 'var(--firewall)',
  };

  const KIND_TEXT: Record<string, string> = {
    Router: 'left.router',
    Switch: 'left.switch',
    Host: 'left.pc',
    Firewall: 'left.firewall',
  };
</script>

<svelte:window onkeydown={onKeydown} />

<div class="canvas-wrap">
  <svg
    bind:this={svgEl}
    onwheel={onWheel}
    onpointerdown={bgPointerDown}
    onpointermove={bgPointerMove}
    onpointerup={bgPointerUp}
    onpointerleave={bgPointerUp}
  >
    <defs>
      <pattern id="grid" width="32" height="32" patternUnits="userSpaceOnUse">
        <path d="M 32 0 L 0 0 0 32" fill="none" stroke="var(--grid-line)" stroke-width="1" />
      </pattern>
    </defs>
    <rect width="100%" height="100%" fill="url(#grid)" />
    <g transform={`translate(${app.pan.x} ${app.pan.y}) scale(${app.zoom})`}>
      <!-- 链路 -->
      {#each app.links as link (link.id)}
        {@const pa = app.positions[link.a[0]]}
        {@const pb = app.positions[link.b[0]]}
        {#if pa && pb}
          <line
            x1={pa.x + NODE_W / 2}
            y1={pa.y + NODE_H / 2}
            x2={pb.x + NODE_W / 2}
            y2={pb.y + NODE_H / 2}
            stroke={edgeUp(link.id) ? 'var(--good)' : 'var(--link-down)'}
            stroke-width={app.selected?.kind === 'link' && app.selected.id === link.id ? 4 : 2.5}
            stroke-dasharray={edgeUp(link.id) ? '' : '6 4'}
          />
          <line
            x1={pa.x + NODE_W / 2}
            y1={pa.y + NODE_H / 2}
            x2={pb.x + NODE_W / 2}
            y2={pb.y + NODE_H / 2}
            stroke="transparent"
            stroke-width="14"
            onpointerdown={(e) => {
              e.stopPropagation();
              app.selected = { kind: 'link', id: link.id };
            }}
            style="cursor:pointer"
          />
        {/if}
      {/each}

      <!-- 连线中的虚线 -->
      {#if app.linkMode && app.linkFrom !== null && app.cursor}
        {@const pa = app.positions[app.linkFrom]}
        {#if pa}
          <line
            x1={pa.x + NODE_W / 2}
            y1={pa.y + NODE_H / 2}
            x2={app.cursor.x}
            y2={app.cursor.y}
            stroke="var(--accent)"
            stroke-width="2"
            stroke-dasharray="5 4"
          />
        {/if}
      {/if}

      <!-- 设备 -->
      {#each app.devices as dev (dev.id)}
        {@const pos = app.positions[dev.id]}
        {@const alive = deviceAlive(dev)}
        <g
          transform={`translate(${pos?.x ?? 0} ${pos?.y ?? 0})`}
          onpointerdown={(e) => nodePointerDown(e, dev.id)}
          onpointermove={nodePointerMove}
          onpointerup={nodePointerUp}
          ondblclick={() => openTerminal(dev.id)}
          style="cursor: grab"
        >
          <rect
            width={NODE_W}
            height={NODE_H}
            rx="10"
            fill="var(--bg-panel)"
            stroke={app.selected?.kind === 'dev' && app.selected.id === dev.id
              ? 'var(--accent)'
              : 'var(--node-stroke)'}
            stroke-width={app.selected?.kind === 'dev' && app.selected.id === dev.id ? 2.5 : 1.5}
          />
          {#if dev.kind === 'Router'}
            <circle cx={NODE_W / 2} cy={25} r="12" fill="none" stroke={KIND_COLOR[dev.kind]} stroke-width="2" />
            <path
              d="M {NODE_W / 2 - 8} 25 h16 M {NODE_W / 2 - 4} 20.5 l4 4.5 l-4 4.5 M {NODE_W / 2 + 4} 29.5 l-4 -4.5 l4 -4.5"
              fill="none"
              stroke={KIND_COLOR[dev.kind]}
              stroke-width="1.6"
            />
          {:else if dev.kind === 'Switch'}
            <rect x={NODE_W / 2 - 15} y={14} width="30" height="17" rx="3" fill="none" stroke={KIND_COLOR[dev.kind]} stroke-width="2" />
            {#each [0, 1, 2, 3] as i}
              <circle cx={NODE_W / 2 - 10 + i * 7} cy={31} r="1.8" fill={KIND_COLOR[dev.kind]} />
            {/each}
          {:else if dev.kind === 'Firewall'}
            <path
              d="M {NODE_W / 2} 12 l11 4 v8 c0 8 -5 12 -11 15 c-6 -3 -11 -7 -11 -15 v-8 z"
              fill="none"
              stroke={KIND_COLOR[dev.kind]}
              stroke-width="2"
            />
            <path d="M {NODE_W / 2 - 5} 25 h10 M {NODE_W / 2} 20 v10" stroke={KIND_COLOR[dev.kind]} stroke-width="1.6" />
          {:else}
            <rect x={NODE_W / 2 - 11} y={12} width="22" height="14" rx="2" fill="none" stroke={KIND_COLOR[dev.kind]} stroke-width="2" />
            <path d="M {NODE_W / 2 - 5} 32 h10 M {NODE_W / 2} 26 v6" stroke={KIND_COLOR[dev.kind]} stroke-width="2" fill="none" />
          {/if}
          <text x={NODE_W / 2} y={52} text-anchor="middle" fill="var(--text)" font-size="12">
            {dev.name}
            <tspan fill="var(--text-dim)" font-size="10"> {t(KIND_TEXT[dev.kind])}</tspan>
          </text>
          <circle cx={NODE_W - 9} cy={9} r="4" fill={alive ? 'var(--good)' : 'var(--dot-off)'} />
        </g>
      {/each}
    </g>
  </svg>

  <div class="floating">
    {#if app.linkMode}
      <button class="primary" onclick={() => { app.linkMode = false; app.linkFrom = null; }}>
        {app.linkFrom === null ? t('topo.connectStart') : t('topo.connectEnd')}（Esc）
      </button>
    {:else}
      <button onclick={() => { app.linkMode = true; }} disabled={app.devices.length < 1}>
        {t('topo.connectBtn')}
      </button>
    {/if}
  </div>
</div>

<style>
  .canvas-wrap {
    position: relative;
    height: 100%;
  }
  svg {
    width: 100%;
    height: 100%;
    display: block;
    background: var(--bg);
    user-select: none;
    touch-action: none;
  }
  .floating {
    position: absolute;
    top: 12px;
    right: 12px;
    display: flex;
    gap: 8px;
  }
</style>
