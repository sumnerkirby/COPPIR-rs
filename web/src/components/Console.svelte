<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { provideWorkspace, Workspace } from '../lib/workspace.svelte';
  import { ui } from '../lib/ui.svelte';
  import TopBar from './console/TopBar.svelte';
  import Sitrep from './Sitrep.svelte';
  import MapView from './map/MapView.svelte';
  import MapToolbar from './map/MapToolbar.svelte';
  import Sidebar from './console/Sidebar.svelte';
  import TaxonomyEditor from './taxonomy/TaxonomyEditor.svelte';
  import ShortcutHelp from './console/ShortcutHelp.svelte';
  import InjectBanner from './console/InjectBanner.svelte';
  import RoomPanel from './room/RoomPanel.svelte';
  import Timeline from './injects/Timeline.svelte';

  let { id }: { id: string } = $props();

  // The parent re-creates this component per exercise (`{#key}`), so the id is fixed.
  const ws = new Workspace(untrack(() => id));
  provideWorkspace(ws);
  onDestroy(() => ws.close());

  const mod = (e: KeyboardEvent) => e.metaKey || e.ctrlKey;

  function onkeydown(e: KeyboardEvent) {
    const typing =
      e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
    if (ui.dialog || ws.showTaxonomy || ws.showRoom || ws.showTimeline) return;
    if (mod(e) && e.key.toLowerCase() === 'z' && !e.shiftKey && !typing) {
      e.preventDefault();
      ws.undo();
    } else if (mod(e) && e.key.toLowerCase() === 'l') {
      e.preventDefault();
      ws.tab = 'log';
    } else if (mod(e) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      ws.tab = 'assets';
      ws.select(null);
      queueMicrotask(() => document.getElementById('asset-search')?.focus());
    } else if (e.key === 'Escape') {
      if (ws.mode.kind !== 'select') ws.setMode({ kind: 'select' });
      else if (ws.placing) ws.placing = null;
      else ws.select(null);
    } else if (typing || mod(e) || e.altKey) {
      return;
    } else if (e.key === 'a') {
      ws.setMode(ws.mode.kind === 'add' ? { kind: 'select' } : { kind: 'add' });
    } else if (e.key === 'm') {
      ws.setMode(ws.mode.kind === 'measure' ? { kind: 'select' } : { kind: 'measure' });
    } else if (e.key === '?') {
      ws.showHelp = true;
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="console">
  <TopBar />
  {#if ws.exercise}
    <Sitrep exercise={ws.exercise} />
  {/if}
  <div class="body">
    <div class="map">
      {#if ws.exercise}
        <MapView />
        <MapToolbar />
        <InjectBanner />
      {:else}
        <p class="empty loading">{ws.live.connection === 'offline' ? 'Cannot reach the server; retrying…' : 'Loading exercise…'}</p>
      {/if}
    </div>
    {#if ws.exercise}<Sidebar />{/if}
  </div>
</div>

{#if ws.showTaxonomy && ws.exercise}
  <TaxonomyEditor onclose={() => (ws.showTaxonomy = false)} />
{/if}
{#if ws.showTimeline && ws.exercise}
  <Timeline onclose={() => (ws.showTimeline = false)} />
{/if}
{#if ws.showRoom && ws.exercise}
  <RoomPanel onclose={() => (ws.showRoom = false)} />
{/if}
{#if ws.showHelp}
  <ShortcutHelp onclose={() => (ws.showHelp = false)} />
{/if}

<style>
  .console { display: grid; grid-template-rows: auto auto 1fr; height: 100%; }
  .body { display: grid; grid-template-columns: 1fr 380px; min-height: 0; }
  .map { position: relative; min-height: 0; overflow: hidden; }
  .loading { padding: 20px; }
  @media (max-width: 900px) {
    .body { grid-template-columns: 1fr; grid-template-rows: 60vh auto; }
  }
</style>
