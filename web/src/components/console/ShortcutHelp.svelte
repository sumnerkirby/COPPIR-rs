<script lang="ts">
  import Modal from '../ui/Modal.svelte';
  let { onclose }: { onclose: () => void } = $props();
  const mod = /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘' : 'Ctrl';
  const rows: [string, string][] = [
    [`${mod}+Z`, 'Undo the last change (any kind)'],
    [`${mod}+K`, 'Find an asset'],
    [`${mod}+L`, 'Decision log'],
    ['Enter (in Find)', 'Zoom the map to the matching assets'],
    ['A', 'Add an asset: click the map to place it'],
    ['M', 'Measure: click points; double-click or Esc to finish'],
    ['Right-click map', 'Add here, search OSM here, measure from here'],
    ['Esc', 'Cancel the current tool, or clear the selection'],
    ['?', 'This list'],
  ];
</script>

<Modal title="Keyboard shortcuts" {onclose}>
  <table>
    <tbody>
      {#each rows as [k, v] (k)}<tr><td><kbd>{k}</kbd></td><td>{v}</td></tr>{/each}
    </tbody>
  </table>
</Modal>

<style>
  table { border-collapse: collapse; }
  td { padding: 5px 8px; vertical-align: top; }
  td:first-child { white-space: nowrap; }
</style>
