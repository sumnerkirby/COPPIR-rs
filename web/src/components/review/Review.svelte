<script lang="ts">
  // The after-action review: replay the run on the map or graph, with every
  // team's actions on lanes beneath it; judge the expected actions; write
  // the hot wash; export the report.
  import { onDestroy, untrack } from 'svelte';
  import { api } from '../../lib/api';
  import { fmtClock } from '../../lib/clock.svelte';
  import { mapKey } from '../../lib/maps';
  import { advance, buildReplay, countAt, stateAt, type Replay } from '../../lib/replay';
  import { provideWorkspace, useWorkspace, Workspace } from '../../lib/workspace.svelte';
  import Sitrep from '../Sitrep.svelte';
  import MapView from '../map/MapView.svelte';
  import ViewSwitch from '../ViewSwitch.svelte';
  import Icon from '../ui/Icon.svelte';
  import Assessment from './Assessment.svelte';
  import Feed from './Feed.svelte';
  import HotWash from './HotWash.svelte';
  import Lanes from './Lanes.svelte';

  // The live exercise, for assessing and the hot wash.
  const live = useWorkspace();
  const id = untrack(() => live.live.id);
  // A second, offline workspace holds the moment being replayed. Everything
  // below that draws an exercise (map, graph, sitrep) sees that one.
  const past = new Workspace(id, true, false);
  past.view = untrack(() => live.view);
  provideWorkspace(past);

  let replay = $state.raw<Replay | null>(null);
  let failed = $state('');
  let t = $state(0);
  let index = 0;
  let playing = $state(false);
  let speed = $state(60);
  let tab = $state<'assess' | 'events' | 'notes'>('assess');
  let menu = $state(false);

  api
    .events(id)
    .then((events) => {
      replay = buildReplay(events);
      show(replay.end);
    })
    .catch((e: Error) => (failed = e.message));

  /** Move the replay to time `to`: forward in place, backward from a saved state. */
  function show(to: number) {
    const r = replay;
    if (!r) return;
    to = Math.min(Math.max(0, to), r.end);
    const n = Math.max(1, countAt(r.times, to));
    const ex = past.live.exercise;
    if (ex && n >= index) {
      index = advance(r, ex, index, n);
    } else {
      const s = stateAt(r, to);
      past.live.exercise = s.ex;
      index = s.index;
    }
    t = to;
    // The clock shows the moment, stopped.
    past.live.exercise!.clock = { elapsed_ms: Math.round(to), running_since: null };
  }

  let raf = 0;
  let last = 0;
  function frame(now: number) {
    if (!playing || !replay) return;
    const next = t + (last ? now - last : 0) * speed;
    last = now;
    show(next);
    if (next >= replay.end) playing = false;
    else raf = requestAnimationFrame(frame);
  }
  function play() {
    if (!replay) return;
    if (t >= replay.end) show(0);
    playing = true;
    last = 0;
    raf = requestAnimationFrame(frame);
  }
  function pause() {
    playing = false;
    cancelAnimationFrame(raf);
  }
  function seek(to: number) {
    pause();
    show(to);
  }
  /** Jump to the previous or next mark on the lanes. */
  function step(dir: 1 | -1) {
    if (!replay) return;
    const times = [...new Set([...replay.marks.map((m) => m.t), ...replay.phases.map((p) => p.t), 0, replay.end])].sort((a, b) => a - b);
    const target = dir > 0 ? times.find((x) => x > t + 1) : [...times].reverse().find((x) => x < t - 1);
    seek(target ?? (dir > 0 ? replay.end : 0));
  }

  onDestroy(() => {
    cancelAnimationFrame(raf);
    past.close();
  });

  function onkeydown(e: KeyboardEvent) {
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
    if (typing) return;
    if (e.key === ' ') {
      e.preventDefault();
      if (playing) pause();
      else play();
    } else if (e.key === 'ArrowRight') step(1);
    else if (e.key === 'ArrowLeft') step(-1);
    else if (e.key === 'Escape') live.reviewing = false;
  }

  function printReport() {
    menu = false;
    // Load the report out of sight and print it from there.
    const frame = document.createElement('iframe');
    frame.style.cssText = 'position:fixed;width:0;height:0;border:0;right:0;bottom:0';
    frame.src = api.reportUrl(id);
    frame.onload = () => {
      frame.contentWindow?.print();
      setTimeout(() => frame.remove(), 60_000);
    };
    document.body.appendChild(frame);
  }
</script>

<svelte:window {onkeydown} onclick={(e) => { if (menu && !(e.target as HTMLElement).closest('.menu-wrap')) menu = false; }} />

<div class="review">
  <header>
    <button class="ghost" onclick={() => (live.reviewing = false)} title="Back to the console (Esc)"><Icon name="arrow-left" /> Console</button>
    <h1>Review <span class="muted">{live.exercise?.name}</span></h1>
    <div class="transport" role="group" aria-label="Replay">
      <button class="ghost" onclick={() => step(-1)} title="Previous event (←)" aria-label="Previous event" disabled={!replay}><Icon name="skip-back" /></button>
      {#if playing}
        <button class="primary" onclick={pause} title="Pause (space)" aria-label="Pause"><Icon name="pause" /></button>
      {:else}
        <button class="primary" onclick={play} title="Play (space)" aria-label="Play" disabled={!replay}><Icon name="play" /></button>
      {/if}
      <button class="ghost" onclick={() => step(1)} title="Next event (→)" aria-label="Next event" disabled={!replay}><Icon name="skip-forward" /></button>
      <span class="clock mono" aria-live="off">T+{fmtClock(t)}</span>
      <select bind:value={speed} aria-label="Replay speed" title="Exercise time per second of replay">
        <option value={10}>10 s / s</option>
        <option value={30}>30 s / s</option>
        <option value={60}>1 min / s</option>
        <option value={300}>5 min / s</option>
      </select>
    </div>
    <div class="spacer"></div>
    <div class="menu-wrap">
      <button onclick={() => (menu = !menu)} aria-expanded={menu} aria-haspopup="menu"><Icon name="file-text" /> Report</button>
      {#if menu}
        <div class="menu" role="menu">
          <a role="menuitem" href={api.reportUrl(id)} target="_blank" rel="noopener" onclick={() => (menu = false)}>Open the report</a>
          <button role="menuitem" onclick={printReport}>Print or save as PDF</button>
          <a role="menuitem" href={api.reportUrl(id, 'md', true)} download onclick={() => (menu = false)}>Download Markdown</a>
          <a role="menuitem" href={api.reportUrl(id, 'html', true)} download onclick={() => (menu = false)}>Download HTML</a>
        </div>
      {/if}
    </div>
  </header>

  {#if past.exercise}<Sitrep exercise={past.exercise} />{/if}

  <div class="body">
    <div class="stage">
      {#if failed}
        <p class="empty pad">Could not load the history: {failed}</p>
      {:else if !past.exercise}
        <p class="empty pad">Loading the history…</p>
      {:else if past.view === 'graph'}
        {#await import('../graph/GraphView.svelte') then { default: GraphView }}<GraphView />{/await}
      {:else}
        {#key mapKey(past.exercise)}<MapView />{/key}
      {/if}
      {#if past.exercise}<ViewSwitch />{/if}
    </div>
    <aside>
      <div class="tabs" role="tablist">
        <button role="tab" aria-selected={tab === 'assess'} class:on={tab === 'assess'} onclick={() => (tab = 'assess')}>Expected actions</button>
        <button role="tab" aria-selected={tab === 'events'} class:on={tab === 'events'} onclick={() => (tab = 'events')}>What happened</button>
        <button role="tab" aria-selected={tab === 'notes'} class:on={tab === 'notes'} onclick={() => (tab = 'notes')}>Hot wash</button>
      </div>
      <div class="pane">
        {#if tab === 'assess'}
          <Assessment ws={live} fired={replay?.fired ?? {}} onseek={seek} />
        {:else if tab === 'events'}
          {#if replay}<Feed {replay} {t} onseek={seek} />{/if}
        {:else}
          <HotWash ws={live} />
        {/if}
      </div>
    </aside>
  </div>

  {#if replay}<Lanes {replay} {t} onseek={seek} />{/if}
</div>

<style>
  .review { display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; grid-template-columns: minmax(0, 1fr); height: 100%; }
  header { display: flex; align-items: center; gap: 10px; padding: 6px 12px; background: var(--panel); border-bottom: 1px solid var(--line); min-width: 0; }
  h1 { font-size: 15px; margin: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  h1 .muted { font-weight: 400; margin-left: 6px; }
  .transport { display: flex; align-items: center; gap: 4px; flex: none; }
  .clock { font-size: 15px; min-width: 9ch; text-align: center; color: var(--accent); }
  .transport select { width: auto; font-size: 12px; }
  .spacer { flex: 1; }
  .menu-wrap { position: relative; }
  .menu {
    position: absolute; right: 0; top: calc(100% + 4px); z-index: 30; display: grid; min-width: 210px;
    background: var(--panel-2); border: 1px solid var(--line); border-radius: var(--radius); padding: 4px;
  }
  .menu > * { text-align: left; border: 0; background: none; padding: 8px 10px; color: var(--text); text-decoration: none; border-radius: 4px; font-size: 14px; }
  .menu > *:hover { background: var(--panel); }
  .body { display: grid; grid-template-columns: minmax(0, 1fr) 400px; min-height: 0; }
  .stage { position: relative; min-height: 0; overflow: hidden; }
  .pad { padding: 20px; }
  aside { display: grid; grid-template-rows: auto minmax(0, 1fr); border-left: 1px solid var(--line); background: var(--panel); min-height: 0; }
  .tabs { display: flex; border-bottom: 1px solid var(--line); }
  .tabs button { flex: 1; border: 0; border-radius: 0; background: none; padding: 9px 4px; font-size: 13px; color: var(--muted); border-bottom: 2px solid transparent; }
  .tabs button.on { color: var(--text); border-bottom-color: var(--accent); }
  .pane { overflow-y: auto; padding: 12px; }
  @media (max-width: 900px) {
    .body { grid-template-columns: 1fr; grid-template-rows: 50vh auto; }
    aside { border-left: 0; border-top: 1px solid var(--line); }
    header { flex-wrap: wrap; }
  }
</style>
