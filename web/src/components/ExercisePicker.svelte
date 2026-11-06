<script lang="ts">
  import { api } from '../lib/api';
  import type { ExerciseSummary } from '../lib/types/ExerciseSummary';

  let exercises = $state<ExerciseSummary[] | null>(null);
  let error = $state('');
  let name = $state('');
  let busy = $state(false);

  async function load() {
    try {
      exercises = await api.list();
    } catch (e) {
      error = (e as Error).message;
    }
  }
  load();

  const open = (id: string) => (location.hash = `#/x/${id}`);

  async function create(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      const snap = await api.create(name);
      open(snap.exercise.id);
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = false;
    }
  }

  async function importFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    busy = true;
    error = '';
    try {
      const scenario: unknown = JSON.parse(await file.text());
      const snap = await api.importV1(file.name.replace(/\.json$/i, ''), scenario);
      open(snap.exercise.id);
    } catch (err) {
      error = err instanceof SyntaxError ? `${file.name} is not valid JSON` : (err as Error).message;
    } finally {
      busy = false;
    }
  }

  async function remove(ex: ExerciseSummary) {
    if (!confirm(`Delete "${ex.name}" and its whole history? This cannot be undone.`)) return;
    try {
      await api.remove(ex.id);
      await load();
    } catch (err) {
      error = (err as Error).message;
    }
  }

  const when = (iso: string) => new Date(iso).toLocaleString();
</script>

<main>
  <header>
    <h1>COPPIR</h1>
    <p class="muted">Common Operational Picture for incident response exercises</p>
  </header>

  <section class="actions">
    <form onsubmit={create}>
      <input bind:value={name} placeholder="New exercise name" aria-label="New exercise name" required maxlength="200" />
      <button class="primary" disabled={busy || !name.trim()}>Create</button>
    </form>
    <label class="import">
      <span>Import a v1 scenario (.json)</span>
      <input type="file" accept=".json,application/json" onchange={importFile} disabled={busy} />
    </label>
  </section>

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <section>
    <h2>Exercises</h2>
    {#if exercises === null}
      <p class="muted">Loading…</p>
    {:else if exercises.length === 0}
      <p class="muted">No exercises yet. Create one above, or import a scenario from the v1 app.</p>
    {:else}
      <ul>
        {#each exercises as ex (ex.id)}
          <li>
            <button class="open" onclick={() => open(ex.id)}>
              <strong>{ex.name}</strong>
              <span class="muted">{ex.asset_count} assets · {ex.head} events · updated {when(ex.updated_at)}</span>
            </button>
            <button class="danger" onclick={() => remove(ex)} aria-label={`Delete ${ex.name}`}>Delete</button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</main>

<style>
  main { max-width: 760px; margin: 0 auto; padding: 48px 16px; display: grid; gap: 28px; }
  h1 { font-family: var(--mono); color: var(--accent); letter-spacing: 0.3em; font-size: 32px; }
  h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.12em; color: var(--muted); margin-bottom: 10px; }
  .actions { display: grid; gap: 14px; }
  form { display: flex; gap: 8px; }
  .import input { padding: 4px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  li { display: flex; gap: 8px; }
  .open { flex: 1; display: grid; text-align: left; gap: 2px; padding: 10px 12px; background: var(--panel); }
  .open span { font-size: 12px; }
  .error { color: var(--danger); margin: 0; }
</style>
