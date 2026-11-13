<script lang="ts">
  // The projector: the shared picture for the room's big screen. It shows only
  // what everyone is allowed to see, and has nothing to click by accident.
  import { onDestroy, untrack } from 'svelte';
  import { session } from '../../lib/session.svelte';
  import { provideWorkspace, Workspace } from '../../lib/workspace.svelte';
  import type { Me } from '../../lib/types/Me';
  import InjectBanner from '../console/InjectBanner.svelte';
  import MapView from '../map/MapView.svelte';
  import Sitrep from '../Sitrep.svelte';

  let { me }: { me: Me } = $props();
  const ws = new Workspace(
    untrack(() => me.exercise_id ?? ''),
    true,
  );
  provideWorkspace(ws);
  ws.live.onDrop = () => void session.load();
  onDestroy(() => ws.close());

  const ex = $derived(ws.exercise);
  const ticker = $derived([...(ex?.log ?? [])].reverse().slice(0, 12));
  let now = $state(new Date());
  $effect(() => {
    const t = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(t);
  });
</script>

<div class="screen">
  <header>
    <span class="brand">COPPIR</span>
    <h1 class="grow">{ex?.name ?? 'Connecting…'}</h1>
    <span class="conn {ws.live.connection}" role="status" aria-label="Connection {ws.live.connection}"></span>
    <span class="mono clock">{now.toLocaleTimeString()}</span>
  </header>
  {#if ex}
    <Sitrep exercise={ex} />
    <div class="body">
      <div class="map"><MapView /><InjectBanner /></div>
      <aside aria-label="Latest log entries">
        {#each ticker as e (e.id)}
          <div class="entry" class:auto={e.auto}>
            <span class="mono muted">{new Date(e.at).toLocaleTimeString()}</span>
            <span>{e.text}</span>
          </div>
        {:else}
          <p class="empty">The log is empty.</p>
        {/each}
      </aside>
    </div>
  {/if}
</div>

<style>
  .screen { display: grid; grid-template-rows: auto auto 1fr; height: 100dvh; }
  header { display: flex; align-items: center; gap: 14px; padding: 8px 16px; background: var(--panel); border-bottom: 1px solid var(--line); }
  .brand { font-family: var(--mono); color: var(--accent); letter-spacing: 0.25em; font-weight: 700; }
  h1 { font-size: 20px; }
  .clock { font-size: 20px; }
  .conn { width: 10px; height: 10px; border-radius: 50%; background: var(--warn); }
  .conn.live { background: var(--accent); }
  .conn.offline { background: var(--danger); }
  .body { display: grid; grid-template-columns: 1fr 340px; min-height: 0; }
  .map { position: relative; min-height: 0; overflow: hidden; }
  aside { border-left: 1px solid var(--line); background: var(--panel); padding: 12px; display: grid; gap: 10px; align-content: start; overflow: hidden; font-size: 15px; }
  .entry { display: grid; gap: 2px; padding-left: 8px; border-left: 3px solid var(--line); }
  .entry.auto { border-left-color: var(--warn); }
  @media (max-width: 900px) { .body { grid-template-columns: 1fr; } aside { display: none; } }
</style>
