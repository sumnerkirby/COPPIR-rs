<script lang="ts">
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Annotation } from '../../lib/types/Annotation';

  let { annotation, onclose }: { annotation: Annotation; onclose: () => void } = $props();
  const ws = useWorkspace();
</script>

<div class="panel" role="dialog" aria-label="Annotation">
  <div class="row">
    <strong class="grow">{annotation.shape}</strong>
    <button class="ghost" onclick={onclose} aria-label="Close">✕</button>
  </div>
  <label>Label
    <input value={annotation.label} maxlength="200" onchange={(e) => ws.send({ type: 'edit_annotation', id: annotation.id, label: e.currentTarget.value })} />
  </label>
  <div class="row">
    <label>Colour
      <input type="color" value={annotation.color} onchange={(e) => ws.send({ type: 'edit_annotation', id: annotation.id, color: e.currentTarget.value })} />
    </label>
    <span class="grow"></span>
    <button class="danger" onclick={async () => { if (await ws.send({ type: 'remove_annotations', ids: [annotation.id] })) onclose(); }}>Delete</button>
  </div>
</div>

<style>
  .panel {
    position: absolute; right: 54px; top: 10px; z-index: 14; width: 240px; display: grid; gap: 8px;
    background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius); padding: 10px;
  }
  strong { text-transform: capitalize; }
  .row { align-items: end; }
</style>
