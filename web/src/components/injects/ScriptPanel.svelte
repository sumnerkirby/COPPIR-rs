<script lang="ts">
  // The exercise script: phases, injects grouped by what fires them, and
  // polls. The Timeline lays the same script out against the clock.
  import { elapsedMs, fmtMinutes, plannedMinute } from '../../lib/clock.svelte';
  import { describeEffect, describeTrigger, describeVisibility } from '../../lib/script';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Inject } from '../../lib/types/Inject';
  import Icon from '../ui/Icon.svelte';
  import InjectForm from './InjectForm.svelte';
  import PhasesEditor from './PhasesEditor.svelte';
  import PollCard from './PollCard.svelte';
  import PollForm from './PollForm.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const now = $derived(elapsedMs(ex.clock) / 60000);
  const all = $derived(Object.values(ex.injects));

  const scheduled = $derived(
    all
      .filter((i) => !i.fired_at && (i.trigger.type === 'at' || i.trigger.type === 'phase'))
      .map((i) => ({ i, at: plannedMinute(ex, i) }))
      .sort((a, b) => (a.at ?? Infinity) - (b.at ?? Infinity)),
  );
  const conditional = $derived(all.filter((i) => !i.fired_at && i.trigger.type === 'when'));
  const manual = $derived(all.filter((i) => !i.fired_at && i.trigger.type === 'manual'));
  const fired = $derived(all.filter((i) => i.fired_at).sort((a, b) => (b.fired_at ?? '').localeCompare(a.fired_at ?? '')));
  const polls = $derived(Object.values(ex.polls));

  async function remove(i: Inject) {
    if (await ui.confirm(`Delete inject “${i.title}”?`, { confirm: 'Delete', danger: true })) await ws.send({ type: 'remove_inject', id: i.id });
  }

  const due = (at: number | null) => {
    if (at === null) return 'phase not begun';
    const d = at - now;
    return d <= 0 ? 'due now' : `in ${fmtMinutes(d)}`;
  };
  const editingInject = $derived(ws.editing?.kind === 'inject' ? ws.editing : null);
</script>

{#snippet card(i: Inject, when?: string)}
  <article class="inject {i.severity}" class:disarmed={!i.armed && i.trigger.type !== 'manual'}>
    <div class="row">
      <strong class="grow">{i.title}</strong>
      {#if when}<span class="when mono">{when}</span>{/if}
    </div>
    <p class="trigger">{describeTrigger(ex, i.trigger)}{#if i.audience.mode !== 'all'} · to {describeVisibility(ex, i.audience)}{/if}</p>
    {#each i.effects as e, n (n)}<p class="effect muted">↳ {describeEffect(ex, e)}</p>{/each}
    {#if i.notes}<p class="notes"><Icon name="eye-off" size={12} /> {i.notes}</p>{/if}
    <div class="row actions">
      <button class="small fire" onclick={() => ws.send({ type: 'fire_inject', id: i.id })}>Fire now</button>
      {#if i.trigger.type !== 'manual'}
        <button class="ghost small" onclick={() => ws.send({ type: 'arm_inject', id: i.id, armed: !i.armed })} title={i.armed ? 'Stop it firing by itself' : 'Let it fire by itself again'}>
          {i.armed ? 'Disarm' : 'Arm'}
        </button>
      {/if}
      <span class="grow"></span>
      <button class="ghost small" onclick={() => (ws.editing = { kind: 'inject', id: i.id })} aria-label={`Edit ${i.title}`}><Icon name="pencil" size={14} /></button>
      <button class="ghost small" onclick={() => remove(i)} aria-label={`Delete ${i.title}`}><Icon name="trash-2" size={14} /></button>
    </div>
  </article>
{/snippet}

{#if editingInject}
  {#key editingInject.id}
    <InjectForm initial={editingInject.id ? ex.injects[editingInject.id] : undefined} ondone={() => (ws.editing = null)} />
  {/key}
{:else if ws.editing?.kind === 'poll'}
  <PollForm ondone={() => (ws.editing = null)} />
{:else}
  <div class="row wrap">
    <button class="primary icon" onclick={() => (ws.editing = { kind: 'inject' })}><Icon name="plus" size={14} /> Inject</button>
    <button class="icon" onclick={() => (ws.editing = { kind: 'poll' })}><Icon name="vote" size={14} /> Poll</button>
    <span class="grow"></span>
    <button class="icon" onclick={() => (ws.showTimeline = true)}><Icon name="gantt-chart" size={14} /> Timeline</button>
  </div>

  <PhasesEditor />

  {#if scheduled.length}
    <section>
      <h3 class="section-title">Scheduled ({scheduled.length})</h3>
      {#each scheduled as { i, at } (i.id)}{@render card(i, i.armed ? due(at) : 'disarmed')}{/each}
    </section>
  {/if}
  {#if conditional.length}
    <section>
      <h3 class="section-title">Waiting on a condition ({conditional.length})</h3>
      {#each conditional as i (i.id)}{@render card(i, i.armed ? undefined : 'disarmed')}{/each}
    </section>
  {/if}
  <section>
    <h3 class="section-title">Fire by hand ({manual.length})</h3>
    {#each manual as i (i.id)}{@render card(i)}{:else}<p class="empty">None.</p>{/each}
  </section>

  {#if polls.length}
    <section>
      <h3 class="section-title">Polls ({polls.length})</h3>
      {#each polls as p (p.id)}<PollCard poll={p} />{/each}
    </section>
  {/if}

  {#if fired.length}
    <section>
      <h3 class="section-title">Fired ({fired.length})</h3>
      {#each fired as i (i.id)}
        <div class="row fired {i.severity}">
          <span class="grow">{i.title}</span>
          <span class="mono muted small">✓ {new Date(i.fired_at ?? '').toLocaleTimeString()}</span>
        </div>
      {/each}
    </section>
  {/if}
{/if}

<style>
  section { display: grid; gap: 8px; }
  .wrap { flex-wrap: wrap; }
  .inject { border: 1px solid var(--line); border-left: 4px solid #3399ff; border-radius: var(--radius); padding: 8px 10px; display: grid; gap: 3px; background: var(--panel-2); }
  .inject.warning { border-left-color: var(--warn); }
  .inject.critical { border-left-color: var(--danger); }
  .inject.disarmed { opacity: 0.55; border-style: dashed; }
  .inject p { margin: 0; font-size: 12px; }
  .trigger { color: var(--accent); }
  .when { font-size: 11px; color: var(--muted); white-space: nowrap; }
  .notes { color: var(--warn); }
  .actions { margin-top: 4px; }
  .fire { background: var(--danger); color: #1a0000; border-color: var(--danger); font-weight: 700; }
  .small { font-size: 12px; padding: 3px 8px; }
  .fired { font-size: 13px; opacity: 0.7; border-left: 3px solid #3399ff; padding-left: 8px; }
  .fired.warning { border-left-color: var(--warn); }
  .fired.critical { border-left-color: var(--danger); }
</style>
