<script lang="ts">
  import { useWorkspace } from '../lib/workspace.svelte';
  import Icon from './ui/Icon.svelte';
  const ws = useWorkspace();
</script>

<div class="switch" role="radiogroup" aria-label="View">
  <button role="radio" aria-checked={ws.view === 'map'} class:on={ws.view === 'map'} onclick={() => (ws.view = 'map')}><Icon name="map" size={14} /> Map</button>
  <button role="radio" aria-checked={ws.view === 'graph'} class:on={ws.view === 'graph'} onclick={() => (ws.view = 'graph')}><Icon name="network" size={14} /> Graph</button>
</div>

<style>
  .switch { position: absolute; top: 10px; left: 50%; transform: translateX(-50%); z-index: 13; display: flex; box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4); }
  button { display: inline-flex; align-items: center; gap: 6px; border-radius: 0; padding: 5px 12px; font-size: 13px; background: var(--panel); }
  button:first-child { border-radius: var(--radius) 0 0 var(--radius); }
  button:last-child { border-radius: 0 var(--radius) var(--radius) 0; border-left: 0; }
  button.on { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); }
</style>
