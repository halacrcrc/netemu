// 界面双语（中文 / English）。课程、题库、实验说明等内容文本保持原文。
import { t as translate } from './i18n-dict';

export type Lang = 'zh' | 'en';
const KEY = 'netemu.lang';

function load(): Lang {
  try {
    return localStorage.getItem(KEY) === 'en' ? 'en' : 'zh';
  } catch {
    return 'zh';
  }
}

export const i18n = $state<{ lang: Lang }>({ lang: load() });

export function lang(): Lang {
  return i18n.lang;
}

export function setLang(l: Lang) {
  i18n.lang = l;
  try {
    localStorage.setItem(KEY, l);
  } catch {
    /* 忽略 */
  }
  document.documentElement.lang = l === 'zh' ? 'zh-CN' : 'en';
}

export function toggleLang() {
  setLang(i18n.lang === 'zh' ? 'en' : 'zh');
}

export function t(key: string): string {
  return translate(i18n.lang, key);
}
