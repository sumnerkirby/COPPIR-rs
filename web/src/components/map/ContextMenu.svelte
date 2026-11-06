<script lang="ts">
  import { useWorkspace } from '../../lib/workspace.svelte';
  import { ui } from '../../lib/ui.svelte';
  import type { GeoPoint } from '../../lib/types/GeoPoint';
  import Icon from '../ui/Icon.svelte';

  let { x, y, at, onclose, onmeasure }: { x: number; y: number; at: GeoPoint; onclose: () => void; onmeasure: (p: GeoPoint) => void } =
    $props();
  const ws = useWorkspace();

  function act(f: () => void) {
    f();
    onclose();
  }
  const coords = $derived(`${at.lat.toFixed(5)}, ${at.lon.toFixed(5)}`);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="menu" role="menu" style:left="{x}px" style:top="{y}px">
  <button role="menuitem" onclick={() => act(() => { ws.select(null); ws.placing = at; ws.tab = 'assets'; })}><Icon name="map-pin-plus" size={14} /> Add asset here</button>
  <button role="menuitem" onclick={() => act(() => { ws.select(null); ws.osmAt = at; ws.tab = 'assets'; })}><Icon name="search" size={14} /> Find infrastructure near here</button>
  <button role="menuitem" onclick={() => act(() => onmeasure(at))}><Icon name="ruler" size={14} /> Measure from here</button>
  <button role="menuitem" onclick={() => act(() => navigator.clipboard?.writeText(coords).then(() => ui.toast(`Copied ${coords}`, 'ok', 2000)))}>
    <Icon name="copy" size={14} /> {coords}
  </button>
</div>

<style>
  .menu {
    position: absolute; z-index: 25; display: grid; min-width: 230px; padding: 4px;
    background: var(--panel-2); border: 1px solid var(--line); border-radius: var(--radius); box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
  }
  button { display: flex; gap: 8px; align-items: center; border: 0; background: none; text-align: left; padding: 7px 10px; font-size: 13px; }
  button:hover, button:focus-visible { background: var(--panel); }
</style>
