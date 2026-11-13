<script lang="ts">
  // The hot wash: what went well, what to fix, in the facilitator's words.
  // Saved when a box loses focus; printed at the top of the report.
  import { untrack } from 'svelte';
  import type { Workspace } from '../../lib/workspace.svelte';
  import type { Review } from '../../lib/types/Review';

  let { ws }: { ws: Workspace } = $props();
  const empty: Review = { summary: '', strengths: '', improvements: '' };
  let draft = $state<Review>(untrack(() => ({ ...(ws.exercise?.review ?? empty) })));
  let saved = $state(false);

  // Someone else's change (an undo) lands here unless this box is mid-edit.
  let editing = $state(false);
  $effect(() => {
    const r = ws.exercise?.review;
    if (r && !untrack(() => editing)) draft = { ...r };
  });

  async function save() {
    editing = false;
    const now = ws.exercise?.review;
    if (!now || (draft.summary === now.summary && draft.strengths === now.strengths && draft.improvements === now.improvements)) return;
    if (await ws.send({ type: 'set_review', review: { ...draft } })) {
      saved = true;
      setTimeout(() => (saved = false), 1500);
    }
  }

  const boxes = [
    { key: 'summary', label: 'Summary', hint: 'What the exercise tested, and how it went overall' },
    { key: 'strengths', label: 'Strengths', hint: 'What worked: one point per line' },
    { key: 'improvements', label: 'Areas for improvement', hint: 'What to fix, who owns it, by when: one per line' },
  ] as const;
</script>

<div class="wash">
  <p class="muted intro">Notes from the debrief. They open the after-action report. {#if saved}<span class="ok">Saved</span>{/if}</p>
  {#each boxes as b (b.key)}
    <label>
      {b.label}
      <textarea rows="5" maxlength="20000" placeholder={b.hint} bind:value={draft[b.key]} onfocus={() => (editing = true)} onblur={save}></textarea>
    </label>
  {/each}
</div>

<style>
  .wash { display: grid; gap: 12px; }
  .intro { margin: 0; font-size: 13px; }
  .ok { color: var(--accent); margin-left: 6px; }
  textarea { resize: vertical; }
</style>
