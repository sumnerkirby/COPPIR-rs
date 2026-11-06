<script lang="ts">
  import { download, fileStem, logCsv } from '../../lib/exports';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import Icon from '../ui/Icon.svelte';
  import Timeline from './Timeline.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const assets = $derived(Object.values(ex.assets).sort((a, b) => a.name.localeCompare(b.name)));

  let text = $state('');
  let asset = $state('');
  let view = $state<'list' | 'timeline'>('list');
  let list = $state<HTMLOListElement>();
  let pinned = true;

  async function log(e: SubmitEvent) {
    e.preventDefault();
    const r = await ws.send({ type: 'log', text, asset: asset || undefined });
    if (r) {
      text = '';
      asset = '';
    }
  }

  // Follow new entries, unless the reader has scrolled up to look at history.
  function onscroll() {
    if (list) pinned = list.scrollHeight - list.scrollTop - list.clientHeight < 24;
  }
  $effect(() => {
    void ex.log.length;
    if (pinned && list) queueMicrotask(() => list && (list.scrollTop = list.scrollHeight));
  });
</script>

<form onsubmit={log}>
  <input bind:value={text} placeholder="Record a decision or action…" aria-label="Log entry" maxlength="500" />
  <div class="row">
    <select bind:value={asset} aria-label="Related asset" class="grow">
      <option value="">No specific asset</option>
      {#each assets as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
    </select>
    <button class="primary" disabled={!text.trim()}>Log</button>
  </div>
</form>

<div class="row">
  <div class="seg grow" role="group" aria-label="View">
    <button class:on={view === 'list'} onclick={() => (view = 'list')}>List</button>
    <button class:on={view === 'timeline'} onclick={() => (view = 'timeline')}>Timeline</button>
  </div>
  <button class="ghost small icon" onclick={() => download(`${fileStem(ex)} decision log.csv`, logCsv(ex), 'text/csv')} disabled={!ex.log.length}>
    <Icon name="download" size={14} /> CSV
  </button>
</div>

{#if !ex.log.length}
  <p class="empty">Nothing logged yet. Fired injects log themselves.</p>
{:else if view === 'timeline'}
  <Timeline entries={ex.log} />
{:else}
  <ol bind:this={list} {onscroll}>
    {#each ex.log as e (e.id)}
      <li class:auto={e.auto}>
        <span class="mono muted">{new Date(e.at).toLocaleTimeString()}</span>
        <span>{e.text}{#if e.auto}<span class="tag">AUTO</span>{/if}</span>
        {#if e.asset_name}
          <button class="ghost link" onclick={() => e.asset && ex.assets[e.asset] && ws.select(e.asset)}>{e.asset_name}</button>
        {/if}
        {#if e.notes}<span class="notes muted">{e.notes}</span>{/if}
      </li>
    {/each}
  </ol>
{/if}

<style>
  form { display: grid; gap: 6px; }
  .seg { display: flex; }
  .seg button { flex: 1; border-radius: 0; font-size: 12px; padding: 4px; }
  .seg button:first-child { border-radius: var(--radius) 0 0 var(--radius); }
  .seg button:last-child { border-radius: 0 var(--radius) var(--radius) 0; }
  .seg button.on { background: var(--accent); color: var(--accent-ink); }
  .small { font-size: 12px; }
  ol { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; max-height: 60vh; overflow-y: auto; font-size: 13px; }
  li { display: grid; gap: 2px; padding-left: 8px; border-left: 2px solid var(--line); }
  li.auto { border-left-color: var(--warn); }
  .tag { margin-left: 6px; font-size: 10px; color: var(--warn); border: 1px solid var(--warn); border-radius: 3px; padding: 0 3px; }
  .link { padding: 0; font-size: 12px; color: var(--accent); justify-self: start; }
  .notes { font-size: 12px; }
</style>
