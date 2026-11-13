<script lang="ts">
  // Open the exercise to the room: other devices on the same network join
  // with a link or QR code and a short code. Nothing is reachable from the
  // network until the room is opened here, and closing it ends every session.
  import { api } from '../../lib/api';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { RoomStatus } from '../../lib/types/RoomStatus';
  import Icon from '../ui/Icon.svelte';
  import Modal from '../ui/Modal.svelte';

  let { onclose }: { onclose: () => void } = $props();
  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);

  let status = $state<RoomStatus | null>(null);
  let busy = $state(false);
  const room = $derived(status?.open ?? null);
  const here = $derived(room?.exercise_id === ex.id);
  const base = $derived(room?.urls[0] ?? null);

  async function refresh() {
    try {
      status = await api.room();
    } catch (e) {
      ui.toast((e as Error).message, 'critical');
    }
  }
  // Poll while the panel is open, so arrivals show up as they happen.
  $effect(() => {
    void refresh();
    const t = setInterval(refresh, 2500);
    return () => clearInterval(t);
  });

  async function run(f: () => Promise<unknown>) {
    busy = true;
    try {
      await f();
      await refresh();
    } catch (e) {
      ui.toast((e as Error).message, 'critical', 6000);
    } finally {
      busy = false;
    }
  }

  async function close() {
    const n = status?.participants.length ?? 0;
    const ok = await ui.confirm('Close the room?', {
      body: n ? `${n} participant${n === 1 ? '' : 's'} will be disconnected and must join again.` : 'Other devices will no longer be able to connect.',
      confirm: 'Close room',
      danger: true,
    });
    if (ok) await run(() => api.closeRoom());
  }

  const copy = (text: string) => navigator.clipboard?.writeText(text).then(() => ui.toast('Copied', 'ok', 1500));
  const teamName = (id: string | null) => ex.teams.find((t) => t.id === id)?.name ?? '—';
</script>

<Modal title="Room" {onclose} wide>
  {#if status === null}
    <p class="empty">Loading…</p>
  {:else if !room}
    <p>
      Opening the room lets people on the same network join from their own phones and laptops. Players see only what you make visible
      to their team; observers get a read-only screen for the projector.
    </p>
    <button class="primary" onclick={() => run(() => api.openRoom(ex.id))} disabled={busy}>Open the room for “{ex.name}”</button>
  {:else}
    {#if !here}
      <p class="warn">The room is open for a different exercise. Opening it here disconnects everyone there.</p>
      <button onclick={() => run(() => api.openRoom(ex.id))} disabled={busy}>Move the room to “{ex.name}”</button>
    {:else}
      <div class="join">
        {#if base}
          <img class="qr" src={api.qrUrl('player', room.player_code + base)} alt="QR code to join as a player" width="200" height="200" />
        {/if}
        <div class="how">
          {#if base}
            <p class="section-title">Players go to</p>
            <button class="ghost addr mono" onclick={() => copy(`${base}/join`)} title="Copy">{base.replace('http://', '')}/join</button>
            {#if room.urls.length > 1}
              <p class="muted small">Also: {room.urls.slice(1).map((u) => u.replace('http://', '')).join(', ')}</p>
            {/if}
          {:else}
            <p class="warn">This computer has no network address, so other devices cannot reach it. Connect to Wi-Fi or a hotspot, then reopen the room.</p>
          {/if}
          <div class="codes">
            <div><p class="section-title">Player code</p><button class="ghost code mono" onclick={() => copy(room.player_code)} title="Copy">{room.player_code}</button></div>
            <div><p class="section-title">Observer code</p><button class="ghost code mono" onclick={() => copy(room.observer_code)} title="Copy">{room.observer_code}</button></div>
          </div>
          <p class="muted small">The connection is not encrypted. Use it on a network you trust, such as the venue's or your own hotspot.</p>
        </div>
      </div>

      <section>
        <h3 class="section-title">In the room ({status.participants.length})</h3>
        {#each status.participants as p (p.id)}
          <div class="row person">
            <span class="dot" class:on={p.connected} title={p.connected ? 'Connected' : 'Not connected'}></span>
            <span class="grow">{p.name} {#if p.role === 'observer'}<span class="muted">· observer</span>{/if}</span>
            {#if p.role === 'player'}
              <select value={p.team ?? ''} aria-label={`Team for ${p.name}`} onchange={(e) => run(() => api.moveParticipant(p.id, e.currentTarget.value || null))}>
                <option value="">No team</option>
                {#each ex.teams as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
              </select>
            {:else}
              <span class="muted small">{teamName(p.team)}</span>
            {/if}
            <button class="ghost danger small" onclick={() => run(() => api.kick(p.id))} aria-label={`Remove ${p.name}`}><Icon name="user-x" size={14} /></button>
          </div>
        {:else}
          <p class="empty">Nobody has joined yet.</p>
        {/each}
        {#if !ex.teams.length}<p class="empty">No teams yet. Add teams under Setup to give groups their own view.</p>{/if}
      </section>
    {/if}
  {/if}

  {#snippet footer()}
    {#if room && here}
      <button class="ghost" onclick={() => run(() => api.rotateCodes())} disabled={busy} title="People already in stay in">New codes</button>
      <button class="danger" onclick={close} disabled={busy}>Close room</button>
    {/if}
    <span class="grow"></span>
    <button onclick={onclose}>Done</button>
  {/snippet}
</Modal>

<style>
  p { margin: 0; }
  .join { display: flex; gap: 20px; flex-wrap: wrap; align-items: flex-start; }
  .qr { background: #fff; border-radius: 8px; padding: 6px; }
  .how { display: grid; gap: 8px; flex: 1; min-width: 240px; }
  .addr { font-size: 20px; justify-self: start; padding: 2px 6px; color: var(--accent); }
  .codes { display: flex; gap: 24px; }
  .code { font-size: 28px; letter-spacing: 0.2em; padding: 2px 6px; }
  .small { font-size: 12px; }
  .warn { color: var(--warn); }
  section { display: grid; gap: 6px; border-top: 1px solid var(--line); padding-top: 12px; }
  .person select { width: auto; }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--line); flex: none; }
  .dot.on { background: var(--accent); }
</style>
