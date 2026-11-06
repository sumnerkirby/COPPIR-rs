<script lang="ts">
  import { breached, counts, sectorHealth } from '../lib/sitrep';
  import type { Exercise } from '../lib/types/Exercise';
  import MetricWheel from './MetricWheel.svelte';

  let { exercise }: { exercise: Exercise } = $props();

  const sets = $derived(exercise.taxonomy.status_sets.filter((s) => s.display !== 'detail'));
  const health = $derived(sectorHealth(exercise));
  const alerts = $derived(breached(exercise, health));
  const alertedSectors = $derived(new Set(alerts.map((t) => t.sector)));
  const total = $derived(Object.keys(exercise.assets).length);
</script>

<section class="sitrep" aria-label="Situation report">
  <div class="group">
    <span class="big">{total}</span>
    <span class="muted label">assets</span>
  </div>
  {#each sets as set (set.id)}
    <div class="group" role="group" aria-label={set.name}>
      {#each counts(exercise, set) as { value, count } (value.id)}
        <span class="count" style:--c={value.color} title={value.label}>
          <b>{count}</b><small>{value.label}</small>
        </span>
      {/each}
    </div>
  {/each}
  <div class="group wheels" role="group" aria-label="Sector integrity">
    {#each health as s (s.id)}
      <MetricWheel value={s.score} label={s.name} alert={alertedSectors.has(s.id)} />
    {/each}
  </div>
  {#if exercise.metrics.length}
    <div class="group wheels" role="group" aria-label="Metrics">
      {#each exercise.metrics as m (m.id)}<MetricWheel value={m.value} label={m.name} />{/each}
    </div>
  {/if}
  {#if alerts.length}
    <div class="alerts" role="alert">
      {#each alerts as t (t.id)}<span class="alert {t.severity}">⚠ {t.name}</span>{/each}
    </div>
  {/if}
</section>

<style>
  .sitrep {
    display: flex; flex-wrap: wrap; gap: 8px 18px; align-items: center; padding: 6px 14px;
    background: var(--panel-2); border-bottom: 1px solid var(--line);
  }
  .group { display: flex; gap: 10px; align-items: center; }
  .wheels { padding-left: 14px; border-left: 1px solid var(--line); }
  .big { font-size: 22px; font-weight: 700; font-family: var(--mono); }
  .label { font-size: 11px; text-transform: uppercase; }
  .count { display: grid; text-align: center; border-top: 3px solid var(--c); padding-top: 2px; min-width: 50px; }
  .count b { font-family: var(--mono); font-size: 16px; }
  .count small { font-size: 9px; color: var(--muted); text-transform: uppercase; white-space: nowrap; }
  .alerts { display: flex; flex-wrap: wrap; gap: 6px; margin-left: auto; }
  .alert { padding: 3px 8px; border-radius: var(--radius); background: #3a2a00; color: var(--warn); font-size: 12px; }
  .alert.critical { background: #3a0d0d; color: var(--danger); }
  .alert.info { background: #0c1f2e; color: #66b3ff; }
</style>
