<script lang="ts">
  // The master scenario events list as a timeline: planned phases as bands,
  // scheduled injects at their minute, and a line for "now". Drag an inject
  // to reschedule it; click one to edit it.
  import { elapsedMs, fmtClock, fmtMinutes, plannedMinute, plannedPhases } from '../../lib/clock.svelte';
  import { describeTrigger } from '../../lib/script';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Inject } from '../../lib/types/Inject';
  import Modal from '../ui/Modal.svelte';

  let { onclose }: { onclose: () => void } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  let width = $state(800);
  const now = $derived(elapsedMs(ex.clock) / 60000);
  const bands = $derived(plannedPhases(ex));
  const placed = $derived(
    Object.values(ex.injects)
      .map((i) => ({ i, at: plannedMinute(ex, i) }))
      .filter((x): x is { i: Inject; at: number } => x.at !== null)
      .sort((a, b) => a.at - b.at),
  );
  const unplaced = $derived(Object.values(ex.injects).filter((i) => plannedMinute(ex, i) === null && !i.fired_at));
  const span = $derived(
    Math.max(60, bands.at(-1)?.end ?? 0, (placed.at(-1)?.at ?? 0) + 10, now + 10),
  );
  const perMin = $derived(width / span);
  const x = (m: number) => m * perMin;

  // Lanes: put each inject in the first lane where it does not overlap.
  const CHIP = 150;
  const lanes = $derived.by(() => {
    const ends: number[] = [];
    return placed.map(({ i, at }) => {
      const left = x(at);
      let lane = ends.findIndex((e) => e <= left);
      if (lane < 0) lane = ends.length;
      ends[lane] = left + CHIP;
      return { i, at, lane };
    });
  });
  const laneCount = $derived(Math.max(1, ...lanes.map((l) => l.lane + 1)));

  const step = $derived([5, 10, 15, 30, 60, 120].find((s) => s * perMin >= 70) ?? 240);
  const ticks = $derived(Array.from({ length: Math.floor(span / step) + 1 }, (_, n) => n * step));

  // Dragging
  let drag = $state<{ id: string; startX: number; startAt: number; at: number; moved: boolean } | null>(null);

  // The window, not the chip, follows the pointer during a drag: it keeps
  // tracking when the pointer outruns the chip or leaves it.
  function down(e: PointerEvent, i: Inject, at: number) {
    if (e.button !== 0) return;
    const draggable = !i.fired_at && (i.trigger.type === 'at' || i.trigger.type === 'phase');
    drag = { id: i.id, startX: e.clientX, startAt: at, at, moved: false };
    const move = (ev: PointerEvent) => {
      if (!drag || !draggable) return;
      const dx = ev.clientX - drag.startX;
      if (Math.abs(dx) > 3) drag.moved = true;
      drag.at = Math.max(0, Math.round(drag.startAt + dx / perMin));
    };
    const up = (ev: PointerEvent) => {
      // The release position counts too, in case no move was reported.
      move(ev);
      removeEventListener('pointermove', move);
      removeEventListener('pointerup', up);
      void finish(i);
    };
    addEventListener('pointermove', move);
    addEventListener('pointerup', up);
    e.preventDefault();
  }

  async function finish(i: Inject) {
    const d = drag;
    drag = null;
    if (!d || !d.moved) {
      edit(i);
      return;
    }
    const delta = d.at - d.startAt;
    if (!delta) return;
    const t = i.trigger;
    const trigger =
      t.type === 'at' ? { ...t, minutes: Math.max(0, t.minutes + delta) }
      : t.type === 'phase' ? { ...t, delay_minutes: Math.max(0, t.delay_minutes + delta) }
      : t;
    const { title, body, severity, audience, effects, notes } = i;
    await ws.send({ type: 'edit_inject', id: i.id, inject: { title, body, severity, audience, effects, notes, trigger } });
  }

  function edit(i: Inject) {
    if (i.fired_at) return;
    ws.tab = 'injects';
    ws.editing = { kind: 'inject', id: i.id };
    onclose();
  }

  const actualStarts = $derived(ex.phase_log.map((e) => ({ e, phase: ex.phases.find((p) => p.id === e.phase) })));
</script>

<Modal title="Timeline" {onclose} wide>
  <p class="muted hint">
    Planned phases and scheduled injects against the exercise clock (now T+{fmtClock(elapsedMs(ex.clock))}). Drag an inject to move it; click
    one to edit it.
  </p>
  <div class="scroll">
    <div class="tl" bind:clientWidth={width}>
      <div class="axis">
        {#each ticks as t (t)}<span class="tick mono" style:left="{x(t)}px">{t ? fmtMinutes(t) : 'T+0'}</span>{/each}
      </div>
      <div class="bands">
        {#each bands as b (b.phase.id)}
          <div class="band" style:left="{x(b.start)}px" style:width="{Math.max(2, x(b.end) - x(b.start))}px" style:--c={b.phase.color}>
            <span>{b.phase.name}</span>
          </div>
        {:else}
          <p class="empty pad">No phases planned. Add them in the Script tab.</p>
        {/each}
        {#each actualStarts as { e, phase }, n (n)}
          {#if phase}<span class="began" style:left="{x(e.elapsed_ms / 60000)}px" style:--c={phase.color} title="{phase.name} began at T+{fmtClock(e.elapsed_ms)}"></span>{/if}
        {/each}
      </div>
      <div class="lanes" style:height="{laneCount * 34 + 8}px">
        {#each lanes as { i, at, lane } (i.id)}
          {@const shown = drag?.id === i.id ? drag.at : at}
          <button
            class="chip {i.severity}"
            class:fired={!!i.fired_at}
            class:disarmed={!i.armed}
            class:fixed={i.trigger.type !== 'at' && i.trigger.type !== 'phase'}
            style:left="{x(shown)}px"
            style:top="{lane * 34 + 4}px"
            title="{i.title}: {describeTrigger(ex, i.trigger)}"
            onpointerdown={(e) => down(e, i, at)}
            onkeydown={(e) => e.key === 'Enter' && edit(i)}
          >
            <span class="mono">{fmtMinutes(shown)}</span> {i.fired_at ? '✓ ' : ''}{i.title}
          </button>
        {/each}
        <div class="now" style:left="{x(now)}px" title="Now"></div>
      </div>
    </div>
  </div>

  {#if unplaced.length}
    <section>
      <h3 class="section-title">Not on the clock</h3>
      {#each unplaced as i (i.id)}
        <button class="ghost row unplaced {i.severity}" onclick={() => edit(i)}>
          <strong>{i.title}</strong><span class="muted">{describeTrigger(ex, i.trigger)}</span>
        </button>
      {/each}
    </section>
  {/if}
</Modal>

<style>
  .hint { margin: 0; font-size: 13px; }
  .scroll { overflow-x: auto; }
  .tl { position: relative; min-width: 700px; }
  .axis { position: relative; height: 18px; border-bottom: 1px solid var(--line); }
  .tick { position: absolute; font-size: 10px; color: var(--muted); transform: translateX(-2px); white-space: nowrap; }
  .tick::before { content: ''; position: absolute; left: 0; top: 14px; height: 6px; border-left: 1px solid var(--line); }
  .bands { position: relative; height: 30px; margin-top: 4px; }
  .band {
    position: absolute; top: 0; height: 26px; background: color-mix(in srgb, var(--c) 22%, transparent);
    border-left: 3px solid var(--c); border-radius: 3px; overflow: hidden; display: flex; align-items: center; padding-left: 6px; font-size: 12px;
  }
  .band span { white-space: nowrap; }
  .began { position: absolute; top: -4px; height: 34px; border-left: 2px dashed var(--c); }
  .pad { padding: 4px 0; }
  .lanes { position: relative; margin-top: 4px; }
  .chip {
    position: absolute; width: 146px; height: 28px; padding: 2px 6px; text-align: left; font-size: 12px; cursor: grab; touch-action: none;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; border-left: 4px solid #3399ff; background: var(--panel-2);
  }
  .chip.warning { border-left-color: var(--warn); }
  .chip.critical { border-left-color: var(--danger); }
  .chip.fired { opacity: 0.5; cursor: default; }
  .chip.disarmed { border-style: dashed; opacity: 0.6; }
  .chip .mono { color: var(--muted); font-size: 10px; }
  .now { position: absolute; top: -60px; bottom: 0; border-left: 2px solid var(--accent); pointer-events: none; }
  section { display: grid; gap: 4px; }
  .unplaced { justify-content: flex-start; gap: 10px; text-align: left; font-size: 13px; border-left: 3px solid #3399ff; }
  .unplaced.warning { border-left-color: var(--warn); }
  .unplaced.critical { border-left-color: var(--danger); }
</style>
