<script lang="ts">
  // 课程视图：左侧课程列表 + 右侧阅读器 + 三厂商命令对照表（运行时验证）。
  import { LESSONS, lessonTitle, lessonMarkdown } from '../lib/course';
  import { t, i18n } from '../lib/i18n.svelte';
  import { CMDMAP, type CmdRow } from '../content/cmdmap';
  import { renderMarkdown } from '../lib/md';
  import { getSim } from '../lib/sim';
  import { app } from '../lib/store.svelte';
  import { LABS, labTitle } from '../lib/labs';
  let currentId = $state(LESSONS[0]?.id ?? '');
  const current = $derived(LESSONS.find((l) => l.id === currentId) ?? LESSONS[0]);

  const LEVELS = [
    { id: 'IA', key: 'course.level.ia' },
    { id: 'IP', key: 'course.level.ip' },
    { id: 'IE', key: 'course.level.ie' },
    { id: 'MAP', key: 'course.level.map' },
  ] as const;
  const groups = $derived(
    LEVELS.map((lv) => ({
      ...lv,
      lessons: LESSONS.filter((l) => l.level === lv.id),
    })).filter((g) => g.lessons.length > 0)
  );
  const html = $derived(
    current ? renderMarkdown(lessonMarkdown(current, i18n.lang === 'en')) : ''
  );

  // 对照表按当前课程主题过滤
  const rows = $derived(
    current ? CMDMAP.filter((r) => r.topic === current.topic) : []
  );

  // 运行时验证：命令能否被对应方言解析（缓存结果）
  const verifyCache = new Map<string, boolean>();
  function verify(row: CmdRow, vendor: 'ios' | 'vrp' | 'comware'): boolean {
    const cmd = row[vendor];
    const mode = vendor === 'ios' ? (row.modeIos ?? row.mode) : vendor === 'vrp' ? (row.modeVrp ?? row.mode) : (row.modeComware ?? row.mode);
    const key = `${mode}|${cmd}`;
    if (!verifyCache.has(key)) {
      try {
        verifyCache.set(key, getSim().verify_command(dialectOf(vendor), mode, cmd));
      } catch {
        verifyCache.set(key, false);
      }
    }
    return verifyCache.get(key)!;
  }

  function dialectOf(v: 'ios' | 'vrp' | 'comware'): string {
    return v === 'ios' ? 'Ios' : v === 'vrp' ? 'Vrp' : 'Comware';
  }

  const VENDOR_NAMES = { ios: '思科 IOS', vrp: '华为 VRP', comware: '华三 Comware' } as const;

  function openLab() {
    if (!current?.lab) return;
    app.view = 'lab';
    // 实验加载交给 LeftPanel：这里用自定义事件简化
    window.dispatchEvent(new CustomEvent('netemu:open-lab', { detail: current.lab }));
  }
</script>

<div class="course-view">
  <aside class="list panel">
    <h3>{t('course.catalog')}</h3>
    <div class="pad lessons">
      {#each groups as g (g.id)}
        <div class="level-head">
          <span class="level-badge">{g.id}</span>
          {t(g.key)}
          <span class="level-count">{g.lessons.length}</span>
        </div>
        {#each g.lessons as l (l.id)}
          <button class="lesson" class:active={l.id === currentId} onclick={() => (currentId = l.id)}>
            <span class="no">{String(l.order).padStart(2, '0')}</span>
            {lessonTitle(l, i18n.lang === 'en')}
          </button>
        {/each}
      {/each}
    </div>
  </aside>

  <div class="reader">
    {#if current}
      <div class="panel lesson-card">
        <div class="pad">
          <h1 class="lesson-title">{lessonTitle(current, i18n.lang === 'en')}</h1>
          <div class="md">{@html html}</div>
          {#if current.lab}
            {@const lab = LABS.find((x) => x.id === current.lab)}
            <button class="primary lab-open" onclick={openLab}>
              {t('course.openLab')}{lab ? `: ${labTitle(lab)}` : ''}
            </button>
          {/if}
        </div>
      </div>

      {#if rows.length}
        <div class="panel">
          <h3>{t('course.table')}</h3>
          <div class="pad table-wrap">
            <table class="cmd-table mono">
              <thead>
                <tr>
                  <th>{t('course.feature')}</th>
                  <th>{t('course.ios')}</th>
                  <th>{t('course.vrp')}</th>
                  <th>{t('course.comware')}</th>
                </tr>
              </thead>
              <tbody>
                {#each rows as row (row.feature)}
                  <tr>
                    <td class="feat">{row.feature}</td>
                    <td class="ok">{verify(row, 'ios') ? '✓ ' : '⚠ '}{row.ios}</td>
                    <td class="ok">{verify(row, 'vrp') ? '✓ ' : '⚠ '}{row.vrp}</td>
                    <td class="ok">{verify(row, 'comware') ? '✓ ' : '⚠ '}{row.comware}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            <div class="hint">{t('course.tableHint')}</div>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .course-view {
    height: 100%;
    display: flex;
    min-height: 0;
  }
  .list {
    width: 240px;
    min-width: 200px;
    margin: 12px 0 12px 12px;
    align-self: flex-start;
    max-height: calc(100% - 24px);
    overflow-y: auto;
  }
  .lessons {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
  }
  .level-head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 2px 2px;
    font-size: 12px;
    color: var(--text-dim);
  }
  .level-head:first-child {
    margin-top: 0;
  }
  .level-badge {
    font-family: Consolas, monospace;
    font-size: 11px;
    padding: 1px 5px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    color: var(--accent);
  }
  .level-count {
    margin-left: auto;
    font-family: Consolas, monospace;
    opacity: 0.7;
  }
  .lesson {
    text-align: left;
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .lesson.active {
    border-color: var(--accent);
    color: var(--accent);
  }
  .lesson .no {
    font-family: Consolas, monospace;
    color: var(--text-dim);
    font-size: 12px;
  }
  .reader {
    flex: 1;
    min-width: 0;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
    max-height: 100%;
  }
  .lesson-card .pad {
    /* 阅读列限宽：中文约 40 字/行，长文不拉满屏 */
    max-width: 42em;
    padding: 20px 26px 24px;
  }
  .lesson-title {
    margin: 0 0 14px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
    font-size: 22px;
    line-height: 1.4;
    letter-spacing: 0.01em;
  }
  .lab-open {
    margin-top: 18px;
    width: 100%;
    white-space: normal;
    text-align: left;
  }
  .table-wrap {
    padding: 0 16px 16px;
    overflow-x: auto;
  }
  .cmd-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  .cmd-table th,
  .cmd-table td {
    border: 1px solid var(--border);
    padding: 7px 9px;
    text-align: left;
    white-space: nowrap;
  }
  .cmd-table th {
    color: var(--text-dim);
    font-weight: 600;
  }
  .cmd-table .feat {
    color: var(--text);
    font-family: inherit;
  }
  .cmd-table td.ok {
    color: var(--text);
  }
  .hint {
    color: var(--text-dim);
    font-size: 12px;
    margin-top: 8px;
  }
</style>
