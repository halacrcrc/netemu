// 课程内容：课程列表 + frontmatter + Markdown 加载。

export interface Lesson {
  id: string;
  title: string;
  order: number;
  topic: string;
  /** 关联实验 id（labs.ts） */
  lab: string | null;
  md: string;
}

// Vite 按原始文本批量导入课程 Markdown
const files = import.meta.glob('../content/lessons/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

function parseFrontmatter(md: string): { meta: Record<string, string>; body: string } {
  const m = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/.exec(md);
  if (!m) return { meta: {}, body: md };
  const meta: Record<string, string> = {};
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.split(':');
    if (kv.length >= 2) {
      meta[kv[0].trim()] = kv.slice(1).join(':').trim();
    }
  }
  return { meta, body: md.slice(m[0].length) };
}

export const LESSONS: Lesson[] = Object.entries(files)
  .map(([path, raw]) => {
    const { meta, body } = parseFrontmatter(raw);
    const id = path.split('/').pop()?.replace(/\.md$/, '') ?? path;
    return {
      id,
      title: meta.title ?? id,
      order: Number(meta.order ?? 99),
      topic: meta.topic ?? '',
      lab: meta.lab || null,
      md: body,
    };
  })
  .sort((a, b) => a.order - b.order);
