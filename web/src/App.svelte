<script lang="ts">
  import ExercisePicker from './components/ExercisePicker.svelte';
  import Console from './components/Console.svelte';

  // #/ → picker, #/x/<id> → console. A hash router keeps every URL valid
  // without the server knowing the client's routes.
  let hash = $state(location.hash);
  $effect(() => {
    const onHash = () => (hash = location.hash);
    addEventListener('hashchange', onHash);
    return () => removeEventListener('hashchange', onHash);
  });
  const exerciseId = $derived(hash.match(/^#\/x\/([\w-]+)/)?.[1] ?? null);
</script>

{#if exerciseId}
  {#key exerciseId}
    <Console id={exerciseId} />
  {/key}
{:else}
  <ExercisePicker />
{/if}
