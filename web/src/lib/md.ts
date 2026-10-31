// 极简 Markdown 渲染器：覆盖课程与笔记所需的子集。
// 支持：##/###/#### 标题、- 无序列表、1. 有序列表、``` 代码块、**加粗**、`行内代码`、段落。
// 所有 HTML 先转义，内容为本地/用户自写，安全可控。

function esc(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function inline(s: string): string {
  let out = esc(s);
  // 行内代码
  out = out.replace(/`([^`]+)`/g, '<code class="md-code">$1</code>');
  // 加粗
  out = out.replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>');
  return out;
}

export function renderMarkdown(src: string): string {
  const lines = src.split(/\r?\n/);
  const out: string[] = [];
  let inCode = false;
  let codeBuf: string[] = [];
  let listMode: 'ul' | 'ol' | null = null;
  let paraBuf: string[] = [];

  const closeList = () => {
    if (listMode) {
      out.push(`</${listMode}>`);
      listMode = null;
    }
  };
  const closePara = () => {
    if (paraBuf.length) {
      out.push(`<p>${inline(paraBuf.join(' '))}</p>`);
      paraBuf = [];
    }
  };

  for (const raw of lines) {
    // 代码围栏
    if (raw.trimStart().startsWith('```')) {
      if (inCode) {
        out.push(
          `<pre class="md-pre">${esc(codeBuf.join('\n'))}</pre>`
        );
        codeBuf = [];
        inCode = false;
      } else {
        closePara();
        closeList();
        inCode = true;
      }
      continue;
    }
    if (inCode) {
      codeBuf.push(raw);
      continue;
    }
    const line = raw.trimEnd();
    // 标题
    const h = /^(#{2,4})\s+(.*)$/.exec(line);
    if (h) {
      closePara();
      closeList();
      const level = h[1].length;
      out.push(`<h${level} class="md-h${level}">${inline(h[2])}</h${level}>`);
      continue;
    }
    // 无序列表
    const ul = /^\s*[-*]\s+(.*)$/.exec(line);
    if (ul) {
      closePara();
      if (listMode !== 'ul') {
        closeList();
        out.push('<ul class="md-ul">');
        listMode = 'ul';
      }
      out.push(`<li>${inline(ul[1])}</li>`);
      continue;
    }
    // 有序列表
    const ol = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    if (ol) {
      closePara();
      if (listMode !== 'ol') {
        closeList();
        out.push('<ol class="md-ol">');
        listMode = 'ol';
      }
      out.push(`<li>${inline(ol[1])}</li>`);
      continue;
    }
    // 空行
    if (line.trim() === '') {
      closePara();
      closeList();
      continue;
    }
    // 普通段落
    closeList();
    paraBuf.push(line.trim());
  }
  // 收尾
  if (inCode && codeBuf.length) {
    out.push(`<pre class="md-pre">${esc(codeBuf.join('\n'))}</pre>`);
  }
  closePara();
  closeList();
  return out.join('\n');
}
