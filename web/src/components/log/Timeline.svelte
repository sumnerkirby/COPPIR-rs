<script lang="ts">
  // The log laid out in time, so gaps and bursts of activity show at a glance.
  import type { LogEntry } from '../../lib/types/LogEntry';

  let { entries }: { entries: LogEntry[] } = $props();
  const sorted = $derived([...entries].sort((a, b) => a.at.localeCompare(b.at)));
  const t0 = $derived(new Date(sorted[0]?.at ?? 0).getTime());
  const t1 = $derived(new Date(sorted.at(-1)?.at ?? 0).getTime());
  const span = $derived(Math.max(t1 - t0, 1));
  let hover = $state<LogEntry | null>(null);

  const pos = (e: LogEntry) => ((new Date(e.at).getTime() - t0) / span) * 100;
  const fmt = (t: number) => new Date(t).toLocaleTimeString();
</script>

<div class="timeline">
  <div class="track">
    {#each sorted as e (e.id)}
      <button
        class="dot"
        class:auto={e.auto}
        style:top="{pos(e)}%"
        aria-label="{fmt(new Date(e.at).getTime())}: {e.text}"
        onmouseenter={() => (hover = e)}
        onfocus={() => (hover = e)}
      ></button>
    {/each}
  </div>
  <div class="axis mono muted"><span>{fmt(t0)}</span><span>{fmt(t1)}</span></div>
  <div class="detail">
    {#if hover}
      <p class="mono muted">{new Date(hover.at).toLocaleString()}</p>
      <p>{hover.text}</p>
      {#if hover.asset_name}<p class="muted">{hover.asset_name}</p>{/if}
      {#if hover.notes}<p class="muted">{hover.notes}</p>{/if}
    {:else}
      <p class="muted">Point at an entry. Amber entries were written by injects.</p>
    {/if}
  </div>
</div>

<style>
  .timeline { display: grid; grid-template-columns: 28px auto 1fr; gap: 10px; height: 360px; }
  .track { position: relative; border-left: 2px solid var(--line); margin-left: 12px; }
  .dot { position: absolute; left: -8px; width: 14px; height: 14px; padding: 0; border-radius: 50%; background: var(--accent); border: 2px solid var(--bg); transform: translateY(-50%); }
  .dot.auto { background: var(--warn); }
  .axis { display: flex; flex-direction: column; justify-content: space-between; font-size: 11px; }
  .detail p { margin: 0 0 4px; font-size: 13px; }
</style>
