<script lang="ts">
  // Expected against actual: for each inject, what the players should have
  // done, and the facilitator's verdict on whether they did. Jump the replay
  // to an inject to see what each team did next.
  import { fmtClock } from '../../lib/clock.svelte';
  import type { Workspace } from '../../lib/workspace.svelte';
  import type { Expected } from '../../lib/types/Expected';
  import type { Inject } from '../../lib/types/Inject';
  import type { Verdict } from '../../lib/types/Verdict';
  import Icon from '../ui/Icon.svelte';

  let { ws, fired, onseek }: { ws: Workspace; fired: Record<string, number>; onseek: (t: number) => void } = $props();
  const ex = $derived(ws.exercise!);
  let team = $state<string>('');

  const VERDICTS: { v: Verdict; label: string }[] = [
    { v: 'met', label: 'Met' },
    { v: 'partial', label: 'Partly' },
    { v: 'missed', label: 'Missed' },
  ];

  // Fired injects in the order they fired, then any others with actions to judge.
  const injects = $derived(
    Object.values(ex.injects)
      .filter((i) => i.fired_at || i.expected.length)
      .sort((a, b) => (fired[a.id] ?? Infinity) - (fired[b.id] ?? Infinity) || (a.fired_at ?? '').localeCompare(b.fired_at ?? '')),
  );
  const shown = (e: Expected) => !team || e.team === team;
  const all = $derived(injects.flatMap((i) => i.expected).filter(shown));
  const tally = $derived({
    met: all.filter((e) => e.verdict === 'met').length,
    partial: all.filter((e) => e.verdict === 'partial').length,
    missed: all.filter((e) => e.verdict === 'missed').length,
    open: all.filter((e) => !e.verdict).length,
  });
  const judged = $derived(tally.met + tally.partial + tally.missed);
  const score = $derived(judged ? Math.round(((tally.met + tally.partial / 2) / judged) * 100) : null);

  const teamName = (id: string | null) => (id ? (ex.teams.find((t) => t.id === id)?.name ?? '?') : 'Anyone');

  function assess(i: Inject, e: Expected, verdict: Verdict) {
    void ws.send({ type: 'assess', inject: i.id, action: e.id, verdict: e.verdict === verdict ? null : verdict });
  }
  function note(i: Inject, e: Expected, text: string) {
    if (text.trim() !== e.note) void ws.send({ type: 'assess', inject: i.id, action: e.id, verdict: e.verdict, note: text });
  }
  const keep = (list: Expected[]) => list.map(({ id, text, team }) => ({ id, text, team }));

  async function add(i: Inject, form: HTMLFormElement) {
    const data = new FormData(form);
    const text = String(data.get('text') ?? '').trim();
    if (!text) return;
    const t = String(data.get('team') ?? '') || null;
    const r = await ws.send({ type: 'set_expected', inject: i.id, expected: [...keep(i.expected), { text, team: t }] });
    if (r) form.reset();
  }
  function remove(i: Inject, e: Expected) {
    void ws.send({ type: 'set_expected', inject: i.id, expected: keep(i.expected.filter((x) => x.id !== e.id)) });
  }
</script>

<div class="assess">
  <div class="summary">
    <div class="bar" aria-hidden="true">
      {#if all.length}
        <span class="met" style:flex={tally.met}></span><span class="partial" style:flex={tally.partial}></span>
        <span class="missed" style:flex={tally.missed}></span><span class="open" style:flex={tally.open}></span>
      {/if}
    </div>
    <p>
      <b class="met-t">{tally.met}</b> met · <b class="partial-t">{tally.partial}</b> partly · <b class="missed-t">{tally.missed}</b> missed ·
      <b>{tally.open}</b> to judge{#if score !== null}<span class="score" title="Met counts fully, partly met half">{score}%</span>{/if}
    </p>
    {#if ex.teams.length}
      <select bind:value={team} aria-label="Show the actions of">
        <option value="">All teams</option>
        {#each ex.teams as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
      </select>
    {/if}
  </div>

  {#each injects as i (i.id)}
    <section class="inject {i.severity}">
      <header>
        <strong>{i.title}</strong>
        {#if fired[i.id] !== undefined}
          <button class="ghost small mono" onclick={() => onseek(fired[i.id]!)} title="Replay from here">T+{fmtClock(fired[i.id]!)} <Icon name="play" size={12} /></button>
        {:else}
          <span class="muted small">{i.fired_at ? 'fired' : 'not fired'}</span>
        {/if}
      </header>
      {#each i.expected.filter(shown) as e (e.id)}
        <div class="action">
          <div class="text">
            <span>{e.text}</span>
            <span class="team muted">{teamName(e.team)}</span>
            <button class="ghost tiny" onclick={() => remove(i, e)} title="Remove this action" aria-label="Remove {e.text}"><Icon name="x" size={12} /></button>
          </div>
          <div class="verdicts" role="group" aria-label="Verdict on {e.text}">
            {#each VERDICTS as { v, label } (v)}
              <button class="v {v}" class:on={e.verdict === v} aria-pressed={e.verdict === v} onclick={() => assess(i, e, v)}>{label}</button>
            {/each}
            <input class="note" value={e.note} placeholder="Note" maxlength="4000" aria-label="Note on {e.text}" onchange={(ev) => note(i, e, ev.currentTarget.value)} />
          </div>
        </div>
      {/each}
      <form class="add" onsubmit={(ev) => { ev.preventDefault(); void add(i, ev.currentTarget); }}>
        <input name="text" placeholder="Add an expected action" maxlength="500" aria-label="New expected action for {i.title}" />
        {#if ex.teams.length}
          <select name="team" aria-label="Team" value={team}>
            <option value="">Anyone</option>
            {#each ex.teams as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
          </select>
        {/if}
        <button class="small" aria-label="Add">Add</button>
      </form>
    </section>
  {:else}
    <p class="empty">No inject has fired yet, and none has expected actions. Add them to injects in the Script tab.</p>
  {/each}
</div>

<style>
  .assess { display: grid; gap: 10px; }
  .summary { display: grid; gap: 6px; }
  .summary p { margin: 0; font-size: 13px; }
  .bar { display: flex; height: 8px; border-radius: 4px; overflow: hidden; background: var(--panel-2); }
  .bar .met, .v.met.on { background: #2ea043; }
  .bar .partial, .v.partial.on { background: #d29922; }
  .bar .missed, .v.missed.on { background: #da3633; }
  .bar .open { background: var(--line); }
  .met-t { color: #2ea043; } .partial-t { color: #d29922; } .missed-t { color: #da3633; }
  .score { margin-left: 8px; font-weight: 700; color: var(--accent); }
  .inject { border: 1px solid var(--line); border-left: 4px solid #3399ff; border-radius: var(--radius); padding: 8px; display: grid; gap: 8px; }
  .inject.warning { border-left-color: var(--warn); }
  .inject.critical { border-left-color: var(--danger); }
  header { display: flex; align-items: center; gap: 8px; justify-content: space-between; }
  header strong { font-size: 14px; }
  .small { font-size: 12px; }
  .action { display: grid; gap: 4px; padding-bottom: 6px; border-bottom: 1px dashed var(--line); }
  .text { display: flex; gap: 6px; align-items: baseline; font-size: 13px; }
  .text > span:first-child { flex: 1; }
  .team { font-size: 11px; white-space: nowrap; }
  .tiny { padding: 0 2px; }
  .verdicts { display: flex; gap: 4px; align-items: center; }
  .v { font-size: 12px; padding: 3px 8px; }
  .v.on { color: #fff; border-color: transparent; }
  .note { flex: 1; min-width: 0; font-size: 12px; padding: 3px 6px; }
  .add { display: flex; gap: 4px; }
  .add input { flex: 1; min-width: 0; font-size: 12px; }
  .add select { width: auto; max-width: 120px; font-size: 12px; }
</style>
