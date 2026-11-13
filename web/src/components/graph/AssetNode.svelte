<script lang="ts">
  // An asset on the topology canvas, drawn like its map marker: fill and glyph
  // for the fill status set, ring colour and style for the ring set, and the
  // type's icon in the middle.
  import { Handle, Position } from '@xyflow/svelte';
  import Icon from '../ui/Icon.svelte';

  interface Data {
    name: string;
    icon: string;
    fill: string;
    ring: string;
    ringStyle: string;
    glyph: string;
    status: string;
    connectable: boolean;
  }
  let { data, selected }: { data: Data; selected?: boolean } = $props();
  const border = $derived(['solid', 'dashed', 'dotted', 'double'].includes(data.ringStyle) ? data.ringStyle : 'solid');
</script>

<div class="node" class:selected title="{data.name}: {data.status}">
  <Handle type="target" position={Position.Top} isConnectable={data.connectable} />
  <div class="mark" style:background={data.fill} style:border-color={data.ring} style:border-style={border}>
    <Icon name={data.icon} size={18} />
    {#if data.glyph}<span class="glyph" style:border-color={data.fill}>{data.glyph}</span>{/if}
  </div>
  <span class="label">{data.name}</span>
  <Handle type="source" position={Position.Bottom} isConnectable={data.connectable} />
</div>

<style>
  .node { width: 150px; display: grid; justify-items: center; gap: 4px; padding-top: 2px; }
  .mark {
    position: relative; width: 42px; height: 42px; border-radius: 50%; border-width: 4px;
    display: grid; place-items: center; color: #fff; box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.6);
  }
  .selected .mark { box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.55); }
  .glyph {
    position: absolute; top: -8px; right: -10px; width: 18px; height: 18px; border-radius: 50%; display: grid; place-items: center;
    background: #0b0f0c; border: 1.5px solid; color: #fff; font: bold 10px system-ui, sans-serif;
  }
  .label {
    max-width: 150px; text-align: center; font-size: 11px; line-height: 1.2; color: var(--text);
    overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical;
  }
</style>
