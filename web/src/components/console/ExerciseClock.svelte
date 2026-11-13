<script lang="ts">
  // The shared exercise clock and current phase. The server keeps both, so
  // every window, phone and projector shows the same time, and scheduled
  // injects fire by it.
  import { currentPhase, elapsedMs, fmtClock } from '../../lib/clock.svelte';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import Icon from '../ui/Icon.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const running = $derived(!!ex.clock.running_since);
  const phase = $derived(currentPhase(ex));
  const next = $derived.by(() => {
    const i = phase ? ex.phases.findIndex((p) => p.id === phase.id) : -1;
    return ex.phases[i + 1];
  });
  let menu = $state(false);

  async function jump() {
    menu = false;
    const v = await ui.prompt('Set the clock', 'Minutes since the start', String(Math.floor(elapsedMs(ex.clock) / 60000)), 'Set');
    const n = Number(v);
    if (v !== null && Number.isFinite(n) && n >= 0) await ws.send({ type: 'set_clock', minutes: n });
  }

  async function reset() {
    menu = false;
    const ok = await ui.confirm('Reset the clock?', {
      body: 'The clock goes back to zero, stopped, and no phase is current. Fired injects stay fired.',
      confirm: 'Reset',
    });
    if (ok) await ws.send({ type: 'reset_clock' });
  }
</script>

<div class="clock" role="timer" aria-label="Exercise clock">
  <span class="time mono" class:running>T+{fmtClock(elapsedMs(ex.clock))}</span>
  {#if !ws.readonly}
    <button class="ghost small" onclick={() => ws.send({ type: running ? 'pause_clock' : 'start_clock' })} aria-label={running ? 'Pause the clock' : 'Start the clock'}>
      <Icon name={running ? 'pause' : 'play'} size={14} />
    </button>
  {/if}
  {#if phase}
    <span class="phase" style:--c={phase.color}>{phase.name}</span>
  {/if}
  {#if !ws.readonly}
    {#if next}
      <button class="ghost small next" onclick={() => ws.send({ type: 'enter_phase', id: next.id })} title="Begin the next phase">
        → {next.name}
      </button>
    {/if}
    <div class="more">
      <button class="ghost small" onclick={() => (menu = !menu)} aria-label="Clock options" aria-expanded={menu}><Icon name="chevron-down" size={14} /></button>
      {#if menu}
        <div class="menu" role="menu">
          <button role="menuitem" onclick={jump}>Set the time…</button>
          {#each ex.phases as p (p.id)}
            <button role="menuitem" onclick={() => { menu = false; ws.send({ type: 'enter_phase', id: p.id }); }}>Begin {p.name}</button>
          {/each}
          <button role="menuitem" onclick={reset}>Reset for a new run…</button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<svelte:window onclick={(e) => { if (menu && !(e.target as HTMLElement).closest('.more')) menu = false; }} />

<style>
  .clock { display: flex; align-items: center; gap: 4px; }
  .time { font-size: 14px; }
  .running { color: var(--accent); }
  .small { padding: 2px 6px; font-size: 12px; }
  .phase { border: 1px solid var(--c); color: var(--c); border-radius: 999px; padding: 0 9px; font-size: 12px; white-space: nowrap; }
  .next { color: var(--muted); white-space: nowrap; }
  .more { position: relative; }
  .menu {
    position: absolute; top: calc(100% + 4px); left: 0; z-index: 30; display: grid; min-width: 200px; padding: 4px;
    background: var(--panel-2); border: 1px solid var(--line); border-radius: var(--radius);
  }
  .menu button { border: 0; background: none; text-align: left; padding: 7px 10px; font-size: 13px; }
  .menu button:hover { background: var(--panel); }
  @media (max-width: 700px) { .next { display: none; } }
</style>
