<script lang="ts">
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { GeoPoint } from '../../lib/types/GeoPoint';

  let { at, typeHint }: { at: GeoPoint; typeHint?: string } = $props();
  const ws = useWorkspace();
  const types = $derived(ws.exercise!.taxonomy.asset_types);

  let name = $state('');
  let typeId = $state('');
  $effect(() => {
    if (!typeId || !types.some((t) => t.id === typeId)) {
      typeId = (typeHint && types.some((t) => t.id === typeHint) ? typeHint : (types.find((t) => t.id === 'asset') ?? types[0])?.id) ?? '';
    }
  });

  async function add(e: SubmitEvent) {
    e.preventDefault();
    const r = await ws.send({ type: 'add_assets', assets: [{ name, type_id: typeId, geo: at }] });
    const ev = r?.events[0]?.event;
    if (ev?.type === 'assets_upserted' && ev.assets[0]) {
      name = '';
      if (ws.mode.kind === 'add') ws.setMode({ kind: 'select' });
      ws.select(ev.assets[0].id);
    }
  }
</script>

<form onsubmit={add}>
  <h2>New asset</h2>
  <p class="muted mono coords">{at.lat.toFixed(5)}, {at.lon.toFixed(5)}</p>
  <!-- svelte-ignore a11y_autofocus -->
  <label>Name <input bind:value={name} required maxlength="200" autofocus /></label>
  <label>Type
    <select bind:value={typeId}>
      {#each types as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
    </select>
  </label>
  <div class="row end">
    <button type="button" onclick={() => (ws.placing = null)}>Cancel</button>
    <button class="primary" disabled={!name.trim()}>Add</button>
  </div>
</form>

<style>
  form { display: grid; gap: 10px; }
  h2 { font-size: 16px; }
  .coords { font-size: 12px; margin: -6px 0 0; }
  .end { justify-content: flex-end; }
</style>
