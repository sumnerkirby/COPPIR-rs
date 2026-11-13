<script lang="ts">
  // The projector: the shared picture for the room's big screen. It shows only
  // what everyone is allowed to see, and has nothing to click by accident.
  import { onDestroy, untrack } from 'svelte';
  import { session } from '../../lib/session.svelte';
  import { provideWorkspace, Workspace } from '../../lib/workspace.svelte';
  import type { Me } from '../../lib/types/Me';
  import InjectBanner from '../console/InjectBanner.svelte';
  import BrandFrame from '../BrandFrame.svelte';
  import MapView from '../map/MapView.svelte';
  import Sitrep from '../Sitrep.svelte';
  import ExerciseClock from '../console/ExerciseClock.svelte';
  import ViewSwitch from '../ViewSwitch.svelte';

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

<BrandFrame branding={ws.exercise?.branding}>
<div class="screen">
  <header>
    {#if ex?.branding.logo}<img src={ex.branding.logo} alt="" class="logo" />{:else}<span class="brand">COPPIR</span>{/if}
    {#if ex?.branding.org}<span class="muted">{ex.branding.org}</span>{/if}
    <h1 class="grow">{ex?.name ?? 'Connecting…'}</h1>
    <span class="conn {ws.live.connection}" role="status" aria-label="Connection {ws.live.connection}"></span>
    {#if ex}<span class="big"><ExerciseClock /></span>{/if}
    <span class="mono clock">{now.toLocaleTimeString()}</span>
  </header>
  {#if ex}
    <Sitrep exercise={ex} />
    <div class="body">
      <div class="map">
        {#if ws.view === 'graph'}
          {#await import('../graph/GraphView.svelte') then { default: GraphView }}<GraphView />{/await}
        {:else}
          {#key ws.exercise?.branding.theme === 'daylight'}
          <MapView />
          {/key}
        {/if}
        <ViewSwitch />
        <InjectBanner />
      </div>
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
</BrandFrame>

<style>
  .screen { display: grid; grid-template-rows: auto auto 1fr; height: 100%; }
  header { display: flex; align-items: center; gap: 14px; padding: 8px 16px; background: var(--panel); border-bottom: 1px solid var(--line); }
  .brand { font-family: var(--mono); color: var(--accent); letter-spacing: 0.25em; font-weight: 700; }
  h1 { font-size: 20px; }
  .logo { max-height: 32px; max-width: 140px; }
  .clock { font-size: 16px; color: var(--muted); }
  .big :global(.time) { font-size: 22px; }
  .big :global(.phase) { font-size: 15px; }
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
