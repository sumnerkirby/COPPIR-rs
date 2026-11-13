<script lang="ts">
  // Write or rewrite an inject: what players see, who sees it, what sets it
  // off, and what it does to the picture.
  import { untrack } from 'svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Condition } from '../../lib/types/Condition';
  import type { Effect } from '../../lib/types/Effect';
  import type { Inject } from '../../lib/types/Inject';
  import type { NewExpected } from '../../lib/types/NewExpected';
  import type { Severity } from '../../lib/types/Severity';
  import type { Trigger } from '../../lib/types/Trigger';
  import type { Visibility } from '../../lib/types/Visibility';
  import VisibilityPicker from '../ui/VisibilityPicker.svelte';

  let { initial, ondone }: { initial?: Inject; ondone: () => void } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const sets = $derived(ex.taxonomy.status_sets);
  const assets = $derived(Object.values(ex.assets).sort((a, b) => a.name.localeCompare(b.name)));
  const polls = $derived(Object.values(ex.polls));
  const otherInjects = $derived(Object.values(ex.injects).filter((i) => i.id !== initial?.id));

  // A form edits a copy, taken once when it opens.
  const start = untrack(() => (initial ? structuredClone($state.snapshot(initial)) : null));
  let title = $state(start?.title ?? '');
  let body = $state(start?.body ?? '');
  let notes = $state(start?.notes ?? '');
  let severity = $state<Severity>(start?.severity ?? 'warning');
  let audience = $state<Visibility>(start?.audience ?? { mode: 'all' });
  let effects = $state<Effect[]>(start?.effects ?? []);
  let trigger = $state<Trigger>(start?.trigger ?? { type: 'manual' });
  let expected = $state<NewExpected[]>(start?.expected.map(({ id, text, team }) => ({ id, text, team })) ?? []);

  const firstSet = () => sets[0];

  function setTriggerType(type: Trigger['type']) {
    if (type === 'manual') trigger = { type };
    else if (type === 'at') trigger = { type, minutes: Math.ceil((ex.clock.elapsed_ms || 0) / 60000) + 10 };
    else if (type === 'phase') trigger = { type, phase: ex.phases[0]?.id ?? '', delay_minutes: 0 };
    else trigger = { type, condition: defaultCondition('asset_status') };
  }

  function defaultCondition(type: Condition['type']): Condition {
    switch (type) {
      case 'asset_status':
        return { type, asset: assets[0]?.id ?? '', set: firstSet()?.id ?? '', value: firstSet()?.values.at(-1)?.id ?? '' };
      case 'inject_fired':
        return { type, inject: otherInjects[0]?.id ?? '' };
      case 'threshold_breached':
        return { type, threshold: ex.thresholds[0]?.id ?? '' };
      case 'poll_choice':
        return { type, poll: polls[0]?.id ?? '', option: polls[0]?.options[0]?.id ?? '' };
    }
  }

  function addEffect(type: Effect['type']) {
    const set = firstSet();
    switch (type) {
      case 'set_status':
        effects.push({ type, asset: assets[0]?.id ?? '', set: set?.id ?? '', value: set?.values.at(-1)?.id ?? '' });
        break;
      case 'reveal_asset':
        effects.push({
          type,
          asset: assets.find((a) => a.visibility.mode !== 'all')?.id ?? assets[0]?.id ?? '',
          visibility: { mode: 'all' },
        });
        break;
      case 'open_poll':
        effects.push({ type, poll: polls.find((p) => p.state === 'draft')?.id ?? polls[0]?.id ?? '' });
        break;
      case 'enter_phase':
        effects.push({ type, phase: ex.phases[0]?.id ?? '' });
        break;
      case 'create_asset':
        void ws.pickLocation('Click the map where the inject should create its asset').then((at) => {
          if (!at) return;
          const t = ex.taxonomy.asset_types.find((x) => x.id === 'asset') ?? ex.taxonomy.asset_types[0];
          effects.push({ type, asset: { name: '', type_id: t?.id ?? '', geo: at } });
        });
        break;
    }
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    const inject = { title, body, severity, audience, effects, notes, trigger, expected: expected.filter((x) => x.text.trim()) };
    const r = initial
      ? await ws.send({ type: 'edit_inject', id: initial.id, inject })
      : await ws.send({ type: 'add_inject', inject });
    if (r) ondone();
  }

  const valuesOf = (set: string) => sets.find((s) => s.id === set)?.values ?? [];
</script>

<form onsubmit={save}>
  <h2>{initial ? 'Edit inject' : 'New inject'}</h2>
  <!-- svelte-ignore a11y_autofocus -->
  <label>Title <input bind:value={title} required maxlength="200" autofocus /></label>
  <label>What happens <textarea rows="3" bind:value={body} maxlength="4000" placeholder="The text players see, and the log records"></textarea></label>
  <label>Severity
    <select bind:value={severity}>
      <option value="info">Info</option><option value="warning">Warning</option><option value="critical">Critical</option>
    </select>
  </label>
  <VisibilityPicker label="Sent to" value={audience} teams={ex.teams} onchange={(v) => (audience = v)} />

  <fieldset>
    <legend>Fires</legend>
    <select value={trigger.type} onchange={(e) => setTriggerType(e.currentTarget.value as Trigger['type'])} aria-label="When it fires">
      <option value="manual">When I fire it</option>
      <option value="at">At a time on the exercise clock</option>
      <option value="phase" disabled={!ex.phases.length}>When a phase begins{ex.phases.length ? '' : ' (add phases first)'}</option>
      <option value="when">When something happens</option>
    </select>
    {#if trigger.type === 'at'}
      <label class="inline">T+ <input type="number" min="0" step="1" bind:value={trigger.minutes} /> minutes</label>
    {:else if trigger.type === 'phase'}
      <div class="row">
        <input class="mins" type="number" min="0" step="1" bind:value={trigger.delay_minutes} aria-label="Minutes after" />
        <span class="muted">minutes into</span>
        <select class="grow" bind:value={trigger.phase} aria-label="Phase">
          {#each ex.phases as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
        </select>
      </div>
    {:else if trigger.type === 'when'}
      {@const c = trigger.condition}
      <select
        value={c.type}
        onchange={(e) => trigger.type === 'when' && (trigger.condition = defaultCondition(e.currentTarget.value as Condition['type']))}
        aria-label="Condition"
      >
        <option value="asset_status">An asset reaches a status</option>
        <option value="inject_fired" disabled={!otherInjects.length}>Another inject fires</option>
        <option value="threshold_breached" disabled={!ex.thresholds.length}>A threshold is breached</option>
        <option value="poll_choice" disabled={!polls.length}>A poll closes with an answer</option>
      </select>
      {#if c.type === 'asset_status'}
        <select bind:value={c.asset} aria-label="Asset">{#each assets as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select>
        <div class="row">
          <select bind:value={c.set} aria-label="Status set" onchange={() => (c.value = valuesOf(c.set)[0]?.id ?? '')}>
            {#each sets as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
          </select>
          <select class="grow" bind:value={c.value} aria-label="Status">
            {#each valuesOf(c.set) as v (v.id)}<option value={v.id}>{v.label}</option>{/each}
          </select>
        </div>
      {:else if c.type === 'inject_fired'}
        <select bind:value={c.inject} aria-label="Inject">{#each otherInjects as i (i.id)}<option value={i.id}>{i.title}</option>{/each}</select>
      {:else if c.type === 'threshold_breached'}
        <select bind:value={c.threshold} aria-label="Threshold">{#each ex.thresholds as t (t.id)}<option value={t.id}>{t.name}</option>{/each}</select>
      {:else}
        <select bind:value={c.poll} aria-label="Poll" onchange={() => (c.option = ex.polls[c.poll]?.options[0]?.id ?? '')}>
          {#each polls as p (p.id)}<option value={p.id}>{p.question}</option>{/each}
        </select>
        <select bind:value={c.option} aria-label="Answer">
          {#each ex.polls[c.poll]?.options ?? [] as o (o.id)}<option value={o.id}>{o.label}</option>{/each}
        </select>
      {/if}
    {/if}
  </fieldset>

  <fieldset>
    <legend>When fired, also</legend>
    {#each effects as eff, i (i)}
      <div class="effect">
        {#if eff.type === 'set_status'}
          <select bind:value={eff.asset} aria-label="Asset">{#each assets as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select>
          <div class="row">
            <select bind:value={eff.set} aria-label="Status set" onchange={() => (eff.value = valuesOf(eff.set)[0]?.id ?? '')}>
              {#each sets as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
            </select>
            <select class="grow" bind:value={eff.value} aria-label="New status">
              {#each valuesOf(eff.set) as v (v.id)}<option value={v.id}>{v.label}</option>{/each}
            </select>
          </div>
        {:else if eff.type === 'create_asset'}
          <input bind:value={eff.asset.name} placeholder="Name of the new asset" required maxlength="200" aria-label="New asset name" />
          <select bind:value={eff.asset.type_id} aria-label="New asset type">
            {#each ex.taxonomy.asset_types as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
          </select>
        {:else if eff.type === 'reveal_asset'}
          <select bind:value={eff.asset} aria-label="Asset to show">{#each assets as a (a.id)}<option value={a.id}>{a.name}</option>{/each}</select>
          <VisibilityPicker label="Show it to" value={eff.visibility} teams={ex.teams} onchange={(v) => eff.type === 'reveal_asset' && (eff.visibility = v)} />
        {:else if eff.type === 'open_poll'}
          <select bind:value={eff.poll} aria-label="Poll to open">{#each polls as p (p.id)}<option value={p.id}>{p.question}</option>{/each}</select>
        {:else}
          <select bind:value={eff.phase} aria-label="Phase to begin">{#each ex.phases as p (p.id)}<option value={p.id}>Begin {p.name}</option>{/each}</select>
        {/if}
        <button type="button" class="ghost small" onclick={() => effects.splice(i, 1)}>Remove</button>
      </div>
    {/each}
    <div class="row wrap">
      <button type="button" class="small" onclick={() => addEffect('set_status')} disabled={!assets.length}>+ Change a status</button>
      <button type="button" class="small" onclick={() => addEffect('reveal_asset')} disabled={!assets.length}>+ Show an asset</button>
      <button type="button" class="small" onclick={() => addEffect('create_asset')}>+ Create an asset…</button>
      <button type="button" class="small" onclick={() => addEffect('open_poll')} disabled={!polls.length}>+ Open a poll</button>
      <button type="button" class="small" onclick={() => addEffect('enter_phase')} disabled={!ex.phases.length}>+ Begin a phase</button>
    </div>
  </fieldset>

  <fieldset>
    <legend>Players should</legend>
    {#each expected as x, i (i)}
      <div class="row">
        <input class="grow" bind:value={x.text} maxlength="500" placeholder="An action you expect, e.g. isolate the host" aria-label="Expected action" />
        {#if ex.teams.length}
          <select bind:value={x.team} aria-label="Team expected to act" class="team">
            <option value={null}>Anyone</option>
            {#each ex.teams as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
          </select>
        {/if}
        <button type="button" class="ghost small" onclick={() => expected.splice(i, 1)} aria-label="Remove">✕</button>
      </div>
    {/each}
    <button type="button" class="small" onclick={() => expected.push({ text: '', team: null })} disabled={expected.length >= 20}>+ Expected action</button>
    <p class="muted hint">Judged in the review, and scored in the after-action report.</p>
  </fieldset>

  <label>Facilitator notes <textarea rows="2" bind:value={notes} maxlength="4000" placeholder="Not shown to players"></textarea></label>
  <div class="row end">
    <button type="button" onclick={ondone}>Cancel</button>
    <button class="primary" disabled={!title.trim()}>{initial ? 'Save' : 'Add to script'}</button>
  </div>
</form>

<style>
  form { display: grid; gap: 10px; }
  h2 { font-size: 16px; }
  fieldset { border: 1px solid var(--line); border-radius: var(--radius); padding: 8px; display: grid; gap: 8px; margin: 0; }
  legend { font-size: 12px; color: var(--muted); padding: 0 4px; }
  .effect { display: grid; gap: 6px; padding-bottom: 8px; border-bottom: 1px dashed var(--line); }
  .small { font-size: 12px; justify-self: start; }
  .inline { flex-direction: row; display: flex; align-items: center; gap: 6px; color: var(--text); font-size: 14px; }
  .inline input, .mins { width: 80px; }
  .wrap { flex-wrap: wrap; }
  .end { justify-content: flex-end; }
  .team { width: auto; max-width: 130px; }
  .hint { margin: 0; font-size: 12px; }
</style>
