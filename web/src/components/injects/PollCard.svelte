<script lang="ts">
  import { ticker } from '../../lib/clock.svelte';
  import { describeVisibility } from '../../lib/script';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Poll } from '../../lib/types/Poll';
  import Icon from '../ui/Icon.svelte';

  let { poll }: { poll: Poll } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const total = $derived(Object.values(poll.results).reduce((a, b) => a + b, 0));
  const texts = $derived(poll.responses.filter((r) => r.text));
  const left = $derived(poll.closes_at ? Math.max(0, Date.parse(poll.closes_at) - ticker.now) : null);
  const teamName = (id: string | null) => ex.teams.find((t) => t.id === id)?.name;

  async function remove() {
    if (await ui.confirm(`Delete the poll “${poll.question}”?`, { body: 'Its answers go with it.', confirm: 'Delete', danger: true })) {
      await ws.send({ type: 'remove_poll', id: poll.id });
    }
  }
</script>

<article class="poll {poll.state}">
  <div class="row">
    <strong class="grow">{poll.question}</strong>
    <span class="badge">{poll.state}</span>
  </div>
  <p class="muted small">
    To {describeVisibility(ex, poll.audience)} · {poll.responses.length} answer{poll.responses.length === 1 ? '' : 's'}
    {#if poll.state === 'open' && left !== null} · closes in {Math.ceil(left / 60000)} min{/if}
  </p>
  {#each poll.options as o (o.id)}
    {@const n = poll.results[o.id] ?? 0}
    <div class="bar" title="{n} vote{n === 1 ? '' : 's'}">
      <span class="fill" style:width="{total ? (n / total) * 100 : 0}%"></span>
      <span class="label">{o.label}</span><span class="n mono">{n}</span>
    </div>
  {/each}
  {#if texts.length}
    <details>
      <summary class="small">Written answers ({texts.length})</summary>
      {#each texts as r (r.participant)}
        <p class="small"><strong>{r.name}</strong>{#if teamName(r.team)} <span class="muted">({teamName(r.team)})</span>{/if}: {r.text}</p>
      {/each}
    </details>
  {/if}
  <div class="row">
    {#if poll.state === 'open'}
      <button class="small" onclick={() => ws.send({ type: 'close_poll', id: poll.id })}>Close</button>
    {:else}
      <button class="small primary" onclick={() => ws.send({ type: 'open_poll', id: poll.id })}>{poll.state === 'closed' ? 'Reopen' : 'Open'}</button>
    {/if}
    <span class="grow"></span>
    <button class="ghost small" onclick={remove} aria-label="Delete poll"><Icon name="trash-2" size={14} /></button>
  </div>
</article>

<style>
  .poll { border: 1px solid var(--line); border-radius: var(--radius); padding: 8px 10px; display: grid; gap: 5px; background: var(--panel-2); }
  .poll.open { border-color: var(--accent); }
  .badge { font-size: 10px; text-transform: uppercase; border: 1px solid var(--line); border-radius: 3px; padding: 0 4px; }
  .open .badge { color: var(--accent); border-color: var(--accent); }
  .small { font-size: 12px; margin: 0; }
  .bar { position: relative; display: flex; gap: 8px; padding: 3px 6px; border-radius: 3px; background: var(--bg); overflow: hidden; font-size: 13px; }
  .fill { position: absolute; inset: 0 auto 0 0; background: rgba(0, 255, 65, 0.18); }
  .label { position: relative; flex: 1; }
  .n { position: relative; }
  button.small { padding: 3px 8px; }
</style>
