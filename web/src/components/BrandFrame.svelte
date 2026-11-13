<script lang="ts">
  // Wraps a screen in the exercise's look: theme tokens on the page, and the
  // exercise banner above and/or below. Leaving restores the default theme.
  import type { Snippet } from 'svelte';
  import { applyTheme, inkFor } from '../lib/theme';
  import type { Branding } from '../lib/types/Branding';

  let { branding, children }: { branding: Branding | undefined; children: Snippet } = $props();
  const banner = $derived(branding?.banner ?? null);

  $effect(() => {
    applyTheme(branding ?? { theme: 'terminal', accent: null });
    return () => applyTheme({ theme: 'terminal', accent: null });
  });
</script>

<div class="frame">
  {#if banner && banner.position !== 'bottom'}
    <div class="banner" style:background={banner.color} style:color={inkFor(banner.color)} role="note">{banner.text}</div>
  {/if}
  <div class="content">{@render children()}</div>
  {#if banner && banner.position !== 'top'}
    <div class="banner" style:background={banner.color} style:color={inkFor(banner.color)} role="note">{banner.text}</div>
  {/if}
</div>

<style>
  .frame { display: flex; flex-direction: column; height: 100%; background: var(--bg); }
  .content { flex: 1; min-height: 0; }
  .banner {
    flex: none; text-align: center; font: 700 12px/1.6 var(--font); letter-spacing: 0.25em; text-transform: uppercase;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding: 0 8px;
  }
</style>
