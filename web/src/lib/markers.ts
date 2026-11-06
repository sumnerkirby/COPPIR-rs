// Marker images for the map, drawn on a canvas and handed to MapLibre.
//
// A marker carries status twice over, so it reads without colour:
//   - fill colour + glyph badge  = the "fill" status set (Security by default)
//   - ring colour + ring style   = the "ring" status set (Operational)
// plus the asset type's icon. Every input is encoded in the image id, so the
// map can ask for any combination on demand (styleimagemissing) and identical
// markers share one image.

import { iconNode, traceIcon } from './icons';

export interface MarkerSpec {
  icon: string;
  fill: string;
  ring: string;
  /** solid | dashed | double | dotted; anything else draws solid. */
  ringStyle: string;
  glyph: string;
  selected: boolean;
}

const PREFIX = 'mk|';

export function markerId(m: MarkerSpec): string {
  return PREFIX + [m.icon, m.fill, m.ring, m.ringStyle, encodeURIComponent(m.glyph), m.selected ? 1 : 0].join('|');
}

export function parseMarkerId(id: string): MarkerSpec | null {
  if (!id.startsWith(PREFIX)) return null;
  const [icon, fill, ring, ringStyle, glyph, sel] = id.slice(PREFIX.length).split('|');
  if (icon === undefined || fill === undefined || ring === undefined) return null;
  return {
    icon,
    fill,
    ring,
    ringStyle: ringStyle ?? 'solid',
    glyph: decodeURIComponent(glyph ?? ''),
    selected: sel === '1',
  };
}

/** Relative luminance, to pick a legible icon colour on the fill. */
function isLight(hex: string): boolean {
  const n = parseInt(hex.slice(1), 16);
  if (Number.isNaN(n)) return false;
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b! > 0.45;
}

export const MARKER_SIZE = 36;

export function drawMarker(m: MarkerSpec, pixelRatio = 2): ImageData {
  const size = MARKER_SIZE * pixelRatio;
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.scale(pixelRatio, pixelRatio);
  const c = MARKER_SIZE / 2;

  if (m.selected) {
    ctx.beginPath();
    ctx.arc(c, c, 17, 0, Math.PI * 2);
    ctx.fillStyle = 'rgba(255,255,255,0.35)';
    ctx.fill();
  }

  // Ring
  ctx.strokeStyle = m.ring;
  ctx.lineWidth = 3;
  ctx.setLineDash(m.ringStyle === 'dashed' ? [5, 3] : m.ringStyle === 'dotted' ? [1.5, 3] : []);
  ctx.lineCap = m.ringStyle === 'dotted' ? 'round' : 'butt';
  ctx.beginPath();
  ctx.arc(c, c, 14, 0, Math.PI * 2);
  ctx.stroke();
  if (m.ringStyle === 'double') {
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(c, c, 16.5, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.setLineDash([]);

  // Fill
  ctx.beginPath();
  ctx.arc(c, c, 11.5, 0, Math.PI * 2);
  ctx.fillStyle = m.fill;
  ctx.fill();
  ctx.lineWidth = 1;
  ctx.strokeStyle = 'rgba(0,0,0,0.6)';
  ctx.stroke();

  // Icon: 24-unit Lucide glyph scaled to 13px.
  const s = 13 / 24;
  ctx.save();
  ctx.translate(c - 12 * s, c - 12 * s);
  ctx.scale(s, s);
  ctx.strokeStyle = isLight(m.fill) ? '#0b0f0c' : '#ffffff';
  ctx.lineWidth = 2.4;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  traceIcon(ctx, iconNode(m.icon));
  ctx.restore();

  // Glyph badge, top right.
  if (m.glyph) {
    const bx = c + 10;
    const by = c - 10;
    ctx.beginPath();
    ctx.arc(bx, by, 6.5, 0, Math.PI * 2);
    ctx.fillStyle = '#0b0f0c';
    ctx.fill();
    ctx.strokeStyle = m.fill;
    ctx.lineWidth = 1.5;
    ctx.stroke();
    ctx.fillStyle = '#ffffff';
    ctx.font = 'bold 9px ui-sans-serif, system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(m.glyph, bx, by + 0.5);
  }

  return ctx.getImageData(0, 0, size, size);
}
