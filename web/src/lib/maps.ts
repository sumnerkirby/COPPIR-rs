// Offline map packs: the tile arithmetic the server uses, so the form can say
// how big a pack will be before anything is downloaded.

import type { Exercise } from './types/Exercise';

export type Bounds = [west: number, south: number, east: number, north: number];

/** Vector tiles stop here; the map over-zooms beyond it. */
export const MAX_ZOOM = 14;
/** The server's limit on one pack. */
export const MAX_TILES = 40_000;
/** A rough average for gzipped OpenMapTiles tiles in a built-up area. */
const BYTES_PER_TILE = 110_000;
/** Fonts, sprites and styles: the same for every pack. */
const PACK_OVERHEAD = 2_500_000;

const tileX = (lon: number, z: number) => {
  const n = 2 ** z;
  return Math.min(n - 1, Math.max(0, Math.floor(((lon + 180) / 360) * n)));
};
const tileY = (lat: number, z: number) => {
  const n = 2 ** z;
  const r = (Math.min(85.0511, Math.max(-85.0511, lat)) * Math.PI) / 180;
  return Math.min(n - 1, Math.max(0, Math.floor(((1 - Math.log(Math.tan(r) + 1 / Math.cos(r)) / Math.PI) / 2) * n)));
};

export function tileCount([w, s, e, n]: Bounds, maxZoom: number): number {
  let total = 0;
  for (let z = 0; z <= maxZoom; z++) total += (tileX(e, z) - tileX(w, z) + 1) * (tileY(s, z) - tileY(n, z) + 1);
  return total;
}

export const estimateBytes = (tiles: number) => PACK_OVERHEAD + tiles * BYTES_PER_TILE;

export function formatBytes(b: number): string {
  if (b < 1024 * 1024) return `${Math.max(1, Math.round(b / 1024))} KB`;
  if (b < 1024 ** 3) return `${(b / 1024 / 1024).toFixed(b < 10 * 1024 * 1024 ? 1 : 0)} MB`;
  return `${(b / 1024 ** 3).toFixed(1)} GB`;
}

/** Remount the map when its look or its tiles change. */
export const mapKey = (ex: Exercise | null | undefined) => `${ex?.branding.theme === 'daylight'}|${ex?.branding.map_pack ?? ''}`;
