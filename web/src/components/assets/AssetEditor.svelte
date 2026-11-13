<script lang="ts">
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Asset } from '../../lib/types/Asset';
  import type { AssetPatch } from '../../lib/types/AssetPatch';
  import type { LinkKind } from '../../lib/types/LinkKind';
  import Icon from '../ui/Icon.svelte';
  import VisibilityPicker from '../ui/VisibilityPicker.svelte';

  let { asset }: { asset: Asset } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const type = $derived(ex.taxonomy.asset_types.find((t) => t.id === asset.type_id));
  const links = $derived(Object.values(ex.links).filter((l) => l.from === asset.id || l.to === asset.id));
  const others = $derived(Object.values(ex.assets).filter((a) => a.id !== asset.id).sort((a, b) => a.name.localeCompare(b.name)));
  const KIND: Record<LinkKind, string> = { depends_on: 'depends on', connects_to: 'connects to', managed_by: 'is managed by' };

  /** A link read from this asset's side, e.g. "Depends on Rosslyn Substation". */
  function describe(l: { from: string; to: string; kind: LinkKind }): string {
    if (l.kind === 'connects_to') return `Connects to ${nameOf(l.from === asset.id ? l.to : l.from)}`;
    // depends_on and managed_by run from the provider to the dependant.
    return l.to === asset.id
      ? `${KIND[l.kind][0]!.toUpperCase()}${KIND[l.kind].slice(1)} ${nameOf(l.from)}`
      : `${nameOf(l.to)} ${KIND[l.kind]} this`;
  }

  const edit = (patch: AssetPatch) => ws.send({ type: 'edit_asset', id: asset.id, patch });

  async function rename(e: Event) {
    const name = (e.currentTarget as HTMLInputElement).value.trim();
    if (name && name !== asset.name) await edit({ name });
  }

  let linkTo = $state('');
  let linkKind = $state<LinkKind>('depends_on');
  let linkLabel = $state('');
  async function addLink() {
    if (!linkTo) return;
    // "This depends on / is managed by X" runs X → this; "connects to" this → X.
    const [from, to] = linkKind === 'connects_to' ? [asset.id, linkTo] : [linkTo, asset.id];
    if (await ws.send({ type: 'add_link', from, to, kind: linkKind, label: linkLabel })) {
      linkTo = '';
      linkLabel = '';
    }
  }

  async function remove() {
    const ok = await ui.confirm(`Remove ${asset.name}?`, { body: 'Its links go with it. Undo brings both back.', confirm: 'Remove', danger: true });
    if (ok && (await ws.send({ type: 'remove_assets', ids: [asset.id] }))) ws.select(null);
  }

  async function place() {
    const p = await ws.pickLocation(`Click the map to place ${asset.name}`);
    if (p) await edit({ geo: p });
  }

  const nameOf = (id: string) => ex.assets[id]?.name ?? '(removed)';
  const when = (iso: string) => new Date(iso).toLocaleTimeString();
  const label = (set: string, value: string) => {
    const s = ex.taxonomy.status_sets.find((x) => x.id === set);
    return `${s?.name ?? set}: ${s?.values.find((v) => v.id === value)?.label ?? value}`;
  };
</script>

<div class="editor">
  <div class="row">
    <button class="ghost small" onclick={() => ws.select(null)} aria-label="Back to the asset list"><Icon name="arrow-left" size={14} /></button>
    <span class="muted grow"><Icon name={type?.icon ?? 'circle'} size={14} /> {type?.name}</span>
    {#if asset.geo}<button class="ghost small" onclick={() => asset.geo && ws.map?.flyTo(asset.geo, 15)} title="Zoom to"><Icon name="crosshair" size={14} /></button>{/if}
  </div>

  <label>Name <input value={asset.name} onchange={rename} maxlength="200" /></label>

  {#each ex.taxonomy.status_sets as set (set.id)}
    <fieldset>
      <legend>{set.name}</legend>
      <div class="statuses">
        {#each set.values as v (v.id)}
          <button
            class="status"
            class:on={asset.statuses[set.id] === v.id}
            style:--c={v.color}
            aria-pressed={asset.statuses[set.id] === v.id}
            onclick={() => ws.send({ type: 'set_status', ids: [asset.id], set: set.id, value: v.id })}
          >{v.label}</button>
        {/each}
      </div>
    </fieldset>
  {/each}

  <div class="row">
    <label class="grow">Type
      <select value={asset.type_id} onchange={(e) => edit({ type_id: e.currentTarget.value })}>
        {#each ex.taxonomy.asset_types as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
      </select>
    </label>
    <label>Colour
      <input type="color" value={asset.color ?? type?.color ?? '#888888'} onchange={(e) => edit({ color: e.currentTarget.value })} />
    </label>
    {#if asset.color}<button class="ghost small reset" onclick={() => edit({ color: null })} title="Use the status colour">Reset</button>{/if}
  </div>

  <label>Notes <textarea rows="3" value={asset.notes} maxlength="4000" onchange={(e) => edit({ notes: e.currentTarget.value })}></textarea></label>
  <label>Tags <input value={asset.tags.join(', ')} placeholder="comma, separated" onchange={(e) => edit({ tags: e.currentTarget.value.split(',') })} /></label>

  <VisibilityPicker label="Visible to" value={asset.visibility} teams={ex.teams} onchange={(visibility) => edit({ visibility })} />

  <div class="row">
    <button class="small" onclick={place}>{asset.geo ? 'Move on map…' : 'Place on map…'}</button>
    {#if asset.geo}<span class="muted mono small">{asset.geo.lat.toFixed(4)}, {asset.geo.lon.toFixed(4)}</span>{/if}
  </div>

  <section>
    <h3 class="section-title">Links</h3>
    {#if links.length}
      <ul class="links">
        {#each links as l (l.id)}
          <li>
            <span class="grow">{describe(l)}{#if l.label}<span class="muted"> · {l.label}</span>{/if}</span>
            <button class="ghost small" aria-label="Remove link" onclick={() => ws.send({ type: 'remove_link', id: l.id })}>✕</button>
          </li>
        {/each}
      </ul>
    {/if}
    <div class="row wrap">
      <select bind:value={linkKind} aria-label="Link kind" class="kind">
        <option value="depends_on">Depends on</option>
        <option value="connects_to">Connects to</option>
        <option value="managed_by">Is managed by</option>
      </select>
      <select bind:value={linkTo} aria-label="Linked asset" class="grow">
        <option value="">Choose an asset…</option>
        {#each others as o (o.id)}<option value={o.id}>{o.name}</option>{/each}
      </select>
    </div>
    <div class="row">
      <input class="grow" bind:value={linkLabel} placeholder="Label (optional)" maxlength="200" aria-label="Link label" />
      <button onclick={addLink} disabled={!linkTo}>Link</button>
    </div>
  </section>

  <details>
    <summary class="section-title">History ({asset.history.length})</summary>
    <ol class="history">
      {#each [...asset.history].reverse() as h, i (i)}
        <li><span class="mono muted">{when(h.at)}</span> {label(h.set, h.value)}</li>
      {/each}
    </ol>
  </details>

  <button class="danger" onclick={remove}><Icon name="trash-2" size={14} /> Remove asset</button>
</div>

<style>
  .editor { display: grid; gap: 12px; }
  fieldset { border: 0; padding: 0; margin: 0; display: grid; gap: 6px; }
  legend { font-size: 12px; color: var(--muted); padding: 0; margin-bottom: 4px; }
  .statuses { display: flex; flex-wrap: wrap; gap: 4px; }
  .status { font-size: 12px; padding: 4px 8px; border-left: 4px solid var(--c); }
  .status.on { background: var(--c); color: #000; font-weight: 600; border-color: var(--c); }
  .reset { align-self: end; }
  .small { font-size: 12px; }
  .links { list-style: none; padding: 0; margin: 0 0 8px; display: grid; gap: 4px; font-size: 13px; }
  .links li { display: flex; gap: 6px; align-items: center; }
  section { display: grid; gap: 6px; }
  .wrap { flex-wrap: wrap; }
  .kind { width: auto; }
  .history { padding-left: 18px; font-size: 12px; display: grid; gap: 2px; }
  summary { cursor: pointer; }
</style>
