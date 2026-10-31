// 题库练习状态与持久化（答题记录 / 错题本 / 统计）。
// 本模块内对 quiz 的赋值都是同模块闭包内修改，响应性安全（ADR D9）。

import { QUESTIONS, type Question, type TopicId } from '../content/questions';

const KEY = 'netemu.quiz.v1';

export interface QuizRecord {
  last: 'right' | 'wrong';
  wrong: number;
  right: number;
  at: number;
}

export interface QuizStore {
  records: Record<string, QuizRecord>;
}

function load(): QuizStore {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as QuizStore;
      if (parsed && typeof parsed.records === 'object') return parsed;
    }
  } catch {
    /* 忽略损坏数据 */
  }
  return { records: {} };
}

export const quiz = $state({ ...load() });

function persist() {
  try {
    localStorage.setItem(KEY, JSON.stringify({ records: quiz.records }));
  } catch {
    /* 忽略 */
  }
}

export function recordAnswer(qid: string, correct: boolean) {
  const prev = quiz.records[qid];
  quiz.records[qid] = {
    last: correct ? 'right' : 'wrong',
    wrong: (prev?.wrong ?? 0) + (correct ? 0 : 1),
    right: (prev?.right ?? 0) + (correct ? 1 : 0),
    at: Date.now(),
  };
  persist();
}

// 云恢复后重新载入记录
window.addEventListener('netemu:backup-restored', () => {
  const fresh = load();
  quiz.records = fresh.records;
});

export function clearRecords() {
  quiz.records = {};
  persist();
}

// ---------- 统计 ----------

export function topicStats(topic: TopicId) {
  let attempted = 0;
  let correct = 0;
  for (const q of QUESTIONS.filter((x) => x.topic === topic)) {
    const r = quiz.records[q.id];
    if (r) {
      attempted += 1;
      if (r.last === 'right') correct += 1;
    }
  }
  return { attempted, total: QUESTIONS.filter((x) => x.topic === topic).length, correct };
}

export function overallStats() {
  let answered = 0;
  let correct = 0;
  let wrongBook = 0;
  for (const q of QUESTIONS) {
    const r = quiz.records[q.id];
    if (r) {
      answered += 1;
      if (r.last === 'right') correct += 1;
      else wrongBook += 1;
    }
  }
  return { answered, total: QUESTIONS.length, correct, wrongBook };
}

// ---------- 练习会话 ----------

export interface QuizSession {
  queue: Question[];
  idx: number;
  chosen: number | null;
  done: number;
  right: number;
}

export const session = $state({
  queue: [] as Question[],
  idx: 0,
  chosen: null as number | null,
  done: 0,
  right: 0,
  /** 当前练习模式：主题过滤或错题本 */
  mode: '' as string,
});

export function startPractice(topic: TopicId | 'wrong' | 'all') {
  let pool = [...QUESTIONS];
  if (topic === 'wrong') {
    pool = pool.filter((q) => quiz.records[q.id]?.last === 'wrong');
  } else if (topic !== 'all') {
    pool = pool.filter((q) => q.topic === topic);
  }
  // 洗牌
  for (let i = pool.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [pool[i], pool[j]] = [pool[j], pool[i]];
  }
  session.queue = pool;
  session.idx = 0;
  session.chosen = null;
  session.done = 0;
  session.right = 0;
  session.mode = topic;
}

export function choose(idx: number) {
  if (session.chosen !== null) return; // 已作答
  session.chosen = idx;
  const q = session.queue[session.idx];
  if (!q) return;
  const correct = idx === q.answer;
  recordAnswer(q.id, correct);
  session.done += 1;
  if (correct) session.right += 1;
}

export function nextQuestion() {
  if (session.idx < session.queue.length - 1) {
    session.idx += 1;
    session.chosen = null;
  } else {
    // 结束：清空会话
    session.queue = [];
    session.idx = 0;
    session.chosen = null;
  }
}
