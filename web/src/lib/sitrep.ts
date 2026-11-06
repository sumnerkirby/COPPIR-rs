// Derived numbers for the SITREP strip: counts per status, sector health and
// breached thresholds. All of it comes from the exercise's own taxonomy, so a
// custom status set or sector shows up without code changes.

import type { Asset } from './types/Asset';
import type { Exercise } from './types/Exercise';
import type { StatusSet } from './types/StatusSet';
import type { StatusValue } from './types/StatusValue';
import type { Threshold } from './types/Threshold';

export function statusValue(set: StatusSet, asset: Asset): StatusValue | undefined {
  const id = asset.statuses[set.id] ?? set.default;
  return set.values.find((v) => v.id === id);
}

export function counts(ex: Exercise, set: StatusSet): { value: StatusValue; count: number }[] {
  const by = new Map(set.values.map((v) => [v.id, 0]));
  for (const a of Object.values(ex.assets)) {
    const v = a.statuses[set.id] ?? set.default;
    by.set(v, (by.get(v) ?? 0) + 1);
  }
  return set.values.map((value) => ({ value, count: by.get(value.id) ?? 0 }));
}

export interface SectorHealth {
  id: string;
  name: string;
  color: string;
  /** Mean score 0–100, or null when the sector has no assets. */
  score: number | null;
  assets: number;
}

export function sectorHealth(ex: Exercise): SectorHealth[] {
  const assets = Object.values(ex.assets);
  return ex.taxonomy.sectors.map((s) => {
    const set = ex.taxonomy.status_sets.find((x) => x.id === s.status_set);
    const members = assets.filter((a) => s.asset_types.includes(a.type_id));
    const scores = set ? members.map((a) => statusValue(set, a)?.score ?? 0) : [];
    const score = scores.length ? scores.reduce((a, b) => a + b, 0) / scores.length : null;
    return { id: s.id, name: s.name, color: s.color, score, assets: members.length };
  });
}

/** Thresholds whose sector (or the whole picture) has dropped below them. */
export function breached(ex: Exercise, health: SectorHealth[]): Threshold[] {
  const scored = health.filter((h) => h.score !== null);
  const overall = scored.length ? scored.reduce((a, h) => a + (h.score ?? 0), 0) / scored.length : null;
  return ex.thresholds.filter((t) => {
    const score = t.sector ? health.find((h) => h.id === t.sector)?.score : overall;
    return score !== null && score !== undefined && score < t.below_pct;
  });
}

/** Score 0–100 as a colour from red through amber to green. */
export function scoreColor(score: number | null): string {
  if (score === null) return 'var(--muted)';
  if (score >= 70) return '#33cc33';
  if (score >= 40) return '#ffaa00';
  return '#ff3333';
}
