<script lang="ts">
  import { useWorkspace, type Tab } from '../../lib/workspace.svelte';
  import AssetsTab from '../assets/AssetsTab.svelte';
  import ScriptPanel from '../injects/ScriptPanel.svelte';
  import LogPanel from '../log/LogPanel.svelte';
  import SetupPanel from '../setup/SetupPanel.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  const queued = $derived(Object.values(ex.injects).filter((i) => !i.fired_at).length);
  const tabs = $derived<[Tab, string, number | null][]>([
    ['assets', 'Assets', Object.keys(ex.assets).length],
    ['injects', 'Script', queued],
    ['log', 'Log', ex.log.length],
    ['setup', 'Setup', null],
  ]);
</script>

<aside>
  <div class="tabs" role="tablist">
    {#each tabs as [id, label, n] (id)}
      <button role="tab" aria-selected={ws.tab === id} class:active={ws.tab === id} onclick={() => (ws.tab = id)}>
        {label}{#if n !== null}<span class="n">{n}</span>{/if}
      </button>
    {/each}
  </div>
  <div class="panel" role="tabpanel">
    {#if ws.tab === 'assets'}<AssetsTab />
    {:else if ws.tab === 'injects'}<ScriptPanel />
    {:else if ws.tab === 'log'}<LogPanel />
    {:else}<SetupPanel />{/if}
  </div>
</aside>

<style>
  aside { border-left: 1px solid var(--line); background: var(--panel); display: grid; grid-template-rows: auto 1fr; min-height: 0; }
  .tabs { display: flex; border-bottom: 1px solid var(--line); }
  .tabs button { flex: 1; border: 0; border-radius: 0; background: none; padding: 10px 4px; color: var(--muted); border-bottom: 2px solid transparent; }
  .tabs button.active { color: var(--text); border-bottom-color: var(--accent); }
  .n { margin-left: 6px; font-family: var(--mono); font-size: 11px; color: var(--muted); }
  .panel { overflow-y: auto; padding: 14px; display: grid; gap: 14px; align-content: start; min-height: 0; }
  @media (max-width: 900px) { aside { border-left: 0; border-top: 1px solid var(--line); } }
</style>
