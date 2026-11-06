<script lang="ts">
  import { iconNode } from '../../lib/icons';

  let { name, size = 16, label }: { name: string; size?: number; label?: string } = $props();
  const node = $derived(iconNode(name));
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  {#each node as [tag, attrs], i (i)}
    <svelte:element this={tag} {...attrs} />
  {/each}
</svg>

<style>
  svg { flex: none; vertical-align: middle; }
</style>
