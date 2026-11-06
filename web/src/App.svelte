<script lang="ts">
  import ExercisePicker from './components/ExercisePicker.svelte';
  import Console from './components/Console.svelte';
  import Dialogs from './components/ui/Dialogs.svelte';
  import Toasts from './components/ui/Toasts.svelte';
  import { router } from './lib/router.svelte';
</script>

{#if router.exerciseId}
  {#key router.exerciseId}
    <Console id={router.exerciseId} />
  {/key}
{:else}
  <ExercisePicker />
{/if}
<Dialogs />
<Toasts />
