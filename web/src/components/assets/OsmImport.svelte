<script lang="ts">
  // Search OpenStreetMap for real infrastructure near a point and add the
  // results as assets in one step.
  import { api } from '../../lib/api';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { OsmResult } from '../../lib/types/OsmResult';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  let open = $state(false);
  let categories = $state<string[]>([]);
  let category = $state('Hospitals');
  let radius = $state(5);
  let results = $state<OsmResult[] | null>(null);
  let chosen = $state<boolean[]>([]);
  let typeId = $state('');
  let busy = $state(false);

  $effect(() => {
    if (ws.osmAt) open = true;
  });
  $effect(() => {
    if (open && !categories.length) api.osmCategories().then((c) => (categories = c)).catch(() => {});
  });
  // Default the asset type to the one named like the OSM category.
  $effect(() => {
    const match = ex.taxonomy.asset_types.find((t) => t.name.toLowerCase() === category.toLowerCase());
    typeId = (match ?? ex.taxonomy.asset_types.find((t) => t.id === 'asset') ?? ex.taxonomy.asset_types[0])?.id ?? '';
  });

  const center = $derived(ws.osmAt ?? ws.map?.center() ?? null);

  async function search() {
    const c = ws.osmAt ?? ws.map?.center();
    if (!c) return;
    busy = true;
    results = null;
    try {
      results = await api.osmSearch(c.lat, c.lon, radius, category);
      chosen = results.map(() => true);
      if (!results.length) ui.toast(`No ${category.toLowerCase()} within ${radius} mi`, 'info');
    } catch (e) {
      ui.toast((e as Error).message, 'critical', 6000);
    } finally {
      busy = false;
    }
  }

  async function add() {
    const items = (results ?? []).filter((_, i) => chosen[i]);
    if (!items.length) return;
    const r = await ws.send({
      type: 'add_assets',
      assets: items.map((it) => ({ name: it.name, type_id: typeId, geo: { lat: it.lat, lon: it.lon } })),
    });
    if (r) {
      ui.toast(`Added ${items.length} assets`, 'ok');
      results = null;
      ws.osmAt = null;
      ws.map?.fit(items.map((it) => ({ lat: it.lat, lon: it.lon })));
    }
  }
</script>

<details bind:open>
  <summary class="section-title">Import from OpenStreetMap</summary>
  <div class="osm">
    <p class="muted small">
      {#if ws.osmAt}Around the point you chose.{:else}Around the centre of the map. Right-click the map to pick a point.{/if}
      {#if center}<span class="mono">{center.lat.toFixed(3)}, {center.lon.toFixed(3)}</span>{/if}
    </p>
    <div class="row">
      <label class="grow">Find
        <select bind:value={category}>
          {#each categories as c (c)}<option>{c}</option>{/each}
        </select>
      </label>
      <label class="radius">Within (mi)<input type="number" min="0.5" max="100" step="0.5" bind:value={radius} /></label>
    </div>
    <button onclick={search} disabled={busy}>{busy ? 'Searching…' : 'Search'}</button>

    {#if results?.length}
      <label>Add as
        <select bind:value={typeId}>
          {#each ex.taxonomy.asset_types as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
        </select>
      </label>
      <div class="row">
        <span class="muted small grow">{chosen.filter(Boolean).length} of {results.length} selected</span>
        <button class="ghost small" onclick={() => (chosen = chosen.map(() => true))}>All</button>
        <button class="ghost small" onclick={() => (chosen = chosen.map(() => false))}>None</button>
      </div>
      <ul>
        {#each results as r, i (i)}
          <li><label class="check"><input type="checkbox" bind:checked={chosen[i]} /> {r.name}</label></li>
        {/each}
      </ul>
      <button class="primary" onclick={add} disabled={!chosen.some(Boolean)}>Add {chosen.filter(Boolean).length} assets</button>
    {/if}
  </div>
</details>

<style>
  summary { cursor: pointer; }
  .osm { display: grid; gap: 8px; margin-top: 8px; }
  .small { font-size: 12px; margin: 0; }
  .radius { width: 100px; }
  ul { list-style: none; padding: 0; margin: 0; max-height: 260px; overflow: auto; display: grid; gap: 2px; }
  .check { display: flex; flex-direction: row; gap: 8px; align-items: center; color: var(--text); font-size: 13px; }
</style>
