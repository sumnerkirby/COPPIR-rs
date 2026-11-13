<script lang="ts">
  // The page a phone lands on from the QR code or the room address.
  import { untrack } from 'svelte';
  import { api } from '../lib/api';
  import { router } from '../lib/router.svelte';
  import { session } from '../lib/session.svelte';
  import type { JoinInfo } from '../lib/types/JoinInfo';

  let code = $state(new URLSearchParams(location.search).get('code')?.toUpperCase() ?? '');
  let info = $state<JoinInfo | null>(null);
  let name = $state('');
  let team = $state('');
  let error = $state('');
  let busy = $state(false);

  async function check(e?: SubmitEvent) {
    e?.preventDefault();
    error = '';
    busy = true;
    try {
      info = await api.joinInfo(code);
      if (info.role === 'observer') name = name || 'Screen';
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = false;
    }
  }
  if (untrack(() => code)) check();

  async function join(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    busy = true;
    try {
      await api.join(code, name, team || undefined);
      // Learn the new role before leaving /join, so the host's own screens
      // never flash up (and fetch) for the new participant.
      await session.load();
      router.go('/');
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<main>
  <h1>COPPIR</h1>
  {#if !info}
    <form onsubmit={check}>
      <label>Join code
        <!-- svelte-ignore a11y_autofocus -->
        <input class="code" bind:value={code} required maxlength="12" autocapitalize="characters" autocomplete="off" spellcheck="false" autofocus />
      </label>
      <button class="primary" disabled={busy || code.trim().length < 4}>Continue</button>
    </form>
    <p class="muted">Ask the facilitator for the code, or scan the QR code on their screen.</p>
  {:else}
    <p class="joining">Joining <strong>{info.exercise}</strong> as {info.role === 'observer' ? 'an observer' : 'a player'}</p>
    <form onsubmit={join}>
      <label>{info.role === 'observer' ? 'Name for this screen' : 'Your name'}
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={name} required maxlength="60" autocomplete="name" autofocus />
      </label>
      {#if info.role === 'player' && info.teams.length}
        <fieldset>
          <legend>Your team</legend>
          {#each info.teams as t (t.id)}
            <label class="team" style:--c={t.color}>
              <input type="radio" bind:group={team} value={t.id} required /> {t.name}
            </label>
          {/each}
        </fieldset>
      {/if}
      <button class="primary" disabled={busy || !name.trim()}>Join</button>
      <button type="button" class="ghost" onclick={() => (info = null)}>Use a different code</button>
    </form>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</main>

<style>
  main { max-width: 420px; margin: 0 auto; padding: 12vh 20px 20px; display: grid; gap: 18px; }
  h1 { font-family: var(--mono); color: var(--accent); letter-spacing: 0.3em; font-size: 30px; text-align: center; }
  form { display: grid; gap: 14px; }
  input, button { font-size: 17px; padding: 12px; }
  .code { font-family: var(--mono); letter-spacing: 0.3em; text-transform: uppercase; text-align: center; font-size: 24px; }
  fieldset { border: 1px solid var(--line); border-radius: var(--radius); display: grid; gap: 4px; padding: 10px; margin: 0; }
  legend { color: var(--muted); font-size: 12px; padding: 0 4px; }
  .team { display: flex; flex-direction: row; align-items: center; gap: 10px; color: var(--text); font-size: 16px; padding: 8px; border-left: 4px solid var(--c); }
  .joining, .muted { margin: 0; text-align: center; }
  .error { color: var(--danger); margin: 0; text-align: center; }
</style>
