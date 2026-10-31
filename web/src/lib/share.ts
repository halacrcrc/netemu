// 拓扑分享：把工作区压缩后放进 URL hash（#topo=...）。
// 使用浏览器原生 CompressionStream（deflate-raw），不支持时退回纯 base64。

import type { Workspace } from './persistence';

function toBase64Url(bytes: Uint8Array): string {
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function fromBase64Url(s: string): Uint8Array {
  const b64 = s.replace(/-/g, '+').replace(/_/g, '/');
  const pad = b64.length % 4 === 0 ? '' : '='.repeat(4 - (b64.length % 4));
  const bin = atob(b64 + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

async function deflate(bytes: Uint8Array): Promise<Uint8Array> {
  const buf = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
  const stream = new Blob([buf]).stream().pipeThrough(new CompressionStream('deflate-raw'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function inflate(bytes: Uint8Array): Promise<Uint8Array> {
  const buf = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
  const stream = new Blob([buf]).stream().pipeThrough(new DecompressionStream('deflate-raw'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

export async function encodeWorkspace(ws: Workspace): Promise<string> {
  const bytes = new TextEncoder().encode(JSON.stringify(ws));
  if (typeof CompressionStream !== 'undefined') {
    return 'z' + toBase64Url(await deflate(bytes));
  }
  return 'j' + toBase64Url(bytes);
}

export async function decodeWorkspace(token: string): Promise<Workspace> {
  const kind = token[0];
  if (kind !== 'z' && kind !== 'j') throw new Error('未知的分享格式');
  const raw = fromBase64Url(token.slice(1));
  const bytes = kind === 'z' ? await inflate(raw) : raw;
  const ws = JSON.parse(new TextDecoder().decode(bytes)) as Workspace;
  if (!ws.net || !Array.isArray(ws.net.devices)) throw new Error('分享数据格式不对');
  return ws;
}

/** 由当前页面地址生成分享链接（保留部署子路径）。 */
export async function buildShareUrl(ws: Workspace): Promise<string> {
  const token = await encodeWorkspace(ws);
  const base = `${location.origin}${location.pathname}`;
  return `${base}#topo=${token}`;
}
