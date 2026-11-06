<script lang="ts">
  // A dialog that traps Tab, closes on Escape and backdrop click, and gives
  // focus back to whatever opened it.
  import type { Snippet } from 'svelte';
  import { onMount } from 'svelte';

  let {
    title,
    onclose,
    wide = false,
    children,
    footer,
  }: { title: string; onclose: () => void; wide?: boolean; children: Snippet; footer?: Snippet } = $props();

  let box: HTMLDivElement;
  const opener = document.activeElement as HTMLElement | null;

  const focusable = () =>
    [...box.querySelectorAll<HTMLElement>('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])')].filter(
      (el) => !el.hasAttribute('disabled'),
    );

  onMount(() => {
    (box.querySelector<HTMLElement>('[autofocus]') ?? focusable()[0] ?? box).focus();
    return () => opener?.focus?.();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    } else if (e.key === 'Tab') {
      const els = focusable();
      if (!els.length) return;
      const first = els[0]!;
      const last = els[els.length - 1]!;
      if (e.shiftKey && document.activeElement === first) {
        last.focus();
        e.preventDefault();
      } else if (!e.shiftKey && document.activeElement === last) {
        first.focus();
        e.preventDefault();
      }
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" class:wide role="dialog" aria-modal="true" aria-label={title} tabindex="-1" bind:this={box} {onkeydown}>
    <header>
      <h2>{title}</h2>
      <button class="ghost" onclick={onclose} aria-label="Close">✕</button>
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed; inset: 0; background: rgba(0, 0, 0, 0.6); z-index: 50;
    display: grid; place-items: center; padding: 16px;
  }
  .modal {
    background: var(--panel); border: 1px solid var(--line); border-radius: 10px;
    width: min(460px, 100%); max-height: calc(100vh - 32px); display: grid; grid-template-rows: auto 1fr auto;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
  }
  .modal.wide { width: min(980px, 100%); }
  .modal:focus { outline: none; }
  header { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid var(--line); }
  h2 { font-size: 15px; }
  .body { overflow: auto; padding: 16px; display: grid; gap: 12px; align-content: start; }
  footer { display: flex; gap: 8px; justify-content: flex-end; padding: 12px 16px; border-top: 1px solid var(--line); }
</style>
