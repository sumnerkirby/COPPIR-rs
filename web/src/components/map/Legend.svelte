<script lang="ts">
  // The key, generated from the exercise's taxonomy so it always matches the
  // markers. Ring styles reuse CSS border styles, which have the same names.
  import { useWorkspace } from '../../lib/workspace.svelte';

  const ws = useWorkspace();
  const sets = $derived(ws.exercise!.taxonomy.status_sets);
  const fill = $derived(sets.find((s) => s.display === 'fill'));
  const ring = $derived(sets.find((s) => s.display === 'ring'));
  const border = (glyph: string) => (['solid', 'dashed', 'dotted', 'double'].includes(glyph) ? glyph : 'solid');
</script>

<div class="legend" aria-label="Legend">
  {#if fill}
    <p class="section-title">{fill.name} · fill</p>
    {#each fill.values as v (v.id)}
      <div class="row"><span class="dot" style:background={v.color}>{v.glyph}</span>{v.label}</div>
    {/each}
  {/if}
  {#if ring}
    <p class="section-title">{ring.name} · ring</p>
    {#each ring.values as v (v.id)}
      <div class="row"><span class="ring" style:border-color={v.color} style:border-style={border(v.glyph)}></span>{v.label}</div>
    {/each}
  {/if}
  <p class="section-title">Links</p>
  <div class="row"><span class="line"></span>connects to</div>
  <div class="row"><span class="line dep"></span>depends on</div>
  <button class="ghost close" onclick={() => (ws.layers.legend = false)} aria-label="Hide legend">✕</button>
</div>

<style>
  .legend {
    position: absolute; left: 10px; bottom: 30px; z-index: 10; display: grid; gap: 3px; font-size: 12px;
    background: color-mix(in srgb, var(--panel) 94%, transparent); border: 1px solid var(--line); border-radius: var(--radius); padding: 8px 10px 8px 10px; min-width: 160px;
  }
  .section-title { margin-top: 4px; font-size: 10px; }
  .dot { width: 16px; height: 16px; border-radius: 50%; display: inline-grid; place-items: center; color: #000; font-size: 10px; font-weight: 700; }
  .ring { width: 16px; height: 16px; border-radius: 50%; border-width: 3px; }
  .line { width: 18px; border-top: 2px solid #00ff41; }
  .line.dep { border-top: 2px dashed #ffaa00; }
  .close { position: absolute; top: 2px; right: 2px; padding: 0 5px; font-size: 11px; }
</style>
