<script lang="ts">
  // Plan the stages of the exercise. Edits are a draft until saved, because
  // the server takes the whole list at once.
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Phase } from '../../lib/types/Phase';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  let open = $state(false);
  let draft = $state<Phase[]>([]);
  let dirty = $state(false);

  // Follow the server's list until the facilitator starts editing.
  $effect(() => {
    const phases = ex.phases;
    if (!dirty) draft = structuredClone($state.snapshot(phases));
  });

  const COLORS = ['#3399ff', '#ffaa00', '#ff6633', '#33cc99', '#aa88ff', '#cccc33'];
  const SUGGESTED = ['Detection', 'Containment', 'Eradication', 'Recovery'];

  function add(name = '') {
    draft.push({ id: '', name, color: COLORS[draft.length % COLORS.length]!, target_minutes: 30 });
    dirty = true;
  }
  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= draft.length) return;
    [draft[i], draft[j]] = [draft[j]!, draft[i]!];
    dirty = true;
  }
  async function save() {
    if (await ws.send({ type: 'set_phases', phases: draft })) dirty = false;
  }
  function cancel() {
    dirty = false;
    draft = structuredClone($state.snapshot(ex.phases));
  }
  const total = $derived(draft.reduce((t, p) => t + (Number(p.target_minutes) || 0), 0));
</script>

<details bind:open>
  <summary class="section-title">Phases ({ex.phases.length}){#if total} · {total} min planned{/if}</summary>
  <div class="phases" oninput={() => (dirty = true)}>
    {#each draft as p, i (i)}
      <div class="row">
        <input type="color" bind:value={p.color} aria-label="Colour" />
        <input class="grow" bind:value={p.name} maxlength="100" placeholder="Phase name" aria-label="Phase name" />
        <input class="mins" type="number" min="0" max="10000" bind:value={p.target_minutes} aria-label="Planned minutes" title="Planned minutes" />
        <button class="ghost small" onclick={() => move(i, -1)} aria-label="Move up">↑</button>
        <button class="ghost small" onclick={() => { draft.splice(i, 1); dirty = true; }} aria-label="Remove">✕</button>
      </div>
    {/each}
    {#if !draft.length}
      <p class="empty">No phases. Phases structure the clock and the timeline, and injects can fire when one begins.</p>
      <button class="small" onclick={() => { for (const n of SUGGESTED) add(n); }}>Use {SUGGESTED.join(', ')}</button>
    {/if}
    <div class="row">
      <button class="small" onclick={() => add()}>+ Phase</button>
      <span class="grow"></span>
      {#if dirty}
        <button class="small" onclick={cancel}>Cancel</button>
        <button class="small primary" onclick={save}>Save phases</button>
      {/if}
    </div>
  </div>
</details>

<style>
  summary { cursor: pointer; }
  .phases { display: grid; gap: 6px; margin-top: 8px; }
  .mins { width: 70px; }
  .small { font-size: 12px; padding: 3px 8px; }
</style>
