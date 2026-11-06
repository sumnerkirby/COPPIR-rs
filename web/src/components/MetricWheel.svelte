<script lang="ts">
  import { scoreColor } from '../lib/sitrep';
  let { value, label, size = 40, alert = false }: { value: number | null; label: string; size?: number; alert?: boolean } = $props();
  const C = 2 * Math.PI * 16;
</script>

<figure title="{label}: {value === null ? 'no assets' : `${Math.round(value)}%`}">
  <svg viewBox="0 0 40 40" width={size} height={size} aria-hidden="true">
    <circle cx="20" cy="20" r="16" class="track" class:alert />
    <circle cx="20" cy="20" r="16" class="arc" stroke={scoreColor(value)} stroke-dasharray="{((value ?? 0) / 100) * C} {C}" />
    <text x="20" y="24">{value === null ? '–' : Math.round(value)}</text>
  </svg>
  <figcaption>{label}</figcaption>
</figure>

<style>
  figure { margin: 0; display: grid; justify-items: center; }
  figcaption { font-size: 9px; color: var(--muted); text-transform: uppercase; max-width: 72px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .track { fill: none; stroke: var(--line); stroke-width: 4; }
  .track.alert { stroke: var(--danger); stroke-width: 6; opacity: 0.6; }
  .arc { fill: none; stroke-width: 4; transform: rotate(-90deg); transform-origin: center; }
  text { fill: var(--text); font: 600 11px var(--mono); text-anchor: middle; }
</style>
