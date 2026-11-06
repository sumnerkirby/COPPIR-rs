<script lang="ts">
  import { statusValue } from '../../lib/sitrep';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import Icon from '../ui/Icon.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const types = $derived(ex.taxonomy.asset_types);
  const fill = $derived(ex.taxonomy.status_sets.find((s) => s.display === 'fill') ?? ex.taxonomy.status_sets[0]);
  const shown = $derived(ws.visible.slice(0, 300));
  const typeCounts = $derived(
    Object.values(ex.assets).reduce<Record<string, number>>((m, a) => ((m[a.type_id] = (m[a.type_id] ?? 0) + 1), m), {}),
  );

  // Bulk status
  let bulkType = $state('');
  let bulkSet = $state('');
  let bulkValue = $state('');
  $effect(() => {
    if (!bulkSet && ex.taxonomy.status_sets[0]) bulkSet = ex.taxonomy.status_sets[0].id;
  });
  const bulkValues = $derived(ex.taxonomy.status_sets.find((s) => s.id === bulkSet)?.values ?? []);

  function zoomToMatches(e: KeyboardEvent) {
    if (e.key !== 'Enter') return;
    const pts = ws.visible.flatMap((a) => (a.geo ? [a.geo] : []));
    if (pts.length) ws.map?.fit(pts);
  }

  function toggleType(id: string) {
    ws.hiddenTypes = ws.hiddenTypes.includes(id) ? ws.hiddenTypes.filter((t) => t !== id) : [...ws.hiddenTypes, id];
  }

  async function applyBulk() {
    const ids = Object.values(ex.assets).filter((a) => !bulkType || a.type_id === bulkType).map((a) => a.id);
    const set = ex.taxonomy.status_sets.find((s) => s.id === bulkSet);
    const value = set?.values.find((v) => v.id === bulkValue);
    if (!ids.length || !set || !value) return;
    const scope = bulkType ? `every ${types.find((t) => t.id === bulkType)?.name ?? ''} asset` : 'every asset';
    const ok = await ui.confirm(`Set ${set.name} to ${value.label}?`, { body: `This changes ${scope} (${ids.length}).`, confirm: 'Apply' });
    if (ok && (await ws.send({ type: 'set_status', ids, set: set.id, value: value.id }))) ui.toast(`Updated ${ids.length} assets`, 'ok');
  }

  const typeOf = (id: string) => types.find((t) => t.id === id);
</script>

<section class="find">
  <input
    id="asset-search"
    type="search"
    placeholder="Find assets by name or tag…  (Enter to zoom)"
    aria-label="Find assets"
    bind:value={ws.query}
    onkeydown={zoomToMatches}
  />
  <p class="muted count">{ws.visible.length} of {Object.keys(ex.assets).length} shown</p>
</section>

<details class="types">
  <summary class="section-title">Asset types</summary>
  <div class="chips">
    {#each types as t (t.id)}
      {#if typeCounts[t.id]}
        <button class="chip" class:off={ws.hiddenTypes.includes(t.id)} onclick={() => toggleType(t.id)} aria-pressed={!ws.hiddenTypes.includes(t.id)}>
          <Icon name={t.icon} size={14} /> {t.name} <span class="muted">{typeCounts[t.id]}</span>
        </button>
      {/if}
    {/each}
  </div>
</details>

{#if Object.keys(ex.assets).length === 0}
  <p class="empty">No assets yet. Press <kbd>A</kbd> and click the map, right-click the map, or import from OpenStreetMap below.</p>
{:else}
  <ul class="list">
    {#each shown as a (a.id)}
      {@const v = fill ? statusValue(fill, a) : undefined}
      <li>
        <button onclick={() => { ws.select(a.id); if (a.geo) ws.map?.flyTo(a.geo); }}>
          <span class="dot" style:background={v?.color ?? '#888'} title={v?.label}>{v?.glyph ?? ''}</span>
          <Icon name={typeOf(a.type_id)?.icon ?? 'circle'} size={14} />
          <span class="grow name">{a.name}</span>
          {#if !a.geo}<span class="muted small" title="Not on the map">off-map</span>{/if}
        </button>
      </li>
    {/each}
  </ul>
  {#if ws.visible.length > shown.length}<p class="empty">Showing the first {shown.length}; refine the search to see more.</p>{/if}
{/if}

<details>
  <summary class="section-title">Bulk status change</summary>
  <div class="bulk">
    <label>Assets
      <select bind:value={bulkType}>
        <option value="">All types</option>
        {#each types as t (t.id)}{#if typeCounts[t.id]}<option value={t.id}>{t.name}</option>{/if}{/each}
      </select>
    </label>
    <div class="row">
      <label class="grow">Status
        <select bind:value={bulkSet} onchange={() => (bulkValue = '')}>
          {#each ex.taxonomy.status_sets as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
        </select>
      </label>
      <label class="grow">Set to
        <select bind:value={bulkValue}>
          <option value="" disabled>Choose…</option>
          {#each bulkValues as v (v.id)}<option value={v.id}>{v.label}</option>{/each}
        </select>
      </label>
    </div>
    <button onclick={applyBulk} disabled={!bulkValue}>Apply</button>
  </div>
</details>

<style>
  .find { display: grid; gap: 4px; }
  .count { font-size: 12px; margin: 0; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
  .chip { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; padding: 3px 8px; border-radius: 999px; }
  .chip.off { opacity: 0.45; text-decoration: line-through; }
  summary { cursor: pointer; }
  .list { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .list button { width: 100%; display: flex; align-items: center; gap: 8px; border: 0; background: none; padding: 5px 6px; text-align: left; }
  .list button:hover { background: var(--panel-2); }
  .dot { width: 18px; height: 18px; border-radius: 50%; display: inline-grid; place-items: center; font-size: 10px; color: #000; font-weight: 700; flex: none; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .small { font-size: 11px; }
  .bulk { display: grid; gap: 8px; margin-top: 8px; }
</style>
