<script lang="ts">
  // A poll as a player sees it: answer while it is open (and change your
  // mind), see the team's answers, and the totals once it closes.
  import { ticker } from '../../lib/clock.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Me } from '../../lib/types/Me';
  import type { Poll } from '../../lib/types/Poll';

  let { poll, me }: { poll: Poll; me: Me } = $props();
  const ws = useWorkspace();
  // Our own answer, if the server has it: the latest one from this name in our team.
  const mine = $derived([...poll.responses].reverse().find((r) => r.name === me.name && r.team === me.team));
  let text = $state('');
  let sending = $state(false);
  const total = $derived(Object.values(poll.results).reduce((a, b) => a + b, 0));
  const left = $derived(poll.closes_at ? Math.max(0, Date.parse(poll.closes_at) - ticker.now) : null);

  async function answer(option?: string) {
    sending = true;
    await ws.send({ type: 'respond', poll: poll.id, option, text: text.trim() || undefined });
    sending = false;
  }
</script>

<article class="poll" class:open={poll.state === 'open'}>
  <div class="row">
    <strong class="grow">{poll.question}</strong>
    {#if poll.state === 'open' && left !== null}<span class="mono muted">{Math.ceil(left / 60000)} min left</span>{/if}
    {#if poll.state === 'closed'}<span class="muted">closed</span>{/if}
  </div>
  {#if poll.state === 'open'}
    <div class="options">
      {#each poll.options as o (o.id)}
        <button class:chosen={mine?.option === o.id} disabled={sending} onclick={() => answer(o.id)} aria-pressed={mine?.option === o.id}>{o.label}</button>
      {/each}
    </div>
    {#if poll.allow_text}
      <form class="row" onsubmit={(e) => { e.preventDefault(); answer(mine?.option ?? undefined); }}>
        <input class="grow" bind:value={text} maxlength="1000" placeholder={mine?.text || 'Or write your answer…'} aria-label="Your answer" />
        <button disabled={sending || !text.trim()}>Send</button>
      </form>
    {/if}
    {#if mine}<p class="muted small">Your answer is in. You can change it until the poll closes.</p>{/if}
  {:else}
    {#each poll.options as o (o.id)}
      {@const n = poll.results[o.id] ?? 0}
      <div class="bar"><span class="fill" style:width="{total ? (n / total) * 100 : 0}%"></span><span class="label">{o.label}</span><span class="mono">{n}</span></div>
    {/each}
  {/if}
  {#if poll.responses.length}
    <details>
      <summary class="small muted">Your team's answers ({poll.responses.length})</summary>
      {#each poll.responses as r (r.participant)}
        <p class="small"><strong>{r.name}</strong>: {poll.options.find((o) => o.id === r.option)?.label ?? ''}{r.text ? ` — ${r.text}` : ''}</p>
      {/each}
    </details>
  {/if}
</article>

<style>
  .poll { border: 1px solid var(--line); border-radius: var(--radius); padding: 10px 12px; background: var(--panel-2); display: grid; gap: 8px; }
  .poll.open { border-color: var(--accent); }
  .options { display: grid; gap: 6px; }
  .options button { padding: 12px; font-size: 16px; text-align: left; }
  .options button.chosen { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); font-weight: 600; }
  form input { font-size: 16px; }
  .small { font-size: 12px; margin: 0; }
  .bar { position: relative; display: flex; gap: 8px; padding: 6px 8px; border-radius: 3px; background: var(--bg); overflow: hidden; }
  .fill { position: absolute; inset: 0 auto 0 0; background: rgba(0, 255, 65, 0.2); }
  .label { position: relative; flex: 1; }
  .bar .mono { position: relative; }
</style>
