<script lang="ts">
  import type { Applied } from '../lib/types/Applied';
  import type { Command } from '../lib/types/Command';
  import type { Exercise } from '../lib/types/Exercise';

  let { exercise, send }: { exercise: Exercise; send: (c: Command) => Promise<Applied | null> } = $props();

  let text = $state('');
  const recent = $derived([...exercise.log].reverse().slice(0, 100));

  async function log(e: SubmitEvent) {
    e.preventDefault();
    if (await send({ type: 'log', text })) text = '';
  }
</script>

<section>
  <h2>Decision log</h2>
  <form onsubmit={log}>
    <input bind:value={text} placeholder="Record a decision…" aria-label="Log entry" maxlength="500" />
    <button disabled={!text.trim()}>Log</button>
  </form>
  {#if recent.length === 0}
    <p class="muted">Nothing logged yet.</p>
  {:else}
    <ol>
      {#each recent as e (e.id)}
        <li class:auto={e.auto}>
          <span class="mono muted">{new Date(e.at).toLocaleTimeString()}</span>
          <span>{e.text}</span>
          {#if e.asset_name}<span class="muted">· {e.asset_name}</span>{/if}
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  section { display: grid; gap: 8px; border-top: 1px solid var(--line); padding-top: 14px; }
  h2 { font-size: 12px; text-transform: uppercase; letter-spacing: 0.1em; color: var(--muted); }
  form { display: flex; gap: 6px; }
  ol { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; font-size: 13px; }
  li { display: flex; flex-wrap: wrap; gap: 6px; }
  li.auto span:nth-child(2) { color: var(--warn); }
  p { margin: 0; }
</style>
