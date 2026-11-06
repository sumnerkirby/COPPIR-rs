<script lang="ts">
  import type { Applied } from '../lib/types/Applied';
  import type { Command } from '../lib/types/Command';
  import type { Exercise } from '../lib/types/Exercise';
  import type { GeoPoint } from '../lib/types/GeoPoint';

  let {
    exercise,
    selected,
    pending,
    send,
    ondone,
    onselect,
  }: {
    exercise: Exercise;
    selected: string | null;
    pending: GeoPoint | null;
    send: (c: Command) => Promise<Applied | null>;
    ondone: () => void;
    onselect: (id: string | null) => void;
  } = $props();

  const asset = $derived(selected ? exercise.assets[selected] : undefined);
  const types = $derived(exercise.taxonomy.asset_types);

  // New-asset form
  let name = $state('');
  let typeId = $state('');
  // Start on the generic "asset" type when the taxonomy has one, so a quick
  // add does not silently count toward a sector. The choice then sticks
  // between adds, since this panel stays mounted.
  $effect(() => {
    if (!typeId || !types.some((t) => t.id === typeId)) {
      typeId = (types.find((t) => t.id === 'asset') ?? types[0])?.id ?? '';
    }
  });

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (!pending) return;
    const r = await send({ type: 'add_assets', assets: [{ name, type_id: typeId, geo: pending }] });
    const ev = r?.events[0]?.event;
    if (ev?.type === 'assets_upserted' && ev.assets[0]) {
      name = '';
      ondone();
      onselect(ev.assets[0].id);
    }
  }

  async function setStatus(set: string, value: string) {
    if (asset) await send({ type: 'set_status', ids: [asset.id], set, value });
  }

  async function saveNotes(e: Event) {
    const notes = (e.currentTarget as HTMLTextAreaElement).value;
    if (asset && notes !== asset.notes) await send({ type: 'edit_asset', id: asset.id, patch: { notes } });
  }

  async function remove() {
    if (asset && confirm(`Remove ${asset.name}? Its links go with it.`)) {
      await send({ type: 'remove_assets', ids: [asset.id] });
      onselect(null);
    }
  }

  const typeName = (id: string) => types.find((t) => t.id === id)?.name ?? id;
</script>

{#if pending}
  <form class="card" onsubmit={add}>
    <h2>New asset</h2>
    <p class="muted mono coords">{pending.lat.toFixed(5)}, {pending.lon.toFixed(5)}</p>
    <label>Name <input bind:value={name} required maxlength="200" /></label>
    <label>
      Type
      <select bind:value={typeId}>
        {#each types as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
      </select>
    </label>
    <div class="row">
      <button type="button" onclick={ondone}>Cancel</button>
      <button class="primary" disabled={!name.trim()}>Add</button>
    </div>
  </form>
{:else if asset}
  <section class="card">
    <h2>{asset.name}</h2>
    <p class="muted">{typeName(asset.type_id)}</p>
    {#each exercise.taxonomy.status_sets as set (set.id)}
      <label>
        {set.name}
        <select value={asset.statuses[set.id]} onchange={(e) => setStatus(set.id, e.currentTarget.value)}>
          {#each set.values as v (v.id)}<option value={v.id}>{v.label}</option>{/each}
        </select>
      </label>
    {/each}
    <label>Notes <textarea rows="3" value={asset.notes} onchange={saveNotes} maxlength="4000"></textarea></label>
    <details>
      <summary class="muted">History ({asset.history.length})</summary>
      <ol>
        {#each [...asset.history].reverse() as h, i (i)}
          <li><span class="mono muted">{new Date(h.at).toLocaleTimeString()}</span> {h.set}: {h.value}</li>
        {/each}
      </ol>
    </details>
    <div class="row"><button class="danger" onclick={remove}>Remove</button></div>
  </section>
{:else}
  <p class="muted hint">Select an asset on the map, or press <kbd>A</kbd> and click the map to add one.</p>
{/if}

<style>
  .card { display: grid; gap: 10px; }
  h2 { font-size: 16px; }
  .card > p { margin: -6px 0 0; }
  .coords { font-size: 12px; }
  .row { display: flex; gap: 8px; justify-content: flex-end; }
  .hint { margin: 0; }
  kbd { font-family: var(--mono); border: 1px solid var(--line); border-radius: 3px; padding: 0 4px; }
  ol { margin: 6px 0 0; padding-left: 18px; font-size: 12px; display: grid; gap: 2px; }
  summary { cursor: pointer; font-size: 12px; }
</style>
