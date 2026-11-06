<script lang="ts">
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Inject } from '../../lib/types/Inject';
  import Icon from '../ui/Icon.svelte';
  import InjectForm from './InjectForm.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const all = $derived(Object.values(ex.injects));
  const queued = $derived(all.filter((i) => !i.fired_at));
  const fired = $derived(all.filter((i) => i.fired_at).sort((a, b) => (b.fired_at ?? '').localeCompare(a.fired_at ?? '')));
  let creating = $state(false);

  async function fire(i: Inject) {
    await ws.send({ type: 'fire_inject', id: i.id });
  }

  async function remove(i: Inject) {
    const ok = await ui.confirm(`Delete inject “${i.title}”?`, { confirm: 'Delete', danger: true });
    if (ok) await ws.send({ type: 'remove_inject', id: i.id });
  }

  function describe(i: Inject): string[] {
    return i.effects.map((e) => {
      if (e.type === 'create_asset') return `Creates ${e.asset.name}`;
      const set = ex.taxonomy.status_sets.find((s) => s.id === e.set);
      const v = set?.values.find((x) => x.id === e.value);
      return `${ex.assets[e.asset]?.name ?? '(removed asset)'} → ${set?.name ?? e.set}: ${v?.label ?? e.value}`;
    });
  }
</script>

{#if creating}
  <InjectForm ondone={() => (creating = false)} />
{:else}
  <button class="primary icon" onclick={() => (creating = true)}><Icon name="plus" size={14} /> New inject</button>

  <section>
    <h3 class="section-title">Queued ({queued.length})</h3>
    {#if !queued.length}<p class="empty">Nothing queued.</p>{/if}
    {#each queued as i (i.id)}
      <article class="inject {i.severity}">
        <div class="row">
          <strong class="grow">{i.title}</strong>
          <button class="small fire" onclick={() => fire(i)}>Fire</button>
          <button class="ghost small" onclick={() => remove(i)} aria-label={`Delete ${i.title}`}><Icon name="trash-2" size={14} /></button>
        </div>
        {#if i.body}<p>{i.body}</p>{/if}
        {#each describe(i) as d (d)}<p class="effect muted">↳ {d}</p>{/each}
        {#if i.notes}<p class="notes"><Icon name="eye-off" size={12} /> {i.notes}</p>{/if}
      </article>
    {/each}
  </section>

  {#if fired.length}
    <section>
      <h3 class="section-title">Fired ({fired.length})</h3>
      {#each fired as i (i.id)}
        <article class="inject fired {i.severity}">
          <div class="row">
            <strong class="grow">{i.title}</strong>
            <span class="mono muted small">✓ {new Date(i.fired_at ?? '').toLocaleTimeString()}</span>
          </div>
        </article>
      {/each}
    </section>
  {/if}
{/if}

<style>
  section { display: grid; gap: 8px; }
  .inject { border: 1px solid var(--line); border-left: 4px solid #3399ff; border-radius: var(--radius); padding: 8px 10px; display: grid; gap: 4px; background: var(--panel-2); }
  .inject.warning { border-left-color: var(--warn); }
  .inject.critical { border-left-color: var(--danger); }
  .inject.fired { opacity: 0.6; }
  .inject p { margin: 0; font-size: 13px; }
  .effect { font-size: 12px !important; }
  .notes { color: var(--warn); font-size: 12px !important; }
  .fire { background: var(--danger); color: #1a0000; border-color: var(--danger); font-weight: 700; }
  .small { font-size: 12px; }
</style>
