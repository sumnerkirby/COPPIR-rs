<script lang="ts">
  import { api } from '../../lib/api';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Place } from '../../lib/types/Place';
  import type { Shape } from '../../lib/types/Shape';
  import Icon from '../ui/Icon.svelte';

  const ws = useWorkspace();
  let open = $state<'draw' | 'layers' | null>(null);

  // Place search
  let query = $state('');
  let places = $state<Place[]>([]);
  let searching = $state(false);
  async function search(e: SubmitEvent) {
    e.preventDefault();
    searching = true;
    try {
      places = await api.geocode(query);
      if (!places.length) ui.toast('No places found', 'info');
    } catch (err) {
      ui.toast((err as Error).message, 'critical');
    } finally {
      searching = false;
    }
  }
  function go(p: Place) {
    ws.map?.flyTo({ lat: p.lat, lon: p.lon }, 15);
    places = [];
  }
  function addAt(p: Place) {
    go(p);
    ws.select(null);
    ws.placing = { lat: p.lat, lon: p.lon };
    ws.tab = 'assets';
  }

  const SHAPES: [Shape, string, string][] = [
    ['point', 'Point', 'map-pin'],
    ['line', 'Line', 'spline'],
    ['freehand', 'Freehand', 'pencil'],
    ['polygon', 'Polygon', 'pentagon'],
    ['rectangle', 'Rectangle', 'square'],
    ['circle', 'Circle', 'circle'],
  ];
  const isMode = (k: string) => ws.mode.kind === k;
  const toggleMode = (k: 'add' | 'measure') => ws.setMode(ws.mode.kind === k ? { kind: 'select' } : { kind: k });

  async function clearMarkup() {
    const ids = Object.keys(ws.exercise?.annotations ?? {});
    if (!ids.length) return;
    if (await ui.confirm(`Delete all ${ids.length} drawings?`, { confirm: 'Delete all', danger: true })) {
      await ws.send({ type: 'remove_annotations', ids });
    }
  }
</script>

<svelte:window
  onclick={(e) => { if (open && !(e.target as HTMLElement).closest('.toolbar')) open = null; }}
  onkeydown={(e) => { if (e.key === 'Escape') { open = null; places = []; } }}
/>

<div class="toolbar">
  <form class="search" onsubmit={search}>
    <input type="search" bind:value={query} placeholder="Search for a place…" aria-label="Search for a place" maxlength="300" />
    <button disabled={!query.trim() || searching} aria-label="Search"><Icon name="search" size={14} /></button>
    {#if places.length}
      <ul class="results">
        {#each places as p, i (i)}
          <li>
            <button type="button" class="ghost grow" onclick={() => go(p)}>{p.name}<span class="muted"> · {p.kind}</span></button>
            <button type="button" class="small" onclick={() => addAt(p)} title="Add an asset here">+ Asset</button>
          </li>
        {/each}
      </ul>
    {/if}
  </form>

  <div class="tools" role="toolbar" aria-label="Map tools">
    <button class:on={isMode('add')} onclick={() => toggleMode('add')} title="Add asset (A)" aria-pressed={isMode('add')}><Icon name="map-pin-plus" /></button>
    <button class:on={isMode('measure')} onclick={() => toggleMode('measure')} title="Measure (M)" aria-pressed={isMode('measure')}><Icon name="ruler" /></button>
    <div class="drop">
      <button class:on={isMode('draw')} onclick={() => (open = open === 'draw' ? null : 'draw')} title="Draw on the map" aria-expanded={open === 'draw'}><Icon name="pen-tool" /></button>
      {#if open === 'draw'}
        <div class="pop">
          {#each SHAPES as [shape, label, icon] (shape)}
            <button class="item" class:on={ws.mode.kind === 'draw' && ws.mode.shape === shape} onclick={() => { ws.setMode({ kind: 'draw', shape }); open = null; }}>
              <Icon name={icon} size={14} /> {label}
            </button>
          {/each}
          <label class="item colour">Colour <input type="color" bind:value={ws.drawColor} /></label>
          <button class="item danger" onclick={clearMarkup}><Icon name="eraser" size={14} /> Clear all drawings</button>
        </div>
      {/if}
    </div>
    <button class:on={ws.locked} onclick={() => (ws.locked = !ws.locked)} title={ws.locked ? 'Unlock map' : 'Lock map (no pan or zoom)'} aria-pressed={ws.locked}>
      <Icon name={ws.locked ? 'lock' : 'lock-open'} />
    </button>
    <div class="drop">
      <button onclick={() => (open = open === 'layers' ? null : 'layers')} title="Layers" aria-expanded={open === 'layers'}><Icon name="layers" /></button>
      {#if open === 'layers'}
        <div class="pop">
          <label class="item"><input type="checkbox" bind:checked={ws.layers.clusters} /> Cluster markers</label>
          <label class="item"><input type="checkbox" bind:checked={ws.layers.zones} /> Sector zones</label>
          <label class="item"><input type="checkbox" bind:checked={ws.layers.links} /> Links</label>
          <label class="item"><input type="checkbox" bind:checked={ws.layers.annotations} /> Drawings</label>
          <label class="item"><input type="checkbox" bind:checked={ws.layers.labels} /> Asset names</label>
          <label class="item"><input type="checkbox" bind:checked={ws.layers.legend} /> Legend</label>
          <p class="section-title">Heat map</p>
          <label class="item"><input type="radio" bind:group={ws.layers.heat} value="off" /> Off</label>
          {#each ws.exercise?.taxonomy.status_sets.filter((s) => s.display !== 'detail') ?? [] as s (s.id)}
            <label class="item"><input type="radio" bind:group={ws.layers.heat} value={s.display} /> {s.name}</label>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .toolbar { position: absolute; top: 10px; left: 10px; z-index: 11; display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
  .search { position: relative; display: flex; gap: 4px; width: min(340px, calc(100vw - 90px)); }
  .search input { background: rgba(5, 10, 6, 0.92); }
  .results {
    position: absolute; top: calc(100% + 4px); left: 0; right: 0; list-style: none; margin: 0; padding: 4px; z-index: 2;
    background: var(--panel-2); border: 1px solid var(--line); border-radius: var(--radius); display: grid; gap: 2px; max-height: 50vh; overflow: auto;
  }
  .results li { display: flex; gap: 4px; align-items: center; }
  .results .ghost { text-align: left; font-size: 12px; }
  .tools { display: flex; flex-direction: column; gap: 4px; }
  .tools > button, .drop > button { width: 36px; height: 36px; padding: 0; display: grid; place-items: center; background: rgba(11, 19, 13, 0.92); }
  .on { background: var(--accent) !important; color: var(--accent-ink); border-color: var(--accent); }
  .drop { position: relative; }
  .pop {
    position: absolute; left: 42px; top: 0; display: grid; gap: 2px; min-width: 190px; padding: 6px;
    background: var(--panel-2); border: 1px solid var(--line); border-radius: var(--radius);
  }
  .item { display: flex; flex-direction: row; gap: 8px; align-items: center; border: 0; background: none; padding: 5px 6px; text-align: left; font-size: 13px; color: var(--text); }
  .item:hover { background: var(--panel); }
  .item.on { color: var(--accent-ink); }
  .colour { justify-content: space-between; }
  .section-title { margin: 6px 0 0; font-size: 10px; }
  .small { font-size: 11px; padding: 3px 6px; white-space: nowrap; }
</style>
