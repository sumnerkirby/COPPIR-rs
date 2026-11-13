<script lang="ts">
  import { api } from '../../lib/api';
  import { briefingHtml, download, fileStem, logCsv } from '../../lib/exports';
  import { router } from '../../lib/router.svelte';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import Icon from '../ui/Icon.svelte';
  import ExerciseClock from './ExerciseClock.svelte';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise);
  let menu = $state(false);

  async function rename() {
    if (!ex) return;
    const name = await ui.prompt('Rename exercise', 'Name', ex.name, 'Rename');
    if (name && name !== ex.name) await ws.send({ type: 'rename', name });
  }

  async function duplicate() {
    if (!ex) return;
    const name = await ui.prompt('Duplicate exercise', 'Name of the copy', `${ex.name} (copy)`, 'Duplicate');
    if (!name) return;
    try {
      const snap = await api.duplicate(ex.id, name);
      router.go(`/x/${snap.exercise.id}`);
    } catch (e) {
      ui.toast((e as Error).message, 'critical');
    }
  }

  function exportLog() {
    if (ex) download(`${fileStem(ex)} decision log.csv`, logCsv(ex), 'text/csv');
    menu = false;
  }
  function exportBriefing() {
    if (ex) download(`${fileStem(ex)} briefing.html`, briefingHtml(ex), 'text/html');
    menu = false;
  }
</script>

<header>
  <a href="/" class="brand" onclick={(e) => { e.preventDefault(); router.go('/'); }} aria-label="All exercises">COPPIR</a>
  {#if ex?.branding.logo}<img src={ex.branding.logo} alt="" class="logo" />{/if}
  {#if ex?.branding.org}<span class="org muted">{ex.branding.org}</span>{/if}
  <button class="ghost name" onclick={rename} title="Rename">{ex?.name ?? 'Loading…'}</button>
  <span class="conn {ws.live.connection}" role="status" aria-label="Connection {ws.live.connection}" title="Connection: {ws.live.connection}"></span>
  {#if ex}<ExerciseClock />{/if}
  <div class="spacer"></div>
  <button class="ghost icon" onclick={() => (ws.showRoom = true)} title="Let people join from their own devices" aria-label="Room"><Icon name="users" /> Room</button>
  <button class="ghost icon" onclick={() => ws.undo()} title="Undo (Ctrl/⌘+Z)" aria-label="Undo"><Icon name="undo-2" /> Undo</button>
  <button class="ghost icon" onclick={() => (ws.showTaxonomy = true)} title="Statuses, asset types and sectors" aria-label="Taxonomy"><Icon name="tags" /> Taxonomy</button>
  <div class="menu-wrap">
    <button class="ghost icon" onclick={() => (menu = !menu)} aria-expanded={menu} aria-haspopup="menu" aria-label="Export"><Icon name="download" /> Export</button>
    {#if menu}
      <div class="menu" role="menu">
        <button role="menuitem" onclick={exportLog}>Decision log (CSV)</button>
        <button role="menuitem" onclick={exportBriefing}>Briefing (HTML, printable)</button>
        {#if ex}<a role="menuitem" href={api.exportUrl(ex.id)} download onclick={() => (menu = false)}>Exercise file (.coppir)</a>{/if}
        <button role="menuitem" onclick={() => { menu = false; duplicate(); }}>Duplicate exercise…</button>
      </div>
    {/if}
  </div>
  <button class="ghost" onclick={() => (ws.showHelp = true)} title="Keyboard shortcuts (?)" aria-label="Keyboard shortcuts">?</button>
</header>

<svelte:window onclick={(e) => { if (menu && !(e.target as HTMLElement).closest('.menu-wrap')) menu = false; }} />

<style>
  header {
    display: flex; align-items: center; gap: 10px; padding: 6px 12px;
    background: var(--panel); border-bottom: 1px solid var(--line);
  }
  .brand { font-family: var(--mono); color: var(--accent); text-decoration: none; letter-spacing: 0.25em; font-weight: 700; }
  header { min-width: 0; }
  .name { font-weight: 600; font-size: 15px; max-width: 40vw; min-width: 60px; flex-shrink: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .logo { max-height: 24px; max-width: 100px; }
  .org { font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 180px; flex-shrink: 2; min-width: 0; }
  .conn { width: 9px; height: 9px; border-radius: 50%; background: var(--warn); flex: none; }
  .conn.live { background: var(--accent); }
  .conn.offline { background: var(--danger); }
  .spacer { flex: 1; }
  .menu-wrap { position: relative; }
  .menu {
    position: absolute; right: 0; top: calc(100% + 4px); z-index: 30; display: grid; min-width: 230px;
    background: var(--panel-2); border: 1px solid var(--line); border-radius: var(--radius); padding: 4px;
  }
  .menu > * { text-align: left; border: 0; background: none; padding: 8px 10px; color: var(--text); text-decoration: none; border-radius: 4px; font-size: 14px; }
  .menu > *:hover { background: var(--panel); }
  @media (max-width: 700px) { header :global(.icon) { font-size: 0; gap: 0; } }
</style>
