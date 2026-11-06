<script lang="ts">
  import { useWorkspace } from '../../lib/workspace.svelte';
  import AssetEditor from './AssetEditor.svelte';
  import AssetList from './AssetList.svelte';
  import NewAssetForm from './NewAssetForm.svelte';
  import OsmImport from './OsmImport.svelte';

  const ws = useWorkspace();
  const selected = $derived(ws.selected ? ws.exercise?.assets[ws.selected] : undefined);
</script>

{#if ws.placing}
  <NewAssetForm at={ws.placing} />
{:else if selected}
  <AssetEditor asset={selected} />
{:else}
  <AssetList />
  <OsmImport />
{/if}
