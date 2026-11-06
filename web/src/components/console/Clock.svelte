<script lang="ts">
  // The exercise clock: elapsed time since START, pausable. Kept per exercise
  // in this browser; a shared, server-side clock arrives with scripted
  // exercises (milestone 4).
  import { untrack } from 'svelte';
  import { load, save } from '../../lib/prefs';
  import Icon from '../ui/Icon.svelte';

  let { id }: { id: string } = $props();

  interface State { startedAt: number | null; banked: number }
  // Console is re-created per exercise, so the id never changes here.
  const key = untrack(() => `clock:${id}`);
  let st = $state<State>(load(key, { startedAt: null, banked: 0 }));
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 500);
    return () => clearInterval(t);
  });
  $effect(() => save(key, st));

  const elapsed = $derived(st.banked + (st.startedAt ? now - st.startedAt : 0));
  const fmt = (ms: number) => {
    const s = Math.floor(ms / 1000);
    return [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60].map((n) => String(n).padStart(2, '0')).join(':');
  };

  function toggle() {
    st = st.startedAt ? { startedAt: null, banked: elapsed } : { ...st, startedAt: Date.now() };
  }
  function reset() {
    st = { startedAt: null, banked: 0 };
  }
</script>

<div class="clock" role="timer" aria-label="Exercise clock">
  <span class="mono" class:running={!!st.startedAt}>T+{fmt(elapsed)}</span>
  <button class="ghost small" onclick={toggle} aria-label={st.startedAt ? 'Pause clock' : 'Start clock'}>
    <Icon name={st.startedAt ? 'pause' : 'play'} size={14} />
  </button>
  {#if elapsed > 0 && !st.startedAt}
    <button class="ghost small" onclick={reset} aria-label="Reset clock"><Icon name="rotate-ccw" size={14} /></button>
  {/if}
  <span class="mono muted wall">{new Date(now).toLocaleTimeString()}</span>
</div>

<style>
  .clock { display: flex; align-items: center; gap: 2px; }
  .mono { font-size: 13px; }
  .running { color: var(--accent); }
  .wall { margin-left: 8px; font-size: 12px; }
  @media (max-width: 700px) { .wall { display: none; } }
</style>
