<script lang="ts">
  // 题库视图：练习 / 错题本 / 统计 三个标签页。
  import { QUESTIONS, TOPICS, type TopicId } from '../content/questions';
  import {
    quiz,
    session,
    startPractice,
    choose,
    nextQuestion,
    topicStats,
    overallStats,
    clearRecords,
  } from '../lib/quiz.svelte';

  let tab = $state<'practice' | 'stats'>('practice');

  const current = $derived(session.queue[session.idx] ?? null);
  const finished = $derived(session.queue.length > 0 && session.idx >= session.queue.length);
  const overall = $derived(overallStats());

  function labelOf(t: TopicId): string {
    return TOPICS.find((x) => x.id === t)?.name ?? t;
  }
</script>

<div class="quiz-view">
  <div class="tabs">
    <button class:active={tab === 'practice'} onclick={() => (tab = 'practice')}>练习</button>
    <button class:active={tab === 'stats'} onclick={() => (tab = 'stats')}>
      统计 <span class="badge">{overall.answered}/{overall.total}</span>
    </button>
  </div>

  {#if tab === 'practice'}
    <div class="body">
      {#if !current}
        <!-- 未开始 / 已完成：选题入口 -->
        {#if finished}
          <section class="panel done-card">
            <h3>本轮完成</h3>
            <div class="pad">
              <div class="score">
                {session.right} / {session.done}
                <span class="pct">
                  {session.done ? Math.round((session.right / session.done) * 100) : 0}%
                </span>
              </div>
              <div class="hint">
                {#if session.right === session.done}
                  全对！可以换个主题继续，或去错题本巩固。
                {:else}
                  错题已进入错题本，建议从"错题重做"再过一遍。
                {/if}
              </div>
            </div>
          </section>
        {/if}
        <section class="panel">
          <h3>选择练习范围</h3>
          <div class="pad topic-grid">
            <button class="topic" onclick={() => startPractice('all')}>
              全部题目<span class="count">{QUESTIONS.length} 题</span>
            </button>
            {#each TOPICS as t (t.id)}
              {@const st = topicStats(t.id)}
              <button class="topic" onclick={() => startPractice(t.id)}>
                {t.name}
                <span class="count">
                  {st.attempted}/{st.total}
                  {#if st.attempted > 0}
                    · {Math.round((st.correct / st.attempted) * 100)}%正确
                  {/if}
                </span>
              </button>
            {/each}
            <button class="topic wrong" disabled={overall.wrongBook === 0} onclick={() => startPractice('wrong')}>
              错题重做<span class="count">{overall.wrongBook} 题待巩固</span>
            </button>
          </div>
        </section>
      {:else}
        <!-- 答题卡 -->
        <section class="panel q-card">
          <div class="q-head">
            <span class="tag">{labelOf(current.topic)}</span>
            <span class="pos">{session.idx + 1} / {session.queue.length}</span>
            <span class="score-inline">本轮 {session.right}/{session.done}</span>
          </div>
          <div class="q-text">{current.q}</div>
          <div class="options">
            {#each current.options as opt, i (i)}
              {@const picked = session.chosen === i}
              {@const isAnswer = i === current.answer}
              {@const revealed = session.chosen !== null}
              <button
                class="opt"
                class:picked={picked && !revealed}
                class:good={revealed && isAnswer}
                class:bad={revealed && picked && !isAnswer}
                class:dim={revealed && !picked && !isAnswer}
                onclick={() => choose(i)}
              >
                <span class="key">{String.fromCharCode(65 + i)}</span>
                {opt}
              </button>
            {/each}
          </div>
          {#if session.chosen !== null}
            {@const ok = session.chosen === current.answer}
            <div class="explain" class:good={ok} class:bad={!ok}>
              <b>{ok ? '✓ 回答正确' : '✗ 回答错误'}</b>
              <p>{current.explain}</p>
              <button class="primary" onclick={nextQuestion}>
                {session.idx < session.queue.length - 1 ? '下一题' : '完成本轮'}
              </button>
            </div>
          {/if}
        </section>
      {/if}
    </div>
  {:else}
    <div class="body">
      <section class="panel">
        <h3>总体进度</h3>
        <div class="pad stats-grid">
          <div class="stat">
            <div class="num">{overall.answered}<span class="of">/{overall.total}</span></div>
            <div class="lbl">已作答题目</div>
          </div>
          <div class="stat">
            <div class="num">{overall.wrongBook}</div>
            <div class="lbl">错题本待巩固</div>
          </div>
          <div class="stat">
            <div class="num">
              {overall.answered ? Math.round((overall.correct / overall.answered) * 100) : 0}%
            </div>
            <div class="lbl">最近正确率</div>
          </div>
        </div>
        <div class="pad">
          <button class="danger" onclick={() => { if (confirm('清空全部答题记录与错题本？')) clearRecords(); }}>
            清空答题记录
          </button>
        </div>
      </section>
      <section class="panel">
        <h3>分主题掌握度</h3>
        <div class="pad">
          {#each TOPICS as t (t.id)}
            {@const st = topicStats(t.id)}
            {@const pct = st.attempted ? Math.round((st.correct / st.attempted) * 100) : 0}
            <div class="topic-row">
              <span class="tname">{t.name}</span>
              <div class="bar">
                <div class="fill" style={`width:${pct}%`}></div>
              </div>
              <span class="tnum mono">{st.correct}/{st.total}</span>
            </div>
          {/each}
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  .quiz-view {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  .tabs {
    display: flex;
    gap: 8px;
    padding: 12px 16px 0;
  }
  .tabs button {
    min-width: 96px;
  }
  .tabs button.active {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--btn-hover-bg);
  }
  .badge {
    opacity: 0.7;
    font-size: 11px;
  }
  .body {
    padding: 12px 16px 24px;
    max-width: 720px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .topic-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 8px;
  }
  .topic {
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
  }
  .topic .count {
    font-size: 11px;
    color: var(--text-dim);
  }
  .topic.wrong:not(:disabled) {
    border-color: var(--bad);
  }
  .q-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 0;
    font-size: 12px;
    color: var(--text-dim);
  }
  .tag {
    background: var(--bg-panel-2);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 8px;
    color: var(--accent);
  }
  .pos {
    margin-left: auto;
    font-family: Consolas, monospace;
  }
  .q-text {
    padding: 8px 12px 4px;
    font-size: 15px;
    line-height: 1.6;
  }
  .options {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 12px 12px;
  }
  .opt {
    text-align: left;
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 9px 12px;
    line-height: 1.5;
  }
  .opt .key {
    font-family: Consolas, monospace;
    color: var(--text-dim);
    font-weight: 700;
  }
  .opt.picked {
    border-color: var(--accent);
  }
  .opt.good {
    border-color: var(--good);
    color: var(--good);
  }
  .opt.bad {
    border-color: var(--bad);
    color: var(--bad);
  }
  .opt.dim {
    opacity: 0.55;
  }
  .explain {
    margin: 0 12px 12px;
    border-top: 1px solid var(--border);
    padding-top: 10px;
    font-size: 13px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .explain p {
    margin: 0;
    color: var(--text-dim);
    line-height: 1.6;
  }
  .explain button {
    align-self: flex-start;
  }
  .explain.good b {
    color: var(--good);
  }
  .explain.bad b {
    color: var(--bad);
  }
  .done-card .score {
    font-size: 28px;
    font-weight: 700;
    color: var(--accent);
  }
  .done-card .pct {
    font-size: 14px;
    color: var(--text-dim);
    margin-left: 8px;
  }
  .stats-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .stat {
    text-align: center;
    padding: 8px 0;
  }
  .stat .num {
    font-size: 24px;
    font-weight: 700;
    color: var(--accent);
  }
  .stat .of {
    font-size: 13px;
    color: var(--text-dim);
  }
  .stat .lbl {
    font-size: 12px;
    color: var(--text-dim);
    margin-top: 2px;
  }
  .topic-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .tname {
    width: 72px;
    font-size: 13px;
  }
  .bar {
    flex: 1;
    height: 8px;
    background: var(--bg-panel-2);
    border-radius: 4px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 4px;
  }
  .tnum {
    font-size: 12px;
    color: var(--text-dim);
    width: 48px;
    text-align: right;
  }
</style>
