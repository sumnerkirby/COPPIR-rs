<script lang="ts">
  // Offline map packs: save the area on screen so the map works with no
  // internet, and pick which map the exercise uses. Players get the pack
  // from this laptop over the room's network.
  import { onDestroy, untrack } from 'svelte';
  import { api } from '../../lib/api';
  import { estimateBytes, formatBytes, MAX_TILES, MAX_ZOOM, tileCount, type Bounds } from '../../lib/maps';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { MapPack } from '../../lib/types/MapPack';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  let packs = $state<MapPack[]>([]);
  let area = $state<Bounds | null>(null);
  let name = $state(untrack(() => ws.exercise?.name ?? ''));
  let zoom = $state(MAX_ZOOM);
  let busy = $state(false);

  let timer: ReturnType<typeof setTimeout> | undefined;
  async function refresh() {
    clearTimeout(timer);
    try {
      packs = await api.mapPacks();
    } catch {
      return;
    }
    // Keep checking while anything downloads.
    if (packs.some((p) => p.progress)) timer = setTimeout(refresh, 1000);
  }
  refresh();
  onDestroy(() => clearTimeout(timer));

  const tiles = $derived(area ? tileCount(area, zoom) : 0);
  const ready = $derived(packs.filter((p) => !p.progress && !p.error));

  function useView() {
    area = ws.map?.bounds() ?? null;
    if (!area) ui.toast('Switch to the map view first', 'warning');
  }

  async function download() {
    if (!area) return;
    busy = true;
    try {
      await api.downloadMapPack({ name: name.trim() || ex.name, bounds: area, max_zoom: zoom });
      area = null;
      await refresh();
    } catch (e) {
      ui.toast((e as Error).message, 'critical', 6000);
    } finally {
      busy = false;
    }
  }

  async function remove(p: MapPack) {
    if (!p.progress && !p.error) {
      const ok = await ui.confirm(`Delete the “${p.name}” map pack?`, {
        body: 'Exercises that use it go back to the online map.',
        confirm: 'Delete',
        danger: true,
      });
      if (!ok) return;
    }
    try {
      await api.removeMapPack(p.id);
      if (ex.branding.map_pack === p.id) await choose('');
    } catch (e) {
      ui.toast((e as Error).message, 'critical');
    }
    await refresh();
  }

  async function choose(id: string) {
    await ws.send({ type: 'set_branding', branding: { ...$state.snapshot(ex.branding), map_pack: id || null } });
  }

  const pct = (p: MapPack) => (p.progress ? Math.floor((p.progress.done / Math.max(1, p.progress.total)) * 100) : 100);
</script>

<section>
  <h3 class="section-title">Offline map</h3>
  <label>Base map
    <select value={ex.branding.map_pack ?? ''} onchange={(e) => choose(e.currentTarget.value)}>
      <option value="">Online (OpenFreeMap)</option>
      {#each ready as p (p.id)}<option value={p.id}>Offline: {p.name}</option>{/each}
      {#if ex.branding.map_pack && !ready.some((p) => p.id === ex.branding.map_pack)}
        <option value={ex.branding.map_pack} disabled>Missing pack</option>
      {/if}
    </select>
  </label>

  {#each packs as p (p.id)}
    <div class="pack" class:failed={!!p.error}>
      <div class="row">
        <strong class="grow">{p.name}</strong>
        <span class="muted small">{p.progress ? `${pct(p)}%` : p.error ? 'failed' : `${formatBytes(p.bytes)} · zoom ${p.max_zoom}`}</span>
        <button class="ghost small" onclick={() => remove(p)} aria-label={p.progress ? `Stop downloading ${p.name}` : `Delete ${p.name}`}>
          {p.progress ? 'Stop' : '✕'}
        </button>
      </div>
      {#if p.progress}<progress max="100" value={pct(p)}></progress>{/if}
      {#if p.error}<p class="error">{p.error}</p>{/if}
    </div>
  {/each}

  {#if area}
    <div class="new">
      <label>Name <input bind:value={name} maxlength="100" /></label>
      <label>Detail
        <select bind:value={zoom}>
          <option value={10}>Region (zoom 10)</option>
          <option value={12}>City (zoom 12)</option>
          <option value={13}>Neighbourhood (zoom 13)</option>
          <option value={14}>Streets and buildings (zoom 14)</option>
        </select>
      </label>
      <p class="muted small" class:error={tiles > MAX_TILES}>
        {tiles.toLocaleString()} tiles, about {formatBytes(estimateBytes(tiles))}.
        {#if tiles > MAX_TILES}Too large: zoom in, or pick less detail.{/if}
      </p>
      <div class="row">
        <button onclick={() => (area = null)}>Cancel</button>
        <button class="primary grow" onclick={download} disabled={busy || tiles > MAX_TILES || !name.trim()}>Download</button>
      </div>
    </div>
  {:else}
    <button onclick={useView}>Save the area on screen for offline use…</button>
    <p class="empty">For venues with poor Wi-Fi. Download before the day, while online; players then load the map from this laptop.</p>
  {/if}
</section>

<style>
  section { display: grid; gap: 8px; border-top: 1px solid var(--line); padding-top: 14px; }
  .pack { display: grid; gap: 4px; font-size: 13px; }
  .pack.failed strong { color: var(--danger); }
  .small { font-size: 12px; }
  progress { width: 100%; height: 6px; }
  .error { color: var(--danger); margin: 0; font-size: 12px; }
  .new { display: grid; gap: 6px; border: 1px dashed var(--line); border-radius: var(--radius); padding: 8px; }
  .new p { margin: 0; }
</style>
