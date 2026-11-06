<script lang="ts">
  import { api } from '../lib/api';
  import { router } from '../lib/router.svelte';
  import { ui } from '../lib/ui.svelte';
  import type { ExerciseSummary } from '../lib/types/ExerciseSummary';
  import Icon from './ui/Icon.svelte';

  let exercises = $state<ExerciseSummary[] | null>(null);
  let legacy = $state<string[]>([]);
  let name = $state('');
  let busy = $state(false);

  async function load() {
    try {
      [exercises, legacy] = await Promise.all([api.list(), api.legacyScenarios().catch(() => [])]);
      // Hide old scenarios that already have an exercise of the same name.
      const names = new Set(exercises.map((e) => e.name));
      legacy = legacy.filter((n) => !names.has(n));
    } catch (e) {
      ui.toast((e as Error).message, 'critical');
    }
  }
  load();

  const open = (id: string) => router.go(`/x/${id}`);

  async function run<T>(f: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    try {
      return await f();
    } catch (e) {
      ui.toast((e as Error).message, 'critical', 6000);
    } finally {
      busy = false;
    }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const snap = await run(() => api.create(name.trim()));
    if (snap) open(snap.exercise.id);
  }

  async function importFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    const snap = await run(async () => {
      let json: unknown;
      try {
        json = JSON.parse(await file.text());
      } catch {
        throw new Error(`${file.name} is not a COPPIR file`);
      }
      // A .coppir file says what it is; anything else is tried as a v1 scenario.
      const isExercise = (json as { format?: string })?.format === 'coppir.exercise';
      return isExercise ? api.importFile(json) : api.importV1(file.name.replace(/\.(json|coppir)$/i, ''), json);
    });
    if (snap) open(snap.exercise.id);
  }

  async function importLegacy(n: string) {
    const snap = await run(() => api.importLegacy(n));
    if (snap) {
      ui.toast(`Imported “${n}”`, 'ok');
      await load();
    }
  }

  async function duplicate(ex: ExerciseSummary) {
    const n = await ui.prompt('Duplicate exercise', 'Name of the copy', `${ex.name} (copy)`, 'Duplicate');
    if (n && (await run(() => api.duplicate(ex.id, n)))) await load();
  }

  async function remove(ex: ExerciseSummary) {
    const ok = await ui.confirm(`Delete “${ex.name}”?`, {
      body: 'The exercise and its whole history are removed. Export it first if you might need it again.',
      confirm: 'Delete',
      danger: true,
    });
    if (ok && (await run(async () => (await api.remove(ex.id), true)))) await load();
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
    <label class="import button">
      <Icon name="upload" /> Import a .coppir file or v1 scenario
      <input type="file" accept=".coppir,.json,application/json" onchange={importFile} disabled={busy} />
    </label>
  </section>

  {#if legacy.length}
    <section class="legacy" aria-label="Scenarios from the previous version">
      <h2 class="section-title">Found {legacy.length} scenario{legacy.length === 1 ? '' : 's'} from the previous version</h2>
      <ul>
        {#each legacy as n (n)}
          <li><span class="grow">{n}</span><button class="small" onclick={() => importLegacy(n)} disabled={busy}>Import</button></li>
        {/each}
      </ul>
    </section>
  {/if}

  <section>
    <h2 class="section-title">Exercises</h2>
    {#if exercises === null}
      <p class="empty">Loading…</p>
    {:else if exercises.length === 0}
      <p class="empty">No exercises yet. Create one above, or import a file.</p>
    {:else}
      <ul>
        {#each exercises as ex (ex.id)}
          <li>
            <button class="open" onclick={() => open(ex.id)}>
              <strong>{ex.name}</strong>
              <span class="muted">{ex.asset_count} assets · {ex.head} changes · updated {when(ex.updated_at)}</span>
            </button>
            <button class="ghost" title="Duplicate" aria-label={`Duplicate ${ex.name}`} onclick={() => duplicate(ex)}><Icon name="copy" /></button>
            <a class="button ghost" title="Export .coppir file" aria-label={`Export ${ex.name}`} href={api.exportUrl(ex.id)} download><Icon name="download" /></a>
            <button class="ghost danger" title="Delete" aria-label={`Delete ${ex.name}`} onclick={() => remove(ex)}><Icon name="trash-2" /></button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</main>

<style>
  main { max-width: 780px; margin: 0 auto; padding: 48px 16px; display: grid; gap: 28px; }
  h1 { font-family: var(--mono); color: var(--accent); letter-spacing: 0.3em; font-size: 32px; }
  header p { margin: 4px 0 0; }
  .actions { display: grid; gap: 10px; }
  form { display: flex; gap: 8px; }
  .import { position: relative; display: inline-flex; align-items: center; gap: 8px; color: var(--text); font-size: 14px; cursor: pointer; justify-self: start; }
  .import input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
  ul { list-style: none; margin: 8px 0 0; padding: 0; display: grid; gap: 8px; }
  li { display: flex; gap: 4px; align-items: stretch; }
  .open { flex: 1; display: grid; text-align: left; gap: 2px; padding: 10px 12px; background: var(--panel); }
  .open span { font-size: 12px; }
  .legacy { border: 1px dashed var(--line); border-radius: var(--radius); padding: 12px; }
  .legacy li { align-items: center; }
  :global(a.button) {
    display: inline-flex; align-items: center; justify-content: center; padding: 6px 10px; color: inherit;
    border: 1px solid transparent; border-radius: var(--radius);
  }
  :global(a.button:hover), :global(label.button:hover) { border-color: var(--line); }
  :global(label.button) { border: 1px solid var(--line); border-radius: var(--radius); padding: 6px 10px; background: var(--panel-2); }
</style>
