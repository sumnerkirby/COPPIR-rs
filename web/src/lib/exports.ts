// Client-side exports: the decision log as CSV and a standalone HTML briefing.
// Both are built from the live exercise, so they match what is on screen.

import { breached, counts, sectorHealth, statusValue } from './sitrep';
import type { Exercise } from './types/Exercise';

export function download(filename: string, text: string, type: string) {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** RFC 4180 quoting, plus a guard against spreadsheet formula injection. */
export function csvField(v: unknown): string {
  let s = v === null || v === undefined ? '' : String(v);
  if (/^[=+\-@\t\r]/.test(s)) s = `'${s}`;
  return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

export function logCsv(ex: Exercise): string {
  const rows = [['Timestamp', 'Entry', 'Asset', 'Notes', 'Automatic']];
  for (const e of ex.log) rows.push([e.at, e.text, e.asset_name ?? '', e.notes, e.auto ? 'yes' : 'no']);
  return rows.map((r) => r.map(csvField).join(',')).join('\r\n') + '\r\n';
}

export function fileStem(ex: Exercise): string {
  return ex.name.replace(/[^\w\- ]+/g, '_').trim() || 'exercise';
}

const esc = (s: unknown) =>
  String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

/** A self-contained, printable situation briefing. */
export function briefingHtml(ex: Exercise, now = new Date()): string {
  const health = sectorHealth(ex);
  const alerts = breached(ex, health);
  const sets = ex.taxonomy.status_sets;
  const assets = Object.values(ex.assets);
  const typeName = (id: string) => ex.taxonomy.asset_types.find((t) => t.id === id)?.name ?? id;

  const statusTables = sets
    .map(
      (set) => `<h3>${esc(set.name)}</h3><table><tr>${counts(ex, set)
        .map(({ value, count }) => `<td><span class="dot" style="background:${esc(value.color)}"></span>${esc(value.label)}<b>${count}</b></td>`)
        .join('')}</tr></table>`,
    )
    .join('');

  const sectorRows = health
    .map(
      (s) =>
        `<tr><td>${esc(s.name)}</td><td>${s.score === null ? '—' : `${Math.round(s.score)}%`}</td><td>${s.assets}</td></tr>`,
    )
    .join('');

  const assetRows = assets
    .map(
      (a) =>
        `<tr><td>${esc(a.name)}</td><td>${esc(typeName(a.type_id))}</td>${sets
          .map((set) => `<td>${esc(statusValue(set, a)?.label ?? '')}</td>`)
          .join('')}<td>${esc(a.notes)}</td></tr>`,
    )
    .join('');

  const logRows = ex.log
    .slice(-30)
    .reverse()
    .map(
      (e) =>
        `<tr><td>${esc(new Date(e.at).toLocaleString())}</td><td>${esc(e.text)}${e.auto ? ' <em>(auto)</em>' : ''}</td><td>${esc(e.asset_name ?? '')}</td></tr>`,
    )
    .join('');

  const metricRows = ex.metrics.map((m) => `<tr><td>${esc(m.name)}</td><td>${m.value}%</td></tr>`).join('');

  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Briefing — ${esc(ex.name)}</title>
<style>
body{font:14px/1.45 system-ui,sans-serif;margin:32px;color:#111}h1{margin:0}h2{margin-top:28px;border-bottom:2px solid #111}
table{border-collapse:collapse;width:100%;margin:8px 0}td,th{border:1px solid #ccc;padding:4px 8px;text-align:left;vertical-align:top}
th{background:#f2f2f2}.dot{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:6px}
td b{float:right}.alert{background:#fde8e8;border:1px solid #e88;padding:6px 10px;margin:4px 0}.meta{color:#555}
@media print{body{margin:12mm}h2{break-after:avoid}tr{break-inside:avoid}}
</style></head><body>
<h1>${esc(ex.name)}</h1><p class="meta">Situation briefing · ${esc(now.toLocaleString())} · ${assets.length} assets</p>
${alerts.map((t) => `<div class="alert">⚠ ${esc(t.name)} (${esc(t.severity)})</div>`).join('')}
<h2>Status</h2>${statusTables}
<h2>Sector integrity</h2><table><tr><th>Sector</th><th>Integrity</th><th>Assets</th></tr>${sectorRows}</table>
${metricRows ? `<h2>Metrics</h2><table>${metricRows}</table>` : ''}
<h2>Assets</h2><table><tr><th>Name</th><th>Type</th>${sets.map((s) => `<th>${esc(s.name)}</th>`).join('')}<th>Notes</th></tr>${assetRows}</table>
<h2>Recent decisions</h2><table><tr><th>Time</th><th>Entry</th><th>Asset</th></tr>${logRows}</table>
</body></html>`;
}
