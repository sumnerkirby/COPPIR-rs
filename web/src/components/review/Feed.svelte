<script lang="ts">
  // Everything on the lanes up to the replay's moment, newest first.
  import { fmtClock } from '../../lib/clock.svelte';
  import type { Replay } from '../../lib/replay';

  let { replay, t, onseek }: { replay: Replay; t: number; onseek: (t: number) => void } = $props();
  const lanes = $derived(new Map(replay.lanes.map((l) => [l.id, l])));
  const shown = $derived(replay.marks.filter((m) => m.t <= t).reverse().slice(0, 300));
</script>

{#if shown.length}
  <ol>
    {#each shown as m, i (i)}
      {@const lane = lanes.get(m.lane)}
      <li>
        <button class="ghost row" onclick={() => onseek(m.t)} style:--c={m.color ?? lane?.color}>
          <span class="mono when">T+{fmtClock(m.t)}</span>
          <span class="who">{lane?.name}</span>
          <span class="what">{m.label}</span>
        </button>
      </li>
    {/each}
  </ol>
{:else}
  <p class="empty">Nothing has happened yet at this point. Press play, or drag along the timeline.</p>
{/if}

<style>
  ol { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .row { display: grid; grid-template-columns: auto 1fr; gap: 0 8px; text-align: left; width: 100%; border-left: 3px solid var(--c); padding: 6px 8px; }
  .when { font-size: 11px; color: var(--muted); }
  .who { font-size: 11px; color: var(--muted); text-align: right; }
  .what { grid-column: 1 / -1; font-size: 13px; overflow-wrap: anywhere; }
</style>
