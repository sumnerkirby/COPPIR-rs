<script lang="ts">
  // Edit the exercise's vocabulary: status sets, asset types and sectors. Works
  // on a draft and saves it as one `set_taxonomy` command, so a half-finished
  // edit never reaches other windows. The server refuses anything invalid,
  // including removing a type that assets still use.
  import { api } from '../../lib/api';
  import { iconNames } from '../../lib/icons';
  import { uniqueSlug } from '../../lib/slug';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Taxonomy } from '../../lib/types/Taxonomy';
  import Icon from '../ui/Icon.svelte';
  import Modal from '../ui/Modal.svelte';

  let { onclose }: { onclose: () => void } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  let draft = $state<Taxonomy>(structuredClone($state.snapshot(ws.exercise!.taxonomy)));
  let tab = $state<'statuses' | 'types' | 'sectors'>('statuses');
  let saving = $state(false);
  const icons = iconNames();

  const usage = $derived(
    Object.values(ex.assets).reduce<Record<string, number>>((m, a) => ((m[a.type_id] = (m[a.type_id] ?? 0) + 1), m), {}),
  );

  /** Give anything new an id derived from its name. */
  function assignIds(t: Taxonomy) {
    const fix = <T extends { id: string }>(items: T[], name: (x: T) => string) => {
      for (const it of items) if (!it.id) it.id = uniqueSlug(name(it), items.map((x) => x.id).filter(Boolean));
    };
    fix(t.asset_types, (x) => x.name);
    fix(t.status_sets, (x) => x.name);
    fix(t.sectors, (x) => x.name);
    for (const s of t.status_sets) {
      fix(s.values, (x) => x.label);
      if (!s.values.some((v) => v.id === s.default)) s.default = s.values[0]?.id ?? '';
    }
  }

  async function save() {
    const t = structuredClone($state.snapshot(draft));
    assignIds(t);
    saving = true;
    const r = await ws.send({ type: 'set_taxonomy', taxonomy: t });
    saving = false;
    if (r) {
      ui.toast('Taxonomy saved', 'ok');
      onclose();
    }
  }

  async function reset() {
    if (!(await ui.confirm('Start from the default taxonomy?', { body: 'Your edits here are replaced. Nothing is saved until you press Save.', confirm: 'Reset' }))) return;
    draft = await api.defaultTaxonomy();
  }

  const move = <T,>(list: T[], i: number, d: number) => {
    const j = i + d;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j]!, list[i]!];
  };

  function addSet() {
    draft.status_sets.push({
      id: '',
      name: 'New status',
      display: 'detail',
      default: '',
      values: [{ id: '', label: 'OK', color: '#33cc33', glyph: '✓', score: 100 }],
    });
  }
</script>

<Modal title="Taxonomy" {onclose} wide>
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === 'statuses'} class:on={tab === 'statuses'} onclick={() => (tab = 'statuses')}>Statuses</button>
    <button role="tab" aria-selected={tab === 'types'} class:on={tab === 'types'} onclick={() => (tab = 'types')}>Asset types</button>
    <button role="tab" aria-selected={tab === 'sectors'} class:on={tab === 'sectors'} onclick={() => (tab = 'sectors')}>Sectors</button>
  </div>

  {#if tab === 'statuses'}
    <p class="empty">
      Each status set is a list from best to worst. The <b>fill</b> set colours a marker and adds its glyph; the <b>ring</b> set colours the
      ring, and its glyph names the ring style (solid, dashed, double, dotted). Scores (0–100) drive sector health.
    </p>
    {#each draft.status_sets as set, si (si)}
      <fieldset>
        <div class="row">
          <label class="grow">Name<input bind:value={set.name} maxlength="100" /></label>
          <label>Shown as
            <select bind:value={set.display}>
              <option value="fill">Fill</option><option value="ring">Ring</option><option value="detail">Details only</option>
            </select>
          </label>
          <label>Starts as
            <select bind:value={set.default}>
              {#each set.values as v, vi (vi)}<option value={v.id}>{v.label}</option>{/each}
            </select>
          </label>
          <button class="ghost danger" onclick={() => draft.status_sets.splice(si, 1)} disabled={draft.status_sets.length === 1} aria-label="Delete status set"><Icon name="trash-2" size={14} /></button>
        </div>
        <table>
          <thead><tr><th>Label</th><th>Colour</th><th>Glyph</th><th>Score</th><th></th></tr></thead>
          <tbody>
            {#each set.values as v, vi (vi)}
              <tr>
                <td><input bind:value={v.label} maxlength="100" aria-label="Label" /></td>
                <td><input type="color" bind:value={v.color} aria-label="Colour" /></td>
                <td class="glyph"><input bind:value={v.glyph} maxlength="16" aria-label="Glyph" /></td>
                <td class="score"><input type="number" min="0" max="100" bind:value={v.score} aria-label="Score" /></td>
                <td class="actions">
                  <button class="ghost small" onclick={() => move(set.values, vi, -1)} aria-label="Move up">↑</button>
                  <button class="ghost small" onclick={() => move(set.values, vi, 1)} aria-label="Move down">↓</button>
                  <button class="ghost small" onclick={() => set.values.splice(vi, 1)} disabled={set.values.length === 1} aria-label="Delete">✕</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <button class="small" onclick={() => set.values.push({ id: '', label: 'New', color: '#888888', glyph: '', score: 50 })}>+ Value</button>
      </fieldset>
    {/each}
    <button onclick={addSet}>+ Status set</button>
  {:else if tab === 'types'}
    <datalist id="icon-names">{#each icons as n (n)}<option value={n}></option>{/each}</datalist>
    <table>
      <thead><tr><th></th><th>Name</th><th>Icon (Lucide name)</th><th>Colour</th><th>In use</th><th></th></tr></thead>
      <tbody>
        {#each draft.asset_types as t, ti (ti)}
          <tr>
            <td class="preview" style:color={t.color}><Icon name={t.icon} size={18} /></td>
            <td><input bind:value={t.name} maxlength="100" aria-label="Name" /></td>
            <td><input bind:value={t.icon} list="icon-names" aria-label="Icon" /></td>
            <td><input type="color" bind:value={t.color} aria-label="Colour" /></td>
            <td class="muted">{usage[t.id] ?? 0}</td>
            <td class="actions">
              <button class="ghost small" onclick={() => draft.asset_types.splice(ti, 1)} disabled={!!usage[t.id]} title={usage[t.id] ? 'In use by assets' : 'Delete'} aria-label="Delete">✕</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <button onclick={() => draft.asset_types.push({ id: '', name: 'New type', icon: 'circle', color: '#888888' })}>+ Asset type</button>
  {:else}
    {#each draft.sectors as s, si (si)}
      <fieldset>
        <div class="row">
          <label class="grow">Name<input bind:value={s.name} maxlength="100" /></label>
          <label>Colour<input type="color" bind:value={s.color} /></label>
          <label>Health from
            <select bind:value={s.status_set}>
              {#each draft.status_sets as set, i (i)}<option value={set.id}>{set.name}</option>{/each}
            </select>
          </label>
          <button class="ghost danger" onclick={() => draft.sectors.splice(si, 1)} aria-label="Delete sector"><Icon name="trash-2" size={14} /></button>
        </div>
        <div class="members">
          {#each draft.asset_types.filter((t) => t.id) as t (t.id)}
            <label class="check">
              <input
                type="checkbox"
                checked={s.asset_types.includes(t.id)}
                onchange={(e) => (s.asset_types = e.currentTarget.checked ? [...s.asset_types, t.id] : s.asset_types.filter((x) => x !== t.id))}
              />
              {t.name}
            </label>
          {/each}
        </div>
      </fieldset>
    {/each}
    <button onclick={() => draft.sectors.push({ id: '', name: 'New sector', color: '#00ccaa', asset_types: [], status_set: draft.status_sets[0]?.id ?? '' })}>+ Sector</button>
    <p class="empty">New asset types can join a sector once they are saved.</p>
  {/if}

  {#snippet footer()}
    <button class="ghost" onclick={reset}>Reset to default…</button>
    <span class="grow"></span>
    <button onclick={onclose}>Cancel</button>
    <button class="primary" onclick={save} disabled={saving}>{saving ? 'Saving…' : 'Save'}</button>
  {/snippet}
</Modal>

<style>
  .tabs { display: flex; gap: 4px; }
  .tabs button.on { background: var(--accent); color: var(--accent-ink); }
  fieldset { border: 1px solid var(--line); border-radius: var(--radius); padding: 10px; display: grid; gap: 8px; margin: 0; }
  .row { align-items: end; flex-wrap: wrap; }
  table { border-collapse: collapse; width: 100%; }
  th { text-align: left; font-size: 11px; color: var(--muted); font-weight: 500; padding: 2px 4px; }
  td { padding: 2px 4px; }
  .glyph { width: 80px; }
  .score { width: 80px; }
  .actions { white-space: nowrap; width: 1%; }
  .preview { width: 30px; text-align: center; }
  .small { font-size: 12px; padding: 2px 6px; justify-self: start; }
  .members { display: flex; flex-wrap: wrap; gap: 4px 14px; }
  .check { flex-direction: row; display: flex; align-items: center; gap: 6px; color: var(--text); font-size: 13px; }
</style>
