<script lang="ts">
  // 应用外壳：顶栏 + 视图切换（练习场 / 题库）。
  import { theme, toggleTheme } from './lib/theme.svelte';
  import { app, setViewName } from './lib/store.svelte';
  import { t, toggleLang, i18n } from './lib/i18n.svelte';
  import TopoCanvas from './components/TopoCanvas.svelte';
  import LeftPanel from './components/LeftPanel.svelte';
  import TerminalPanel from './components/TerminalPanel.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import QuizView from './components/QuizView.svelte';
  import CourseView from './components/CourseView.svelte';
  import NotesView from './components/NotesView.svelte';

  function nav(name: 'lab' | 'quiz' | 'course' | 'notes') {
    setViewName(name);
  }
</script>

<div class="shell">
  <header>
    <div class="brand">{t('app.brand')}</div>
    <nav>
      <span class="nav-item" class:active={app.viewName === 'lab'} onclick={() => nav('lab')}>
        {t('nav.lab')}
      </span>
      <span class="nav-item" class:active={app.viewName === 'quiz'} onclick={() => nav('quiz')}>
        {t('nav.quiz')}
      </span>
      <span
        class="nav-item"
        class:active={app.viewName === 'course'}
        onclick={() => nav('course')}
      >
        {t('nav.course')}
      </span>
      <span
        class="nav-item"
        class:active={app.viewName === 'notes'}
        onclick={() => nav('notes')}
      >
        {t('nav.notes')}
      </span>
    </nav>
    <button
      class="theme-toggle"
      onclick={toggleLang}
      title={t('lang.toggle')}
      aria-label={t('lang.toggle')}
    >
      {i18n.lang === 'zh' ? 'EN' : '中'}
    </button>
    <button
      class="theme-toggle"
      onclick={toggleTheme}
      title={theme.name === 'dark' ? t('theme.title.light') : t('theme.title.dark')}
      aria-label={theme.name === 'dark' ? t('theme.title.light') : t('theme.title.dark')}
    >
      {theme.name === 'dark' ? '☀︎' : '☾'}
    </button>
  </header>
  {#if app.viewName === 'lab'}
    <div class="main">
      <aside><LeftPanel /></aside>
      <div class="center">
        <TopoCanvas />
        <StatusBar />
      </div>
    </div>
    <TerminalPanel />
  {:else if app.viewName === 'quiz'}
    <div class="page"><QuizView /></div>
  {:else if app.viewName === 'course'}
    <div class="page"><CourseView /></div>
  {:else if app.viewName === 'notes'}
    <div class="page"><NotesView /></div>
  {/if}
</div>

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  header {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 0 16px;
    height: 46px;
    border-bottom: var(--header-border);
    background: var(--bg-panel);
  }
  .theme-toggle {
    margin-left: auto;
    width: 32px;
    padding: 4px 0;
    font-size: 15px;
    line-height: 1;
  }
  .brand {
    font-weight: 700;
    font-size: 16px;
    color: var(--accent);
    letter-spacing: 0.02em;
  }
  nav {
    display: flex;
    gap: 18px;
  }
  .nav-item {
    color: var(--text-dim);
    cursor: pointer;
    font-size: 14px;
  }
  .nav-item.active {
    color: var(--text);
    font-weight: 600;
    border-bottom: 2px solid var(--accent);
    padding-bottom: 2px;
  }
  .nav-item.soon {
    opacity: 0.55;
    cursor: default;
  }
  .page {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  aside {
    width: 300px;
    min-width: 260px;
    border-right: 1px solid var(--border);
    background: var(--bg-panel-2);
  }
  .center {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
</style>
