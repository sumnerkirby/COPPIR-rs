<script lang="ts">
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Effect } from '../../lib/types/Effect';
  import type { Severity } from '../../lib/types/Severity';
  import type { Visibility } from '../../lib/types/Visibility';
  import VisibilityPicker from '../ui/VisibilityPicker.svelte';

  let { ondone }: { ondone: () => void } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const sets = $derived(ex.taxonomy.status_sets);
  const assets = $derived(Object.values(ex.assets).sort((a, b) => a.name.localeCompare(b.name)));

  let title = $state('');
  let body = $state('');
  let notes = $state('');
  let severity = $state<Severity>('warning');
  let effects = $state<Effect[]>([]);
  let audience = $state<Visibility>({ mode: 'all' });

  function addStatus() {
    const set = sets[0];
    effects.push({ type: 'set_status', asset: assets[0]?.id ?? '', set: set?.id ?? '', value: set?.values.at(-1)?.id ?? '' });
  }

  async function addAsset() {
    const at = await ws.pickLocation('Click the map where the inject should create its asset');
    if (!at) return;
    const type = ex.taxonomy.asset_types.find((t) => t.id === 'asset') ?? ex.taxonomy.asset_types[0];
    effects.push({ type: 'create_asset', asset: { name: '', type_id: type?.id ?? '', geo: at } });
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    const r = await ws.send({ type: 'add_inject', inject: { title, body, severity, audience, effects, notes } });
    if (r) ondone();
  }
</script>

<form onsubmit={save}>
  <h2>New inject</h2>
  <!-- svelte-ignore a11y_autofocus -->
  <label>Title <input bind:value={title} required maxlength="200" autofocus /></label>
  <label>What happens <textarea rows="3" bind:value={body} maxlength="4000" placeholder="The text players see, and the log records"></textarea></label>
  <label>Severity
    <select bind:value={severity}>
      <option value="info">Info</option>
      <option value="warning">Warning</option>
      <option value="critical">Critical</option>
    </select>
  </label>

  <VisibilityPicker label="Sent to" value={audience} teams={ex.teams} onchange={(v) => (audience = v)} />

  <fieldset>
    <legend>When fired</legend>
    {#each effects as eff, i (i)}
      <div class="effect">
        {#if eff.type === 'set_status'}
          <select bind:value={eff.asset} aria-label="Asset">
            {#each assets as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
          </select>
          <div class="row">
            <select bind:value={eff.set} aria-label="Status set" onchange={() => (eff.value = sets.find((s) => s.id === eff.set)?.values[0]?.id ?? '')}>
              {#each sets as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
            </select>
            <select bind:value={eff.value} aria-label="New status">
              {#each sets.find((s) => s.id === eff.set)?.values ?? [] as v (v.id)}<option value={v.id}>{v.label}</option>{/each}
            </select>
          </div>
        {:else}
          <input bind:value={eff.asset.name} placeholder="Name of the new asset" required maxlength="200" aria-label="New asset name" />
          <select bind:value={eff.asset.type_id} aria-label="New asset type">
            {#each ex.taxonomy.asset_types as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
          </select>
        {/if}
        <button type="button" class="ghost small" onclick={() => effects.splice(i, 1)}>Remove</button>
      </div>
    {/each}
    <div class="row">
      <button type="button" class="small" onclick={addStatus} disabled={!assets.length}>+ Change a status</button>
      <button type="button" class="small" onclick={addAsset}>+ Create an asset…</button>
    </div>
  </fieldset>

  <label>Facilitator notes <textarea rows="2" bind:value={notes} maxlength="4000" placeholder="Not shown to players"></textarea></label>
  <div class="row end">
    <button type="button" onclick={ondone}>Cancel</button>
    <button class="primary" disabled={!title.trim()}>Queue inject</button>
  </div>
</form>

<style>
  form { display: grid; gap: 10px; }
  h2 { font-size: 16px; }
  fieldset { border: 1px solid var(--line); border-radius: var(--radius); padding: 8px; display: grid; gap: 8px; margin: 0; }
  legend { font-size: 12px; color: var(--muted); padding: 0 4px; }
  .effect { display: grid; gap: 6px; padding-bottom: 8px; border-bottom: 1px dashed var(--line); }
  .small { font-size: 12px; justify-self: start; }
  .end { justify-content: flex-end; }
</style>
