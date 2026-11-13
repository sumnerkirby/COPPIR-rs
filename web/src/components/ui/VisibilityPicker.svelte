<script lang="ts">
  // Who can see something: everyone, chosen teams, or only the facilitator.
  import type { Team } from '../../lib/types/Team';
  import type { Visibility } from '../../lib/types/Visibility';

  let { label, value, teams, onchange }: { label: string; value: Visibility; teams: Team[]; onchange: (v: Visibility) => void } = $props();

  const chosen = $derived(value.mode === 'teams' ? value.teams : []);

  function setMode(mode: string) {
    if (mode === 'all') onchange({ mode: 'all' });
    else if (mode === 'facilitator') onchange({ mode: 'facilitator' });
    else onchange({ mode: 'teams', teams: chosen.length ? chosen : teams.slice(0, 1).map((t) => t.id) });
  }

  function toggle(id: string, on: boolean) {
    const next = on ? [...chosen, id] : chosen.filter((t) => t !== id);
    // No teams left would mean nobody: fall back to facilitator only.
    onchange(next.length ? { mode: 'teams', teams: next } : { mode: 'facilitator' });
  }
</script>

<div class="vis">
  <label>{label}
    <select value={value.mode} onchange={(e) => setMode(e.currentTarget.value)}>
      <option value="all">Everyone</option>
      {#if teams.length}<option value="teams">Chosen teams</option>{/if}
      <option value="facilitator">Facilitator only</option>
    </select>
  </label>
  {#if value.mode === 'teams'}
    <div class="teams">
      {#each teams as t (t.id)}
        <label class="check" style:--c={t.color}>
          <input type="checkbox" checked={chosen.includes(t.id)} onchange={(e) => toggle(t.id, e.currentTarget.checked)} />
          {t.name}
        </label>
      {/each}
    </div>
  {/if}
</div>

<style>
  .vis { display: grid; gap: 6px; }
  .teams { display: flex; flex-wrap: wrap; gap: 4px 12px; }
  .check { display: flex; flex-direction: row; align-items: center; gap: 6px; color: var(--text); font-size: 13px; border-left: 3px solid var(--c); padding-left: 6px; }
</style>
