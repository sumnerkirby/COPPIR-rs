<script lang="ts">
  // How a status change spreads along links: "when an asset goes Offline,
  // what depends on it becomes Degraded". Saved as one list.
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { ImpactRule } from '../../lib/types/ImpactRule';
  import type { LinkKind } from '../../lib/types/LinkKind';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const sets = $derived(ex.taxonomy.status_sets);
  let draft = $state<ImpactRule[]>([]);
  let dirty = $state(false);
  $effect(() => {
    const rules = ex.impact_rules;
    if (!dirty) draft = structuredClone($state.snapshot(rules));
  });

  const KINDS: [LinkKind, string][] = [
    ['depends_on', 'depends on it'],
    ['managed_by', 'is managed by it'],
    ['connects_to', 'it connects to'],
  ];
  const valuesOf = (set: string) => sets.find((s) => s.id === set)?.values ?? [];

  function add() {
    const op = sets.find((s) => s.display === 'ring') ?? sets[0];
    const vals = op?.values ?? [];
    draft.push({
      id: '',
      kind: 'depends_on',
      set: op?.id ?? '',
      when: vals.slice(-1).map((v) => v.id),
      then_set: op?.id ?? '',
      then_value: vals[Math.min(1, vals.length - 1)]?.id ?? '',
    });
    dirty = true;
  }

  function toggle(r: ImpactRule, v: string, on: boolean) {
    r.when = on ? [...r.when, v] : r.when.filter((x) => x !== v);
    dirty = true;
  }

  async function save() {
    if (await ws.send({ type: 'set_impact_rules', rules: draft })) dirty = false;
  }
</script>

<section>
  <h3 class="section-title">Impact rules</h3>
  <p class="empty">A status change spreads along links to whatever relies on the asset, and on down the chain. It never makes anything better than it is.</p>
  {#each draft as r, i (i)}
    <div class="rule" oninput={() => (dirty = true)} onchange={() => (dirty = true)}>
      <div class="row wrap">
        <span>When</span>
        <select bind:value={r.set} aria-label="Status set" onchange={() => (r.when = [])}>
          {#each sets as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
        </select>
        <span>becomes</span>
      </div>
      <div class="vals">
        {#each valuesOf(r.set) as v (v.id)}
          <label class="check"><input type="checkbox" checked={r.when.includes(v.id)} onchange={(e) => toggle(r, v.id, e.currentTarget.checked)} /> {v.label}</label>
        {/each}
      </div>
      <div class="row wrap">
        <span>anything that</span>
        <select bind:value={r.kind} aria-label="Link kind">{#each KINDS as [k, label] (k)}<option value={k}>{label}</option>{/each}</select>
      </div>
      <div class="row wrap">
        <span>becomes</span>
        <select bind:value={r.then_set} aria-label="Status set to change" onchange={() => (r.then_value = valuesOf(r.then_set)[0]?.id ?? '')}>
          {#each sets as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
        </select>
        <select bind:value={r.then_value} aria-label="New status">{#each valuesOf(r.then_set) as v (v.id)}<option value={v.id}>{v.label}</option>{/each}</select>
        <span class="grow"></span>
        <button class="ghost small" onclick={() => { draft.splice(i, 1); dirty = true; }} aria-label="Remove rule">✕</button>
      </div>
    </div>
  {/each}
  <div class="row">
    <button class="small" onclick={add}>+ Rule</button>
    <span class="grow"></span>
    {#if dirty}
      <button class="small" onclick={() => { dirty = false; draft = structuredClone($state.snapshot(ex.impact_rules)); }}>Cancel</button>
      <button class="small primary" onclick={save}>Save rules</button>
    {/if}
  </div>
</section>

<style>
  section { display: grid; gap: 8px; border-top: 1px solid var(--line); padding-top: 14px; }
  .rule { border: 1px solid var(--line); border-radius: var(--radius); padding: 8px; display: grid; gap: 6px; font-size: 13px; }
  .rule select { width: auto; }
  .wrap { flex-wrap: wrap; }
  .vals { display: flex; flex-wrap: wrap; gap: 4px 12px; }
  .check { flex-direction: row; display: flex; align-items: center; gap: 5px; color: var(--text); font-size: 13px; }
  .small { font-size: 12px; padding: 3px 8px; }
</style>
