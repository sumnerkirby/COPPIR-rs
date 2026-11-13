<script lang="ts">
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Visibility } from '../../lib/types/Visibility';
  import VisibilityPicker from '../ui/VisibilityPicker.svelte';

  let { ondone }: { ondone: () => void } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  let question = $state('');
  let options = $state(['', '']);
  let allowText = $state(false);
  let audience = $state<Visibility>({ mode: 'all' });
  let timed = $state(false);
  let minutes = $state(5);

  async function save(e: SubmitEvent) {
    e.preventDefault();
    const r = await ws.send({
      type: 'add_poll',
      poll: {
        question,
        options: options.map((o) => o.trim()).filter(Boolean),
        allow_text: allowText,
        audience,
        duration_minutes: timed ? minutes : undefined,
      },
    });
    if (r) ondone();
  }
</script>

<form onsubmit={save}>
  <h2>New poll</h2>
  <!-- svelte-ignore a11y_autofocus -->
  <label>Question <input bind:value={question} required maxlength="500" autofocus placeholder="e.g. Do you pay the ransom?" /></label>
  <fieldset>
    <legend>Answers</legend>
    {#each options as _, i (i)}
      <div class="row">
        <input class="grow" bind:value={options[i]} maxlength="200" placeholder={`Option ${i + 1}`} aria-label={`Option ${i + 1}`} />
        <button type="button" class="ghost small" onclick={() => options.splice(i, 1)} aria-label="Remove option">✕</button>
      </div>
    {/each}
    <button type="button" class="small" onclick={() => options.push('')} disabled={options.length >= 12}>+ Option</button>
    <label class="check"><input type="checkbox" bind:checked={allowText} /> Players can also write an answer</label>
  </fieldset>
  <VisibilityPicker label="Put to" value={audience} teams={ex.teams} onchange={(v) => (audience = v)} />
  <label class="check"><input type="checkbox" bind:checked={timed} /> Close by itself after
    <input type="number" min="1" max="1440" bind:value={minutes} disabled={!timed} class="mins" aria-label="Minutes" /> minutes</label>
  <p class="empty">Polls start as drafts. Open one yourself, or from an inject.</p>
  <div class="row end">
    <button type="button" onclick={ondone}>Cancel</button>
    <button class="primary" disabled={!question.trim()}>Add poll</button>
  </div>
</form>

<style>
  form { display: grid; gap: 10px; }
  h2 { font-size: 16px; }
  fieldset { border: 1px solid var(--line); border-radius: var(--radius); padding: 8px; display: grid; gap: 6px; margin: 0; }
  legend { font-size: 12px; color: var(--muted); padding: 0 4px; }
  .small { font-size: 12px; justify-self: start; }
  .check { flex-direction: row; display: flex; align-items: center; gap: 8px; color: var(--text); font-size: 13px; }
  .mins { width: 70px; }
  .end { justify-content: flex-end; }
</style>
