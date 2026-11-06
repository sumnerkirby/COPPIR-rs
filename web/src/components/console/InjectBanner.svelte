<script lang="ts">
  // A banner across the map when an inject fires, from any window.
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Inject } from '../../lib/types/Inject';

  const ws = useWorkspace();
  let shown = $state<Inject | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() =>
    ws.live.on((env) => {
      if (env.event.type !== 'inject_fired') return;
      shown = env.event.inject;
      clearTimeout(timer);
      timer = setTimeout(() => (shown = null), 8000);
    }),
  );
</script>

{#if shown}
  <div class="banner {shown.severity}" role="alert">
    <strong>INJECT · {shown.title}</strong>
    {#if shown.body}<span>{shown.body}</span>{/if}
    <button class="ghost" onclick={() => (shown = null)} aria-label="Dismiss">✕</button>
  </div>
{/if}

<style>
  .banner {
    position: absolute; top: 12px; left: 50%; transform: translateX(-50%); z-index: 20; width: min(640px, calc(100% - 24px));
    display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; padding: 10px 14px; border-radius: var(--radius);
    background: #0c1f2e; border: 1px solid #3399ff; box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
  }
  .banner.warning { background: #2e2200; border-color: var(--warn); }
  .banner.critical { background: #2e0909; border-color: var(--danger); }
  strong { letter-spacing: 0.04em; }
  span { grid-column: 1; font-size: 13px; }
  button { grid-row: 1 / span 2; grid-column: 2; align-self: start; }
</style>
