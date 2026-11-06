<script lang="ts">
  import { ui } from '../../lib/ui.svelte';
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each ui.toasts as t (t.id)}
    <div class="toast {t.tone}">{t.text}</div>
  {/each}
</div>

<style>
  .toasts { position: fixed; bottom: 16px; left: 50%; transform: translateX(-50%); display: grid; gap: 6px; z-index: 60; pointer-events: none; }
  .toast {
    background: var(--panel-2); border: 1px solid var(--line); border-left: 4px solid var(--accent);
    padding: 8px 14px; border-radius: var(--radius); box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5); max-width: 520px;
  }
  .toast.warning { border-left-color: var(--warn); }
  .toast.critical { border-left-color: var(--danger); }
  .toast.ok { border-left-color: var(--accent); }
</style>
