<script lang="ts">
  import { ui } from '../../lib/ui.svelte';
  import Modal from './Modal.svelte';

  let value = $state('');
  $effect(() => {
    value = ui.dialog?.input?.value ?? '';
  });
</script>

{#if ui.dialog}
  {@const d = ui.dialog}
  <Modal title={d.title} onclose={() => ui.settle(null)}>
    {#if d.body}<p>{d.body}</p>{/if}
    {#if d.input}
      <form id="dialog-form" onsubmit={(e) => { e.preventDefault(); if (value.trim()) ui.settle(value.trim()); }}>
        <!-- svelte-ignore a11y_autofocus -->
        <label>{d.input.label}<input bind:value autofocus maxlength="200" /></label>
      </form>
    {/if}
    {#snippet footer()}
      <button onclick={() => ui.settle(null)}>Cancel</button>
      {#if d.input}
        <button class="primary" type="submit" form="dialog-form" disabled={!value.trim()}>{d.confirm}</button>
      {:else}
        <!-- svelte-ignore a11y_autofocus -->
        <button class={d.danger ? 'danger-solid' : 'primary'} autofocus onclick={() => ui.settle(true)}>{d.confirm}</button>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  p { margin: 0; }
</style>
