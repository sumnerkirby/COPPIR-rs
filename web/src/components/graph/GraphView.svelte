<script lang="ts">
  // The topology canvas: the same assets and links as the map, laid out by how
  // they depend on each other instead of where they are. Positions are saved
  // with the exercise, so every window sees the same arrangement.
  import '@xyflow/svelte/dist/style.css';
  import {
    Background,
    Controls,
    MarkerType,
    SvelteFlow,
    type Connection,
    type Edge,
    type Node,
  } from '@xyflow/svelte';
  import { autoLayout } from '../../lib/graph';
  import { statusValue } from '../../lib/sitrep';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { GraphPoint } from '../../lib/types/GraphPoint';
  import Icon from '../ui/Icon.svelte';
  import AssetNode from './AssetNode.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const nodeTypes = { asset: AssetNode };

  // Where assets without a saved position would go, so nothing piles up at 0,0.
  const fallback = $derived.by((): Record<string, GraphPoint> => {
    const missing = ws.visible.some((a) => !a.graph);
    return missing ? autoLayout(ws.visible, Object.values(ex.links)) : {};
  });

  // Svelte Flow owns these while the user drags; they are rebuilt from the
  // exercise whenever it changes.
  let nodes = $state.raw<Node[]>([]);
  let edges = $state.raw<Edge[]>([]);

  $effect(() => {
    const sets = ex.taxonomy.status_sets;
    const fill = sets.find((s) => s.display === 'fill');
    const ring = sets.find((s) => s.display === 'ring');
    nodes = ws.visible.map((a) => {
      const type = ex.taxonomy.asset_types.find((t) => t.id === a.type_id);
      const f = fill ? statusValue(fill, a) : undefined;
      const r = ring ? statusValue(ring, a) : undefined;
      return {
        id: a.id,
        type: 'asset',
        position: a.graph ?? fallback[a.id] ?? { x: 0, y: 0 },
        selected: a.id === ws.selected,
        draggable: !ws.readonly,
        data: {
          name: a.name,
          icon: type?.icon ?? 'circle',
          fill: a.color ?? f?.color ?? type?.color ?? '#888888',
          ring: r?.color ?? '#ffffff',
          ringStyle: r?.glyph ?? 'solid',
          glyph: f?.glyph ?? '',
          status: sets.map((s) => statusValue(s, a)?.label).join(', '),
          connectable: !ws.readonly,
        },
      };
    });
    const shown = new Set(ws.visible.map((a) => a.id));
    edges = Object.values(ex.links)
      .filter((l) => shown.has(l.from) && shown.has(l.to))
      .map((l) => {
        const depends = l.kind === 'depends_on';
        const color = depends ? '#ffaa00' : l.kind === 'managed_by' ? '#aa88ff' : '#00ff41';
        return {
          id: l.id,
          source: l.from,
          target: l.to,
          label: l.label || undefined,
          animated: depends,
          style: `stroke: ${color}; stroke-width: 2px;`,
          markerEnd: { type: MarkerType.ArrowClosed, color },
        };
      });
  });

  async function dragged({ nodes: moved }: { nodes: Node[] }) {
    if (ws.readonly || !moved.length) return;
    const positions = Object.fromEntries(moved.map((n) => [n.id, { x: Math.round(n.position.x), y: Math.round(n.position.y) }]));
    await ws.send({ type: 'set_graph_positions', positions });
  }

  async function connect(c: Connection) {
    if (ws.readonly || c.source === c.target) return;
    // Drawn from provider to dependant: "target depends on source".
    await ws.send({ type: 'add_link', from: c.source, to: c.target, kind: 'depends_on' });
  }

  async function arrange() {
    const ok = await ui.confirm('Arrange the graph automatically?', {
      body: 'Every asset is placed by its dependencies, providers above dependants. Undo puts the old layout back.',
      confirm: 'Arrange',
    });
    if (ok) await ws.send({ type: 'set_graph_positions', positions: autoLayout(ws.visible, Object.values(ex.links)) });
  }

  const light = $derived(ex.branding.theme === 'daylight');
</script>

<div class="graph">
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    fitView
    colorMode={light ? 'light' : 'dark'}
    nodesConnectable={!ws.readonly}
    elementsSelectable
    minZoom={0.1}
    onnodeclick={({ node }) => ws.select(node.id)}
    onpaneclick={() => ws.select(null)}
    onnodedragstop={dragged}
    onconnect={connect}
    proOptions={{ hideAttribution: true }}
  >
    <Background gap={24} />
    <Controls showLock={false} />
  </SvelteFlow>
  {#if !ws.readonly}
    <div class="tools">
      <button class="icon" onclick={arrange} title="Lay the graph out by dependencies"><Icon name="network" size={14} /> Arrange</button>
    </div>
    <p class="hint muted">Drag a node to move it. Drag from its lower dot to another node to add a dependency.</p>
  {/if}
</div>

<style>
  .graph { position: absolute; inset: 0; }
  .graph :global(.svelte-flow) { background: var(--bg); }
  .graph :global(.svelte-flow__node-asset) { background: none; border: 0; padding: 0; }
  .graph :global(.svelte-flow__edge-text) { fill: var(--text); font-size: 11px; }
  .graph :global(.svelte-flow__edge-textbg) { fill: var(--panel); }
  .tools { position: absolute; top: 10px; right: 10px; z-index: 5; }
  .hint {
    position: absolute; bottom: 6px; left: 50%; transform: translateX(-50%); z-index: 5; margin: 0; font-size: 11px;
    white-space: nowrap; pointer-events: none;
  }
</style>
