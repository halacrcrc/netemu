// 生成 PWA 图标（纯 Node，无外部依赖）：暗色底 + 网络节点六边形。
import { deflateSync } from 'node:zlib';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, '..', 'public', 'icons');
mkdirSync(outDir, { recursive: true });

const BG = [11, 18, 32];
const TEAL = [45, 212, 191];
const INK = [226, 232, 240];
const LINE = [100, 116, 139];

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const mix = (a, b, t) => a + (b - a) * t;
const smooth = (edge0, edge1, x) => {
  const t = clamp((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
};

function sdSegment(px, py, ax, ay, bx, by) {
  const vx = bx - ax;
  const vy = by - ay;
  const wx = px - ax;
  const wy = py - ay;
  const t = clamp((wx * vx + wy * vy) / (vx * vx + vy * vy));
  const dx = wx - vx * t;
  const dy = wy - vy * t;
  return Math.hypot(dx, dy);
}

function sdPolygon(px, py, pts) {
  let d = Infinity;
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i];
    const [xj, yj] = pts[j];
    d = Math.min(d, sdSegment(px, py, xi, yi, xj, yj));
    if (yi > py !== yj > py && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside ? -d : d;
}

function draw(size) {
  const buf = Buffer.alloc(size * size * 4);
  const c = size / 2;
  const s = size / 512;
  // 六边形（尖顶朝上）
  const hex = [];
  for (let i = 0; i < 6; i++) {
    const a = -Math.PI / 2 + (i * Math.PI) / 3;
    hex.push([c + Math.cos(a) * 168 * s, c + Math.sin(a) * 168 * s]);
  }
  const nodes = [hex[0], hex[2], hex[4]];
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      // 背景 + 中心微光
      const glow = smooth(size * 0.62, 0, Math.hypot(x - c, y - c)) * 0.16;
      let r = mix(BG[0], TEAL[0], glow);
      let g = mix(BG[1], TEAL[1], glow);
      let b = mix(BG[2], TEAL[2], glow);
      // 六边形描边
      const dHex = Math.abs(sdPolygon(x, y, hex));
      const hexA = 1 - smooth(6 * s, 10 * s, dHex);
      if (hexA > 0) {
        r = mix(r, TEAL[0], hexA * 0.85);
        g = mix(g, TEAL[1], hexA * 0.85);
        b = mix(b, TEAL[2], hexA * 0.85);
      }
      // 节点之间的连线
      let lineA = 0;
      for (let i = 0; i < nodes.length; i++) {
        const a = nodes[i];
        const bb = nodes[(i + 1) % nodes.length];
        lineA = Math.max(lineA, 1 - smooth(4 * s, 7 * s, sdSegment(x, y, a[0], a[1], bb[0], bb[1])));
      }
      if (lineA > 0) {
        r = mix(r, LINE[0], lineA * 0.9);
        g = mix(g, LINE[1], lineA * 0.9);
        b = mix(b, LINE[2], lineA * 0.9);
      }
      // 节点圆点（底部节点用青色强调）
      for (let i = 0; i < nodes.length; i++) {
        const [nx, ny] = nodes[i];
        const rad = 30 * s;
        const a = 1 - smooth(rad - 1.5 * s, rad + 1.5 * s, Math.hypot(x - nx, y - ny));
        if (a > 0) {
          const col = i === 2 ? TEAL : INK;
          r = mix(r, col[0], a);
          g = mix(g, col[1], a);
          b = mix(b, col[2], a);
        }
      }
      const o = (y * size + x) * 4;
      buf[o] = Math.round(r);
      buf[o + 1] = Math.round(g);
      buf[o + 2] = Math.round(b);
      buf[o + 3] = 255;
    }
  }
  return buf;
}

const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const byte of buf) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function png(rgba, size) {
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    rgba.copy(raw, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

for (const size of [192, 512]) {
  const file = join(outDir, `icon-${size}.png`);
  writeFileSync(file, png(draw(size), size));
  console.log('wrote', file);
}
