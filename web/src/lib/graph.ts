// Layered auto-layout for the topology view: providers above the things that
// depend on them, so a dependency chain reads top to bottom.

import { Graph, layout } from '@dagrejs/dagre';
import type { Asset } from './types/Asset';
import type { GraphPoint } from './types/GraphPoint';
import type { Link } from './types/Link';

export const NODE_W = 150;
export const NODE_H = 80;

export function autoLayout(assets: Asset[], links: Link[]): Record<string, GraphPoint> {
  const g = new Graph();
  g.setGraph({ rankdir: 'TB', nodesep: 40, ranksep: 70, marginx: 20, marginy: 20 });
  g.setDefaultEdgeLabel(() => ({}));
  const ids = new Set(assets.map((a) => a.id));
  for (const a of assets) g.setNode(a.id, { width: NODE_W, height: NODE_H });
  for (const l of links) if (ids.has(l.from) && ids.has(l.to) && l.from !== l.to) g.setEdge(l.from, l.to);
  layout(g);
  const out: Record<string, GraphPoint> = {};
  for (const id of ids) {
    const n = g.node(id);
    // dagre gives centres; Svelte Flow positions top-left corners.
    out[id] = { x: Math.round(n.x - NODE_W / 2), y: Math.round(n.y - NODE_H / 2) };
  }
  return out;
}
