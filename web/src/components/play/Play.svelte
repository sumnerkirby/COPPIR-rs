<script lang="ts">
  // What a player sees on their own device: their team's picture, the injects
  // sent to them, and their team's log. Everything here is read-only except
  // adding log entries; the server sends only what this team may see.
  import { onDestroy, untrack } from 'svelte';
  import { session } from '../../lib/session.svelte';
  import { statusValue } from '../../lib/sitrep';
  import { ui } from '../../lib/ui.svelte';
  import { provideWorkspace, Workspace } from '../../lib/workspace.svelte';
  import type { Me } from '../../lib/types/Me';
  import InjectBanner from '../console/InjectBanner.svelte';
  import BrandFrame from '../BrandFrame.svelte';
  import MapView from '../map/MapView.svelte';
  import { mapKey } from '../../lib/maps';
  import Sitrep from '../Sitrep.svelte';
  import Icon from '../ui/Icon.svelte';
  import ExerciseClock from '../console/ExerciseClock.svelte';
  import PlayerPoll from './PlayerPoll.svelte';
  import ViewSwitch from '../ViewSwitch.svelte';

  let { me }: { me: Me } = $props();
  const ws = new Workspace(
    untrack(() => me.exercise_id ?? ''),
    true,
  );
  provideWorkspace(ws);
  // A dropped connection may mean we were moved to another team or removed:
  // ask the server who we are now. The app re-renders if the answer changed.
  ws.live.onDrop = () => void session.load();
  onDestroy(() => ws.close());

  const ex = $derived(ws.exercise);
  const team = $derived(ex?.teams.find((t) => t.id === me.team));
  const injects = $derived(
    Object.values(ex?.injects ?? {}).sort((a, b) => (b.fired_at ?? '').localeCompare(a.fired_at ?? '')),
  );
  const selected = $derived(ws.selected ? ex?.assets[ws.selected] : undefined);
  // Open polls first: they want an answer.
  const polls = $derived(
    Object.values(ex?.polls ?? {}).sort((a, b) => Number(b.state === 'open') - Number(a.state === 'open')),
  );

  let tab = $state<'feed' | 'map' | 'log'>('feed');
  let unread = $state(0);
  $effect(() =>
    ws.live.on((env) => {
      const t = env.event.type;
      if ((t === 'inject_upserted' || (t === 'poll_upserted' && env.event.poll.state === 'open')) && tab !== 'feed') unread += 1;
    }),
  );
  $effect(() => {
    if (tab === 'feed') unread = 0;
  });

  let text = $state('');
  async function log(e: SubmitEvent) {
    e.preventDefault();
    if (await ws.send({ type: 'log', text })) text = '';
  }

  async function leave() {
    if (await ui.confirm('Leave the exercise?', { body: 'You can join again with the code.', confirm: 'Leave' })) await session.leave();
  }
  const time = (iso: string | null) => (iso ? new Date(iso).toLocaleTimeString() : '');
</script>

<BrandFrame branding={ws.exercise?.branding}>
<div class="play">
  <header>
    <span class="brand">COPPIR</span>
    <span class="name grow">{ex?.name ?? 'Connecting…'}</span>
    {#if ex}<ExerciseClock />{/if}
    {#if team}<span class="chip" style:--c={team.color}>{team.name}</span>{/if}
    <span class="conn {ws.live.connection}" role="status" aria-label="Connection {ws.live.connection}"></span>
    <button class="ghost" onclick={leave} aria-label="Leave"><Icon name="log-out" size={16} /></button>
  </header>

  {#if ex}
    <main class:map={tab === 'map'}>
      {#if tab === 'feed'}
        <Sitrep exercise={ex} />
        <section class="feed">
          {#each polls as p (p.id)}<PlayerPoll poll={p} {me} />{/each}
          {#each injects as i (i.id)}
            <article class="inject {i.severity}">
              <div class="row"><strong class="grow">{i.title}</strong><span class="mono muted">{time(i.fired_at)}</span></div>
              {#if i.body}<p>{i.body}</p>{/if}
            </article>
          {:else}
            <p class="empty">Nothing yet. Injects from the facilitator appear here.</p>
          {/each}
        </section>
      {:else if tab === 'map'}
        <div class="mapwrap">
          {#if ws.view === 'graph'}
            {#await import('../graph/GraphView.svelte') then { default: GraphView }}<GraphView />{/await}
          {:else}
            {#key mapKey(ws.exercise)}
            <MapView />
            {/key}
          {/if}
          <ViewSwitch />
          <InjectBanner />
          {#if selected}
            <div class="card">
              <div class="row"><strong class="grow">{selected.name}</strong><button class="ghost" onclick={() => ws.select(null)} aria-label="Close">✕</button></div>
              <span class="muted">{ex.taxonomy.asset_types.find((t) => t.id === selected.type_id)?.name}</span>
              {#each ex.taxonomy.status_sets as set (set.id)}
                {@const v = statusValue(set, selected)}
                <span><span class="dot" style:background={v?.color}></span> {set.name}: {v?.label}</span>
              {/each}
              {#each ex.taxonomy.fields.filter((f) => selected.fields[f.id] !== undefined) as f (f.id)}
                {@const v = selected.fields[f.id]}
                <span><span class="muted">{f.label}:</span> {v === true ? 'Yes' : v === false ? 'No' : String(v)}</span>
              {/each}
              {#if selected.notes}<p>{selected.notes}</p>{/if}
            </div>
          {/if}
        </div>
      {:else}
        <section class="log">
          <form onsubmit={log}>
            <input bind:value={text} placeholder="Record what your team decided or did…" aria-label="Log entry" maxlength="500" />
            <button class="primary" disabled={!text.trim()}>Log</button>
          </form>
          {#each [...ex.log].reverse() as e (e.id)}
            <div class="entry" class:auto={e.auto}>
              <span class="mono muted">{time(e.at)}{e.author ? ` · ${e.author}` : ''}</span>
              <span>{e.text}</span>
            </div>
          {:else}
            <p class="empty">Your team's log is empty.</p>
          {/each}
        </section>
      {/if}
    </main>
  {:else}
    <p class="empty pad">{ws.live.connection === 'offline' ? 'Connection lost; retrying…' : 'Connecting…'}</p>
  {/if}

  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === 'feed'} class:on={tab === 'feed'} onclick={() => (tab = 'feed')}>
      <Icon name="inbox" size={20} /> Feed{#if unread}<span class="badge">{unread}</span>{/if}
    </button>
    <button role="tab" aria-selected={tab === 'map'} class:on={tab === 'map'} onclick={() => (tab = 'map')}><Icon name="map" size={20} /> Map</button>
    <button role="tab" aria-selected={tab === 'log'} class:on={tab === 'log'} onclick={() => (tab = 'log')}><Icon name="notebook-pen" size={20} /> Log</button>
  </div>
</div>
</BrandFrame>

<style>
  .play { display: grid; grid-template-rows: auto 1fr auto; height: 100%; }
  header { display: flex; align-items: center; gap: 8px; padding: 8px 12px; background: var(--panel); border-bottom: 1px solid var(--line); }
  .brand { font-family: var(--mono); color: var(--accent); letter-spacing: 0.2em; font-weight: 700; font-size: 13px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chip { border: 1px solid var(--c); color: var(--c); border-radius: 999px; padding: 1px 10px; font-size: 12px; white-space: nowrap; }
  .conn { width: 9px; height: 9px; border-radius: 50%; background: var(--warn); flex: none; }
  .conn.live { background: var(--accent); }
  .conn.offline { background: var(--danger); }
  main { overflow-y: auto; min-height: 0; }
  main.map { overflow: hidden; }
  .mapwrap { position: relative; height: 100%; }
  .feed, .log { display: grid; gap: 10px; padding: 12px; align-content: start; }
  .inject { border: 1px solid var(--line); border-left: 5px solid #3399ff; border-radius: var(--radius); padding: 10px 12px; background: var(--panel-2); display: grid; gap: 4px; }
  .inject.warning { border-left-color: var(--warn); }
  .inject.critical { border-left-color: var(--danger); }
  .inject p { margin: 0; }
  form { display: flex; gap: 6px; }
  form input { font-size: 16px; }
  .entry { display: grid; gap: 2px; padding-left: 8px; border-left: 2px solid var(--line); }
  .entry.auto { border-left-color: var(--warn); }
  .card {
    position: absolute; left: 10px; right: 10px; bottom: 34px; z-index: 12; display: grid; gap: 3px;
    background: color-mix(in srgb, var(--panel) 94%, transparent); border: 1px solid var(--line); border-radius: var(--radius); padding: 10px 12px;
  }
  .card p { margin: 4px 0 0; font-size: 13px; }
  .dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; }
  .pad { padding: 20px; }
  .tabs { display: flex; background: var(--panel); border-top: 1px solid var(--line); padding-bottom: env(safe-area-inset-bottom); }
  .tabs button { flex: 1; display: grid; justify-items: center; gap: 2px; border: 0; border-radius: 0; background: none; padding: 8px 0; color: var(--muted); font-size: 12px; position: relative; }
  .tabs button.on { color: var(--accent); }
  .badge { position: absolute; top: 4px; left: calc(50% + 8px); background: var(--danger); color: #fff; border-radius: 999px; font-size: 11px; padding: 0 6px; }
</style>
