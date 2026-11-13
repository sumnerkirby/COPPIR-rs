<script lang="ts">
  import ExercisePicker from './components/ExercisePicker.svelte';
  import Join from './components/Join.svelte';
  import Dialogs from './components/ui/Dialogs.svelte';
  import Toasts from './components/ui/Toasts.svelte';
  import { router } from './lib/router.svelte';
  import { session } from './lib/session.svelte';

  session.load();
  const me = $derived(session.me);
  // /join always shows the join page, so the host can try the player's side.
  const joining = $derived(router.path.startsWith('/join'));
</script>

{#if me === undefined}
  <p class="loading">{session.error || 'Loading…'}</p>
{:else if me === null || joining}
  <Join />
{:else if me.role === 'facilitator'}
  {#if router.exerciseId}
    <!-- The console brings the map libraries; load them only when one opens. -->
    {#await import('./components/Console.svelte')}
      <p class="loading">Loading…</p>
    {:then { default: Console }}
      {#key router.exerciseId}
        <Console id={router.exerciseId} />
      {/key}
    {:catch}
      <p class="loading">Could not load the console. Reload the page to try again.</p>
    {/await}
  {:else}
    <ExercisePicker />
  {/if}
{:else if me.exercise_id}
  {#await me.role === 'observer' ? import('./components/play/Screen.svelte') : import('./components/play/Play.svelte')}
    <p class="loading">Loading…</p>
  {:then { default: View }}
    {#key `${me.exercise_id}:${me.team}`}
      <View {me} />
    {/key}
  {:catch}
    <p class="loading">Could not load. Reload the page to try again.</p>
  {/await}
{/if}
<Dialogs />
<Toasts />

<style>
  .loading { padding: 24px; color: var(--muted); }
</style>
