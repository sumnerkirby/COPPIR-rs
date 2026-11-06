<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { LiveExercise } from '../lib/live.svelte';
  import type { Command } from '../lib/types/Command';
  import type { GeoPoint } from '../lib/types/GeoPoint';
  import MapView from './MapView.svelte';
  import Sitrep from './Sitrep.svelte';
  import AssetPanel from './AssetPanel.svelte';
  import LogFeed from './LogFeed.svelte';

  let { id }: { id: string } = $props();

  // The parent re-creates this component per exercise (`{#key}`), so the id is fixed.
  const live = new LiveExercise(untrack(() => id));
  onDestroy(() => live.close());

  let selected = $state<string | null>(null);
  let adding = $state(false);
  let pending = $state<GeoPoint | null>(null);
  let error = $state('');

  /** Send a command; surface a refusal instead of throwing. */
  async function send(cmd: Command) {
    error = '';
    try {
      return await live.send(cmd);
    } catch (e) {
      error = (e as Error).message;
      return null;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
    if (e.key === 'a') adding = !adding;
    if (e.key === 'Escape') {
      adding = false;
      pending = null;
      selected = null;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="console">
  <header>
    <a href="#/" class="brand" aria-label="All exercises">COPPIR</a>
    <h1>{live.exercise?.name ?? 'Loading…'}</h1>
    <span class="conn {live.connection}" title="Connection: {live.connection}" aria-label="Connection {live.connection}"></span>
    <span class="muted mono seq">seq {live.seq}</span>
    <button class:primary={adding} onclick={() => (adding = !adding)} title="Add asset (A)">
      {adding ? 'Click the map…' : '+ Asset'}
    </button>
  </header>

  {#if live.exercise}
    <Sitrep exercise={live.exercise} />
  {/if}

  <div class="body">
    <div class="map">
      {#if live.exercise}
        <MapView
          exercise={live.exercise}
          {selected}
          {adding}
          onselect={(a) => {
            selected = a;
            pending = null;
          }}
          onplace={(p) => {
            pending = p;
            selected = null;
          }}
        />
      {:else}
        <p class="muted loading">Loading exercise…</p>
      {/if}
    </div>
    <aside>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      {#if live.exercise}
        <AssetPanel
          exercise={live.exercise}
          {selected}
          {pending}
          {send}
          ondone={() => {
            pending = null;
            adding = false;
          }}
          onselect={(a) => (selected = a)}
        />
        <LogFeed exercise={live.exercise} {send} />
      {/if}
    </aside>
  </div>
</div>

<style>
  .console { display: grid; grid-template-rows: auto auto 1fr; height: 100%; }
  header {
    display: flex; align-items: center; gap: 12px; padding: 8px 14px;
    background: var(--panel); border-bottom: 1px solid var(--line);
  }
  .brand { font-family: var(--mono); color: var(--accent); text-decoration: none; letter-spacing: 0.25em; font-weight: 700; }
  h1 { font-size: 15px; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .conn { width: 9px; height: 9px; border-radius: 50%; background: var(--warn); }
  .conn.live { background: var(--accent); }
  .conn.offline { background: var(--danger); }
  .seq { font-size: 12px; }
  .body { display: grid; grid-template-columns: 1fr 340px; min-height: 0; }
  .map { position: relative; min-height: 0; }
  .loading { padding: 20px; }
  aside {
    border-left: 1px solid var(--line); background: var(--panel); overflow-y: auto;
    display: grid; align-content: start; gap: 16px; padding: 14px;
  }
  .error { color: var(--danger); margin: 0; }
  @media (max-width: 800px) {
    .body { grid-template-columns: 1fr; grid-template-rows: 55vh auto; }
    aside { border-left: 0; border-top: 1px solid var(--line); }
  }
</style>
