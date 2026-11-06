<script lang="ts">
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Severity } from '../../lib/types/Severity';
  import Icon from '../ui/Icon.svelte';
  import MetricWheel from '../MetricWheel.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  // Thresholds
  let tName = $state('');
  let tSector = $state('');
  let tPct = $state(50);
  let tSeverity = $state<Severity>('warning');
  async function addThreshold(e: SubmitEvent) {
    e.preventDefault();
    const r = await ws.send({ type: 'add_threshold', name: tName, sector: tSector || undefined, below_pct: tPct, severity: tSeverity });
    if (r) tName = '';
  }
  const sectorName = (id: string | null) => (id ? (ex.taxonomy.sectors.find((s) => s.id === id)?.name ?? id) : 'All sectors');

  // Metrics
  let mName = $state('');
  async function addMetric(e: SubmitEvent) {
    e.preventDefault();
    if (await ws.send({ type: 'add_metric', name: mName, value: 100 })) mName = '';
  }
  const nudge = (id: string, v: number) => ws.send({ type: 'edit_metric', id, value: Math.max(0, Math.min(100, v)) });
  async function setExact(id: string, current: number) {
    const v = await ui.prompt('Set metric', 'Value (0–100)', String(current), 'Set');
    const n = Number(v);
    if (v !== null && Number.isFinite(n)) await nudge(id, Math.round(n));
  }
  async function renameMetric(id: string, name: string) {
    const n = await ui.prompt('Rename metric', 'Name', name, 'Rename');
    if (n) await ws.send({ type: 'edit_metric', id, name: n });
  }
</script>

<section>
  <h3 class="section-title">Exercise</h3>
  <label>Description
    <textarea rows="3" value={ex.description} maxlength="4000" onchange={(e) => ws.send({ type: 'rename', name: ex.name, description: e.currentTarget.value })}></textarea>
  </label>
  <button class="icon" onclick={() => (ws.showTaxonomy = true)}><Icon name="tags" size={14} /> Edit statuses, asset types and sectors…</button>
</section>

<section>
  <h3 class="section-title">Metrics</h3>
  <p class="empty">Percentages you track by hand (comms readiness, staff available…). Everyone sees the same values.</p>
  {#each ex.metrics as m (m.id)}
    <div class="metric">
      <button class="ghost wheel" onclick={() => setExact(m.id, m.value)} aria-label={`Set ${m.name}`}><MetricWheel value={m.value} label="" size={44} /></button>
      <button class="ghost grow name" onclick={() => renameMetric(m.id, m.name)}>{m.name}</button>
      <button class="small" onclick={() => nudge(m.id, m.value - 5)} aria-label={`Lower ${m.name}`}>−5</button>
      <button class="small" onclick={() => nudge(m.id, m.value + 5)} aria-label={`Raise ${m.name}`}>+5</button>
      <button class="ghost small" onclick={() => ws.send({ type: 'remove_metric', id: m.id })} aria-label={`Delete ${m.name}`}>✕</button>
    </div>
  {/each}
  <form class="row" onsubmit={addMetric}>
    <input class="grow" bind:value={mName} placeholder="New metric name" aria-label="New metric name" maxlength="100" />
    <button disabled={!mName.trim()}>Add</button>
  </form>
</section>

<section>
  <h3 class="section-title">Alert thresholds</h3>
  {#each ex.thresholds as t (t.id)}
    <div class="row threshold {t.severity}">
      <span class="grow"><strong>{t.name}</strong> <span class="muted">· {sectorName(t.sector)} below {t.below_pct}%</span></span>
      <button class="ghost small" onclick={() => ws.send({ type: 'remove_threshold', id: t.id })} aria-label={`Delete ${t.name}`}>✕</button>
    </div>
  {:else}
    <p class="empty">No thresholds. Add one to flag a sector when its integrity drops.</p>
  {/each}
  <form onsubmit={addThreshold}>
    <input bind:value={tName} placeholder="Name, e.g. Grid integrity critical" aria-label="Threshold name" maxlength="200" />
    <div class="row">
      <select bind:value={tSector} aria-label="Sector" class="grow">
        <option value="">All sectors</option>
        {#each ex.taxonomy.sectors as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
      </select>
      <label class="pct">Below %<input type="number" min="0" max="100" bind:value={tPct} /></label>
    </div>
    <div class="row">
      <select bind:value={tSeverity} aria-label="Severity" class="grow">
        <option value="info">Info</option><option value="warning">Warning</option><option value="critical">Critical</option>
      </select>
      <button disabled={!tName.trim()}>Add threshold</button>
    </div>
  </form>
</section>

<style>
  section { display: grid; gap: 8px; }
  section + section { border-top: 1px solid var(--line); padding-top: 14px; }
  form { display: grid; gap: 6px; }
  .metric { display: flex; align-items: center; gap: 6px; }
  .wheel { padding: 0; }
  .name { text-align: left; }
  .small { font-size: 12px; padding: 3px 7px; }
  .pct { width: 90px; }
  .threshold { border-left: 3px solid #3399ff; padding-left: 8px; font-size: 13px; }
  .threshold.warning { border-left-color: var(--warn); }
  .threshold.critical { border-left-color: var(--danger); }
</style>
