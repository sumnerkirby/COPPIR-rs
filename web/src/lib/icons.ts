// Lucide icons by kebab-case name ("radio-tower"), as stored in the taxonomy.
// The whole set ships so a facilitator can pick any icon for an asset type.

import { icons } from 'lucide';

export type IconNode = [tag: string, attrs: Record<string, string | number | undefined>][];

const pascal = (name: string) =>
  name
    .split('-')
    .map((p) => (p[0]?.toUpperCase() ?? '') + p.slice(1))
    .join('');

const table = icons as unknown as Record<string, IconNode>;

export function hasIcon(name: string): boolean {
  return pascal(name) in table;
}

export function iconNode(name: string): IconNode {
  return table[pascal(name)] ?? table.Circle ?? [];
}

let names: string[] | null = null;

/** Every icon name, kebab-case, for the picker. */
export function iconNames(): string[] {
  names ??= Object.keys(table).map((k) =>
    k
      .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
      .replace(/([A-Z])([A-Z][a-z])/g, '$1-$2')
      .replace(/([a-zA-Z])(\d)/g, '$1-$2')
      .toLowerCase(),
  );
  return names;
}

/** Trace an icon into a 2D canvas context, in its native 24×24 units. */
export function traceIcon(ctx: CanvasRenderingContext2D, node: IconNode) {
  const n = (v: string | number | undefined) => Number(v ?? 0);
  for (const [tag, a] of node) {
    switch (tag) {
      case 'path':
        ctx.stroke(new Path2D(String(a.d)));
        break;
      case 'circle':
        ctx.beginPath();
        ctx.arc(n(a.cx), n(a.cy), n(a.r), 0, Math.PI * 2);
        ctx.stroke();
        break;
      case 'ellipse':
        ctx.beginPath();
        ctx.ellipse(n(a.cx), n(a.cy), n(a.rx), n(a.ry), 0, 0, Math.PI * 2);
        ctx.stroke();
        break;
      case 'line':
        ctx.beginPath();
        ctx.moveTo(n(a.x1), n(a.y1));
        ctx.lineTo(n(a.x2), n(a.y2));
        ctx.stroke();
        break;
      case 'rect':
        ctx.beginPath();
        ctx.roundRect(n(a.x), n(a.y), n(a.width), n(a.height), n(a.rx));
        ctx.stroke();
        break;
      case 'polyline':
      case 'polygon': {
        const pts = String(a.points).trim().split(/[\s,]+/).map(Number);
        ctx.beginPath();
        for (let i = 0; i + 1 < pts.length; i += 2) {
          if (i === 0) ctx.moveTo(pts[i]!, pts[i + 1]!);
          else ctx.lineTo(pts[i]!, pts[i + 1]!);
        }
        if (tag === 'polygon') ctx.closePath();
        ctx.stroke();
        break;
      }
    }
  }
}
