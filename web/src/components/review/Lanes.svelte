<script lang="ts">
  // One lane per actor (the facilitator, the script, each team, the assets)
  // against the exercise clock, phases as bands above. Click or drag anywhere
  // to move the replay; click a mark to jump to it.
  import { fmtClock, fmtMinutes } from '../../lib/clock.svelte';
  import type { Mark, Replay } from '../../lib/replay';

  let { replay, t, onseek }: { replay: Replay; t: number; onseek: (t: number) => void } = $props();

  let width = $state(800);
  let track: HTMLDivElement;
  const span = $derived(replay.end);
  const x = (ms: number) => (ms / span) * width;
  const byLane = $derived.by(() => {
    const out = new Map<string, Mark[]>();
    for (const m of replay.marks) out.set(m.lane, [...(out.get(m.lane) ?? []), m]);
    return out;
  });

  const minutes = $derived(span / 60000);
  const step = $derived([1, 2, 5, 10, 15, 30, 60, 120].find((s) => (s / minutes) * width >= 70) ?? 240);
  const ticks = $derived(Array.from({ length: Math.floor(minutes / step) + 1 }, (_, n) => n * step));

  const bands = $derived(
    replay.phases.map((p, i) => ({ ...p, end: replay.phases[i + 1]?.t ?? span })),
  );

  function at(clientX: number) {
    const r = track.getBoundingClientRect();
    return ((clientX - r.left) / r.width) * span;
  }

  function down(e: PointerEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('.mark')) return;
    onseek(at(e.clientX));
    const move = (ev: PointerEvent) => onseek(at(ev.clientX));
    const up = () => {
      removeEventListener('pointermove', move);
      removeEventListener('pointerup', up);
    };
    addEventListener('pointermove', move);
    addEventListener('pointerup', up);
    e.preventDefault();
  }

  function onkeydown(e: KeyboardEvent) {
    const by = e.shiftKey ? 600_000 : 60_000;
    if (e.key === 'ArrowRight') onseek(t + by);
    else if (e.key === 'ArrowLeft') onseek(t - by);
    else if (e.key === 'Home') onseek(0);
    else if (e.key === 'End') onseek(span);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  const title = (m: Mark) => `T+${fmtClock(m.t)} ${m.label}`;
</script>

<section class="lanes" aria-label="Timeline by team">
  <div class="names">
    <span class="axis-pad"></span>
    <span class="axis-pad phase-pad"></span>
    {#each replay.lanes as lane (lane.id)}<span class="name" style:--c={lane.color} title={lane.name}>{lane.name}</span>{/each}
  </div>
  <div
    class="track"
    bind:this={track}
    bind:clientWidth={width}
    onpointerdown={down}
    {onkeydown}
    role="slider"
    tabindex="0"
    aria-label="Replay position"
    aria-valuemin={0}
    aria-valuemax={Math.round(span / 1000)}
    aria-valuenow={Math.round(t / 1000)}
    aria-valuetext="T+{fmtClock(t)}"
  >
    <div class="axis">
      {#each ticks as m (m)}<span class="tick mono" style:left="{x(m * 60000)}px">{m ? fmtMinutes(m) : 'T+0'}</span>{/each}
    </div>
    <div class="phases">
      {#each bands as b, i (i)}
        <div class="band" style:left="{x(b.t)}px" style:width="{Math.max(2, x(b.end) - x(b.t))}px" style:--c={b.color}><span>{b.name}</span></div>
      {/each}
    </div>
    {#each replay.lanes as lane (lane.id)}
      <div class="lane">
        {#each byLane.get(lane.id) ?? [] as m, i (i)}
          <button
            class="mark {m.kind}"
            class:past={m.t <= t}
            style:left="{x(m.t)}px"
            style:--c={m.color ?? lane.color}
            title={title(m)}
            aria-label={title(m)}
            tabindex="-1"
            onclick={() => onseek(m.t)}
          ></button>
        {/each}
      </div>
    {/each}
    <div class="cursor" style:left="{x(t)}px"><span class="mono">T+{fmtClock(t)}</span></div>
  </div>
</section>

<style>
  .lanes {
    display: grid; grid-template-columns: 110px minmax(0, 1fr); background: var(--panel); border-top: 1px solid var(--line);
    max-height: 34vh; overflow-y: auto; padding: 4px 16px 8px 0; user-select: none;
  }
  .names { display: grid; grid-auto-rows: 22px; }
  .names .axis-pad { height: 18px; }
  .names .phase-pad { height: 18px; }
  .name {
    font-size: 12px; padding: 0 8px 0 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; line-height: 22px;
    border-left: 3px solid var(--c);
  }
  .track { position: relative; display: grid; grid-auto-rows: 22px; cursor: col-resize; outline: none; }
  .track:focus-visible { box-shadow: 0 0 0 2px var(--accent); border-radius: 4px; }
  .axis { position: relative; height: 18px; border-bottom: 1px solid var(--line); }
  .tick { position: absolute; font-size: 10px; color: var(--muted); white-space: nowrap; transform: translateX(-2px); }
  .phases { position: relative; height: 18px; }
  .band {
    position: absolute; top: 2px; height: 14px; background: color-mix(in srgb, var(--c) 25%, transparent);
    border-left: 2px solid var(--c); font-size: 10px; line-height: 14px; padding-left: 4px; overflow: hidden; white-space: nowrap;
  }
  .lane { position: relative; border-bottom: 1px dashed color-mix(in srgb, var(--line) 60%, transparent); }
  .mark {
    position: absolute; top: 6px; width: 10px; height: 10px; padding: 0; margin-left: -5px; border-radius: 50%;
    background: var(--c); border: 1px solid rgb(0 0 0 / 0.4); opacity: 0.35; cursor: pointer;
  }
  .mark.past { opacity: 1; }
  .mark.inject { border-radius: 2px; transform: rotate(45deg); }
  .mark.vote { border-radius: 2px; }
  .mark.poll { background: transparent; border: 2px solid var(--c); }
  .mark.undo { background: var(--muted); width: 6px; margin-left: -3px; border-radius: 1px; }
  .mark:hover { transform: scale(1.5); opacity: 1; z-index: 2; }
  .mark.inject:hover { transform: rotate(45deg) scale(1.5); }
  .cursor { position: absolute; top: 0; bottom: 0; border-left: 2px solid var(--accent); pointer-events: none; }
  .cursor span {
    position: absolute; top: 0; left: 3px; font-size: 10px; background: var(--accent); color: var(--bg, #000);
    padding: 0 3px; border-radius: 2px; white-space: nowrap;
  }
</style>
