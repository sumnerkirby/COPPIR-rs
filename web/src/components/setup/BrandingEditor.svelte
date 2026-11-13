<script lang="ts">
  // How the exercise looks on every screen: theme, accent, organisation,
  // logo and banner. Changes apply everywhere at once.
  import { untrack } from 'svelte';
  import { THEMES } from '../../lib/theme';
  import { ui } from '../../lib/ui.svelte';
  import { useWorkspace } from '../../lib/workspace.svelte';
  import type { Branding } from '../../lib/types/Branding';
  import type { Theme } from '../../lib/types/Theme';

  const ws = useWorkspace();
  const ex = $derived(ws.exercise!);
  let b = $state<Branding>(untrack(() => structuredClone($state.snapshot(ws.exercise!.branding))));
  let dirty = $state(false);
  $effect(() => {
    const current = ex.branding;
    if (!dirty) b = structuredClone($state.snapshot(current));
  });

  const MAX_LOGO = 256 * 1024;
  async function pickLogo(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    if (file.size > MAX_LOGO) {
      ui.toast('That image is over 256 KB. Use a smaller logo.', 'warning');
      return;
    }
    b.logo = await new Promise<string>((resolve) => {
      const r = new FileReader();
      r.onload = () => resolve(String(r.result));
      r.readAsDataURL(file);
    });
    dirty = true;
  }

  async function save() {
    if (await ws.send({ type: 'set_branding', branding: b })) dirty = false;
  }
</script>

<section oninput={() => (dirty = true)} onchange={() => (dirty = true)}>
  <h3 class="section-title">Look and banner</h3>
  <div class="themes" role="radiogroup" aria-label="Theme">
    {#each Object.entries(THEMES) as [id, t] (id)}
      <button
        role="radio"
        aria-checked={b.theme === id}
        class:on={b.theme === id}
        style:background={t.tokens.bg}
        style:color={t.tokens.text}
        style:border-color={b.theme === id ? t.tokens.accent : t.tokens.line}
        onclick={() => { b.theme = id as Theme; dirty = true; }}
      ><span class="swatch" style:background={t.tokens.accent}></span>{t.label}</button>
    {/each}
  </div>
  <div class="row">
    <label class="check"><input type="checkbox" checked={!!b.accent} onchange={(e) => (b.accent = e.currentTarget.checked ? '#ff6600' : null)} /> Own accent colour</label>
    {#if b.accent}<input type="color" bind:value={b.accent} aria-label="Accent colour" />{/if}
  </div>
  <label>Organisation <input bind:value={b.org} maxlength="100" placeholder="Shown beside COPPIR" /></label>
  <div class="row">
    {#if b.logo}<img src={b.logo} alt="Logo" class="logo" />{/if}
    <label class="button small">{b.logo ? 'Change logo' : 'Add logo'}<input type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml" onchange={pickLogo} hidden /></label>
    {#if b.logo}<button class="ghost small" onclick={() => { b.logo = null; dirty = true; }}>Remove</button>{/if}
  </div>
  <label class="check">
    <input type="checkbox" checked={!!b.banner} onchange={(e) => (b.banner = e.currentTarget.checked ? { text: 'EXERCISE — EXERCISE — EXERCISE', color: '#b8860b', position: 'both' } : null)} />
    Banner on every screen
  </label>
  {#if b.banner}
    <input bind:value={b.banner.text} maxlength="120" aria-label="Banner text" />
    <div class="row">
      <input type="color" bind:value={b.banner.color} aria-label="Banner colour" />
      <select bind:value={b.banner.position} aria-label="Banner position" class="grow">
        <option value="top">Top</option><option value="bottom">Bottom</option><option value="both">Top and bottom</option>
      </select>
    </div>
  {/if}
  {#if dirty}
    <div class="row end">
      <button class="small" onclick={() => { dirty = false; b = structuredClone($state.snapshot(ex.branding)); }}>Cancel</button>
      <button class="small primary" onclick={save}>Apply to every screen</button>
    </div>
  {/if}
</section>

<style>
  section { display: grid; gap: 8px; border-top: 1px solid var(--line); padding-top: 14px; }
  .themes { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .themes button { display: flex; align-items: center; gap: 8px; border-width: 2px; padding: 8px; }
  .swatch { width: 12px; height: 12px; border-radius: 50%; }
  .check { flex-direction: row; display: flex; align-items: center; gap: 8px; color: var(--text); font-size: 13px; }
  .logo { max-height: 32px; max-width: 120px; }
  .small { font-size: 12px; padding: 3px 8px; }
  .end { justify-content: flex-end; }
</style>
