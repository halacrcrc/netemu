<script lang="ts">
  // 笔记视图：列表 + 编辑/预览 + Gist 云同步。
  import { renderMarkdown } from '../lib/md';
  import {
    loadNotes,
    saveNotes,
    getToken,
    saveToken,
    getGistId,
    backupToGist,
    restoreFromGist,
    applyBackup,
    type Note,
  } from '../lib/notes';

  let notes = $state<Note[]>(loadNotes());
  let activeId = $state<string | null>(notes[0]?.id ?? null);
  let editing = $state(false);
  let draftTitle = $state('');
  let draftBody = $state('');

  const active = $derived(notes.find((n) => n.id === activeId) ?? null);

  let token = $state(getToken());
  let syncMsg = $state('');
  let syncBusy = $state(false);
  let gistOk = $state(!!getGistId());

  function persist() {
    saveNotes(notes);
  }

  function newNote() {
    const n: Note = {
      id: `n${Date.now()}`,
      title: '无标题笔记',
      body: '',
      at: Date.now(),
    };
    notes = [n, ...notes];
    activeId = n.id;
    editing = true;
    draftTitle = n.title;
    draftBody = n.body;
    persist();
  }

  function openNote(n: Note) {
    activeId = n.id;
    editing = false;
  }

  function startEdit() {
    if (!active) return;
    draftTitle = active.title;
    draftBody = active.body;
    editing = true;
  }

  function saveEdit() {
    if (!activeId) return;
    notes = notes.map((n) =>
      n.id === activeId
        ? { ...n, title: draftTitle.trim() || '无标题笔记', body: draftBody, at: Date.now() }
        : n
    );
    editing = false;
    persist();
  }

  function removeNote() {
    if (!activeId) return;
    if (!confirm('删除这篇笔记？')) return;
    notes = notes.filter((n) => n.id !== activeId);
    activeId = notes[0]?.id ?? null;
    editing = false;
    persist();
  }

  const previewHtml = $derived(renderMarkdown(draftBody));
  const readHtml = $derived(active ? renderMarkdown(active.body) : '');

  // ---------- Gist 同步 ----------
  async function doBackup() {
    syncBusy = true;
    syncMsg = '';
    try {
      saveToken(token.trim());
      const id = await backupToGist(token.trim());
      gistOk = true;
      syncMsg = `✓ 已备份到 Gist（${id.slice(0, 8)}…）`;
    } catch (e) {
      syncMsg = `✗ ${String(e instanceof Error ? e.message : e)}`;
    } finally {
      syncBusy = false;
    }
  }

  async function doRestore() {
    syncBusy = true;
    syncMsg = '';
    try {
      saveToken(token.trim());
      const data = await restoreFromGist(token.trim());
      applyBackup(data);
      applyLocal(data);
      syncMsg = '✓ 已从云端恢复（笔记已覆盖，题库/实验进度按次数合并）';
    } catch (e) {
      syncMsg = `✗ ${String(e instanceof Error ? e.message : e)}`;
    } finally {
      syncBusy = false;
    }
  }

  function applyLocal(data: Record<string, unknown>) {
    if (Array.isArray(data.notes)) {
      notes = data.notes as Note[];
      activeId = notes[0]?.id ?? null;
      editing = false;
      persist();
    }
    // 题库/实验进度写回后提示刷新生效
    window.dispatchEvent(new CustomEvent('netemu:backup-restored'));
  }
</script>

<div class="notes-view">
  <aside class="list panel">
    <h3>我的笔记</h3>
    <div class="pad">
      <button class="primary" onclick={newNote}>＋ 新建笔记</button>
    </div>
    <div class="pad note-list">
      {#each notes as n (n.id)}
        <button class="note-item" class:active={n.id === activeId} onclick={() => openNote(n)}>
          <span class="nt">{n.title}</span>
          <span class="nd">{new Date(n.at).toLocaleDateString()}</span>
        </button>
      {:else}
        <div class="hint">还没有笔记，点上方新建。</div>
      {/each}
    </div>
  </aside>

  <div class="editor">
    {#if active}
      <div class="panel">
        {#if editing}
          <div class="pad">
            <input class="title-input" bind:value={draftTitle} placeholder="笔记标题" />
            <textarea
              class="body-input mono"
              bind:value={draftBody}
              rows={16}
              placeholder="支持 Markdown：## 标题、**加粗**、`代码`、- 列表、``` 代码块"
            ></textarea>
            <div class="row">
              <button class="primary" onclick={saveEdit}>保存</button>
              <button onclick={() => (editing = false)}>取消</button>
              <span class="hint">预览随输入实时更新 →</span>
            </div>
          </div>
          {#if draftBody}
            <div class="pad preview">
              <div class="md">{@html previewHtml}</div>
            </div>
          {/if}
        {:else}
          <div class="pad">
            <div class="read-head">
              <h1 class="note-title">{active.title}</h1>
              <div class="row">
                <button onclick={startEdit}>✎ 编辑</button>
                <button class="danger" onclick={removeNote}>删除</button>
              </div>
            </div>
            {#if active.body}
              <div class="md">{@html readHtml}</div>
            {:else}
              <div class="hint">这篇笔记还没有内容，点"编辑"开始写。</div>
            {/if}
          </div>
        {/if}
      </div>
    {:else}
      <div class="panel"><div class="pad hint">选择或新建一篇笔记。</div></div>
    {/if}

    <div class="panel">
      <h3>云同步（GitHub Gist）</h3>
      <div class="pad">
        <div class="hint">
          在 GitHub 生成<b>经典 PAT</b>（勾选 gist 权限），粘贴到下面。笔记、题库记录、实验进度会备份到一个私有
          Gist；Token 只保存在本机浏览器中。
        </div>
        <input
          class="mono token-input"
          type="password"
          bind:value={token}
          placeholder="ghp_…（经典 PAT，gist 权限）"
        />
        <div class="row">
          <button class="primary" disabled={syncBusy} onclick={doBackup}>
            {gistOk ? '↻ 备份到云' : '☁ 备份到云'}
          </button>
          <button disabled={syncBusy || !gistOk} onclick={doRestore}>⤓ 从云恢复</button>
        </div>
        {#if syncMsg}<div class="sync-msg">{syncMsg}</div>{/if}
      </div>
    </div>
  </div>
</div>

<style>
  .notes-view {
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
  .note-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .note-item {
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .note-item.active {
    border-color: var(--accent);
  }
  .nt {
    font-size: 13px;
  }
  .nd {
    font-size: 11px;
    color: var(--text-dim);
  }
  .editor {
    flex: 1;
    min-width: 0;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
    max-height: 100%;
  }
  .note-title {
    margin: 0;
    font-size: 20px;
  }
  .read-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }
  .title-input {
    width: 100%;
    font-size: 16px;
    font-weight: 600;
    margin-bottom: 8px;
  }
  .body-input {
    width: 100%;
    resize: vertical;
    line-height: 1.6;
    background: var(--bg-panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .preview {
    border-top: 1px solid var(--border);
  }
  .token-input {
    width: 100%;
  }
  .sync-msg {
    font-size: 13px;
    color: var(--text-dim);
  }
</style>
