// 课程内容：课程列表 + frontmatter + Markdown 加载。

export type LessonLevel = 'IA' | 'IP' | 'IE' | 'MAP';

export interface Lesson {
  id: string;
  title: string;
  order: number;
  topic: string;
  /** 课程分级：IA 初级 / IP 中级 / IE 高级 */
  level: LessonLevel;
  /** 关联实验 id（labs.ts） */
  lab: string | null;
  md: string;
  /** 英文译文（存在时 EN 界面使用） */
  titleEn: string | null;
  mdEn: string | null;
}

// Vite 按原始文本批量导入课程 Markdown
const files = import.meta.glob('../content/lessons/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const filesEn = import.meta.glob('../content/lessons-en/*.md', {
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
      const value = kv.slice(1).join(':').trim();
      meta[kv[0].trim()] = value.replace(/^(["'])(.*)\1$/, '$2');
    }
  }
  return { meta, body: md.slice(m[0].length) };
}

export const LESSONS: Lesson[] = Object.entries(files)
  .map(([path, raw]) => {
    const { meta, body } = parseFrontmatter(raw);
    const id = path.split('/').pop()?.replace(/\.md$/, '') ?? path;
    const level: LessonLevel =
      meta.level === 'IP' || meta.level === 'IE' || meta.level === 'MAP' ? meta.level : 'IA';
    return {
      id,
      title: meta.title ?? id,
      order: Number(meta.order ?? 99),
      topic: meta.topic ?? '',
      level,
      lab: meta.lab || null,
      md: body,
      titleEn: null,
      mdEn: null,
    };
  })
  .sort((a, b) => a.order - b.order);

// 合并英文译文
{
  const en = new Map<string, { title: string; md: string }>();
  for (const [path, raw] of Object.entries(filesEn)) {
    const { meta, body } = parseFrontmatter(raw);
    const id = path.split('/').pop()?.replace(/\.md$/, '') ?? path;
    en.set(id, { title: meta.title ?? '', md: body });
  }
  for (const l of LESSONS) {
    const e = en.get(l.id);
    if (e) {
      l.titleEn = e.title;
      l.mdEn = e.md;
    }
  }
}

export function lessonTitle(l: Lesson, english: boolean): string {
  return english && l.titleEn ? l.titleEn : l.title;
}

export function lessonMarkdown(l: Lesson, english: boolean): string {
  return english && l.mdEn ? l.mdEn : l.md;
}
