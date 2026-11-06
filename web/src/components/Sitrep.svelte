<script lang="ts">
  import { breached, counts, scoreColor, sectorHealth } from '../lib/sitrep';
  import type { Exercise } from '../lib/types/Exercise';

  let { exercise }: { exercise: Exercise } = $props();

  const sets = $derived(exercise.taxonomy.status_sets.filter((s) => s.display !== 'detail'));
  const health = $derived(sectorHealth(exercise));
  const alerts = $derived(breached(exercise, health));
  const total = $derived(Object.keys(exercise.assets).length);

  // Wheel geometry: r = 16, so the circumference is 2πr.
  const C = 2 * Math.PI * 16;
</script>

<section class="sitrep" aria-label="Situation report">
  <div class="group">
    <span class="big">{total}</span>
    <span class="muted label">assets</span>
  </div>
  {#each sets as set (set.id)}
    <div class="group" aria-label={set.name}>
      {#each counts(exercise, set) as { value, count } (value.id)}
        <span class="count" style:--c={value.color} title={value.label}>
          <b>{count}</b><small>{value.label}</small>
        </span>
      {/each}
    </div>
  {/each}
  <div class="group wheels">
    {#each health as s (s.id)}
      <figure title="{s.name}: {s.score === null ? 'no assets' : `${Math.round(s.score)}% (${s.assets} assets)`}">
        <svg viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
          <circle cx="20" cy="20" r="16" class="track" />
          <circle
            cx="20" cy="20" r="16" class="arc"
            stroke={scoreColor(s.score)}
            stroke-dasharray="{((s.score ?? 0) / 100) * C} {C}"
          />
          <text x="20" y="24">{s.score === null ? '–' : Math.round(s.score)}</text>
        </svg>
        <figcaption>{s.name}</figcaption>
      </figure>
    {/each}
  </div>
  {#if alerts.length}
    <div class="alerts" role="alert">
      {#each alerts as t (t.id)}<span class="alert {t.severity}">⚠ {t.name}</span>{/each}
    </div>
  {/if}
</section>

<style>
  .sitrep {
    display: flex; flex-wrap: wrap; gap: 18px; align-items: center; padding: 8px 14px;
    background: var(--panel-2); border-bottom: 1px solid var(--line);
  }
  .group { display: flex; gap: 10px; align-items: center; }
  .big { font-size: 22px; font-weight: 700; font-family: var(--mono); }
  .label { font-size: 11px; text-transform: uppercase; }
  .count { display: grid; text-align: center; border-top: 3px solid var(--c); padding-top: 2px; min-width: 52px; }
  .count b { font-family: var(--mono); font-size: 16px; }
  .count small { font-size: 9px; color: var(--muted); text-transform: uppercase; white-space: nowrap; }
  figure { margin: 0; display: grid; justify-items: center; }
  figcaption { font-size: 9px; color: var(--muted); text-transform: uppercase; }
  .track { fill: none; stroke: var(--line); stroke-width: 4; }
  .arc { fill: none; stroke-width: 4; transform: rotate(-90deg); transform-origin: center; }
  text { fill: var(--text); font: 600 11px var(--mono); text-anchor: middle; }
  .alerts { display: flex; gap: 8px; margin-left: auto; }
  .alert { padding: 3px 8px; border-radius: var(--radius); background: #3a2a00; color: var(--warn); font-size: 12px; }
  .alert.critical { background: #3a0d0d; color: var(--danger); }
</style>
