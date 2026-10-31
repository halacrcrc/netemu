// 笔记存储 + GitHub Gist 云同步（经典 PAT，直连 api.github.com）。
// 同步范围：笔记 + 题库记录 + 实验进度，聚合为一个私有 gist 文件。

export interface Note {
  id: string;
  title: string;
  body: string;
  at: number;
}

const NOTES_KEY = 'netemu.notes.v1';
const TOKEN_KEY = 'netemu.gist.token';
const GIST_ID_KEY = 'netemu.gist.id';

export function loadNotes(): Note[] {
  try {
    const raw = localStorage.getItem(NOTES_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Note[];
      if (Array.isArray(parsed)) return parsed;
    }
  } catch {
    /* 忽略 */
  }
  return [];
}

export function saveNotes(notes: Note[]) {
  try {
    localStorage.setItem(NOTES_KEY, JSON.stringify(notes));
  } catch {
    /* 忽略 */
  }
}

export function getToken(): string {
  try {
    return localStorage.getItem(TOKEN_KEY) ?? '';
  } catch {
    return '';
  }
}

export function saveToken(t: string) {
  try {
    localStorage.setItem(TOKEN_KEY, t);
  } catch {
    /* 忽略 */
  }
}

export function getGistId(): string {
  try {
    return localStorage.getItem(GIST_ID_KEY) ?? '';
  } catch {
    return '';
  }
}

export function saveGistId(id: string) {
  try {
    localStorage.setItem(GIST_ID_KEY, id);
  } catch {
    /* 忽略 */
  }
}

/** 聚合要备份的数据（笔记 + 题库记录 + 实验进度）。 */
export function collectBackup(): Record<string, unknown> {
  const quizRaw = localStorage.getItem('netemu.quiz.v1');
  const labRaw = localStorage.getItem('netemu.labs.v1');
  return {
    version: 1,
    at: new Date().toISOString(),
    notes: loadNotes(),
    quiz: quizRaw ? JSON.parse(quizRaw) : {},
    labs: labRaw ? JSON.parse(labRaw) : {},
  };
}

/** 应用恢复的数据（笔记直接覆盖；题库/实验进度由调用方决定策略后写回）。 */
export function applyBackup(data: Record<string, unknown>) {
  if (Array.isArray(data.notes)) {
    saveNotes(data.notes as Note[]);
  }
  if (data.quiz && typeof data.quiz === 'object') {
    // 按答题次数合并（次数多者保留）
    try {
      const local = JSON.parse(localStorage.getItem('netemu.quiz.v1') ?? '{}');
      const remote = data.quiz as { records: Record<string, QuizRec> };
      const merged: Record<string, QuizRec> = { ...local.records };
      for (const [qid, r] of Object.entries(remote.records ?? {})) {
        const cur = merged[qid];
        if (!cur || r.wrong + r.right > cur.wrong + cur.right) {
          merged[qid] = r;
        }
      }
      localStorage.setItem(
        'netemu.quiz.v1',
        JSON.stringify({ records: merged })
      );
    } catch {
      /* 忽略 */
    }
  }
  if (data.labs && typeof data.labs === 'object') {
    try {
      const local = JSON.parse(localStorage.getItem('netemu.labs.v1') ?? '{}');
      localStorage.setItem(
        'netemu.labs.v1',
        JSON.stringify({ ...local, ...(data.labs as object) })
      );
    } catch {
      /* 忽略 */
    }
  }
}

interface QuizRec {
  last: 'right' | 'wrong';
  wrong: number;
  right: number;
  at: number;
}

const API = 'https://api.github.com';

/** 备份到 Gist（无 id 则新建私有 gist）。返回 gist id。 */
export async function backupToGist(token: string): Promise<string> {
  if (!token) throw new Error('请先粘贴 GitHub Token（经典 PAT，勾选 gist 权限）');
  const body = JSON.stringify(collectBackup(), null, 2);
  const gistId = getGistId();
  const payload = {
    description: `网工模拟器备份 ${new Date().toISOString().slice(0, 10)}`,
    public: false,
    files: { 'netemu-backup.json': { content: body } },
  };
  const url = gistId ? `${API}/gists/${gistId}` : `${API}/gists`;
  const resp = await fetch(url, {
    method: gistId ? 'PATCH' : 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: 'application/vnd.github+json',
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });
  if (resp.status === 401 || resp.status === 403) {
    throw new Error('Token 无效或缺少 gist 权限（需要经典 PAT）');
  }
  if (!resp.ok) {
    throw new Error(`GitHub API ${resp.status}`);
  }
  const data = (await resp.json()) as { id: string };
  saveGistId(data.id);
  return data.id;
}

/** 从 Gist 恢复。返回备份数据。 */
export async function restoreFromGist(token: string): Promise<Record<string, unknown>> {
  const gistId = getGistId();
  if (!gistId) throw new Error('还没有备份过：先执行一次备份');
  if (!token) throw new Error('请先粘贴 GitHub Token');
  const resp = await fetch(`${API}/gists/${gistId}`, {
    headers: { Authorization: `Bearer ${token}`, Accept: 'application/vnd.github+json' },
  });
  if (resp.status === 404) throw new Error('找不到备份 gist（可能已被删除）');
  if (resp.status === 401 || resp.status === 403) throw new Error('Token 无效');
  if (!resp.ok) throw new Error(`GitHub API ${resp.status}`);
  const data = (await resp.json()) as {
    files: Record<string, { content?: string }>;
  };
  const file = data.files['netemu-backup.json'];
  if (!file?.content) throw new Error('备份文件为空');
  return JSON.parse(file.content) as Record<string, unknown>;
}
