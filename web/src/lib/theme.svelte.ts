// 主题状态：暗色「机房夜航」（默认）/ 亮色「工程图纸」（暖纸白）。

export type ThemeName = 'light' | 'dark';

const KEY = 'netemu.theme';

function initial(): ThemeName {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved === 'light' || saved === 'dark') return saved;
  } catch {
    /* localStorage 不可用时用默认 */
  }
  return 'dark';
}

export const theme = $state({ name: initial() as ThemeName });

export function applyTheme() {
  document.documentElement.dataset.theme = theme.name;
}

export function toggleTheme() {
  theme.name = theme.name === 'dark' ? 'light' : 'dark';
  try {
    localStorage.setItem(KEY, theme.name);
  } catch {
    /* 忽略持久化失败 */
  }
  applyTheme();
}

// 模块加载即应用（index.html 的内联脚本已先行设置，这里保持一致）
applyTheme();
