// Small geometry helpers. Positions are [lon, lat], as in GeoJSON.

export type Position = [number, number];

const R_MILES = 3958.8;

/** Great-circle distance in miles. */
export function distanceMi(a: Position, b: Position): number {
  const rad = Math.PI / 180;
  const dLat = (b[1] - a[1]) * rad;
  const dLon = (b[0] - a[0]) * rad;
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(a[1] * rad) * Math.cos(b[1] * rad) * Math.sin(dLon / 2) ** 2;
  return 2 * R_MILES * Math.asin(Math.min(1, Math.sqrt(h)));
}

export function pathMi(points: Position[]): number {
  let total = 0;
  for (let i = 1; i < points.length; i++) total += distanceMi(points[i - 1]!, points[i]!);
  return total;
}

export function formatMi(mi: number): string {
  if (mi < 0.1) return `${Math.round(mi * 5280)} ft`;
  return `${mi < 10 ? mi.toFixed(2) : mi.toFixed(1)} mi`;
}

/** Convex hull (Andrew's monotone chain), closed ring for GeoJSON. */
export function hull(points: Position[]): Position[] {
  const pts = [...new Map(points.map((p) => [`${p[0]},${p[1]}`, p])).values()].sort(
    (a, b) => a[0] - b[0] || a[1] - b[1],
  );
  if (pts.length < 3) return [];
  const cross = (o: Position, a: Position, b: Position) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const lower: Position[] = [];
  for (const p of pts) {
    while (lower.length >= 2 && cross(lower[lower.length - 2]!, lower[lower.length - 1]!, p) <= 0) lower.pop();
    lower.push(p);
  }
  const upper: Position[] = [];
  for (const p of [...pts].reverse()) {
    while (upper.length >= 2 && cross(upper[upper.length - 2]!, upper[upper.length - 1]!, p) <= 0) upper.pop();
    upper.push(p);
  }
  const ring = [...lower.slice(0, -1), ...upper.slice(0, -1)];
  return [...ring, ring[0]!];
}

export function centroid(points: Position[]): Position {
  const n = points.length || 1;
  return [points.reduce((s, p) => s + p[0], 0) / n, points.reduce((s, p) => s + p[1], 0) / n];
}

/** A circle of `radiusMi` around `center`, as a closed ring of `steps` points. */
export function circle(center: Position, radiusMi: number, steps = 48): Position[] {
  const dLat = radiusMi / 69.0;
  const dLon = dLat / Math.cos((center[1] * Math.PI) / 180);
  const ring: Position[] = [];
  for (let i = 0; i <= steps; i++) {
    const a = (i / steps) * 2 * Math.PI;
    ring.push([center[0] + dLon * Math.cos(a), center[1] + dLat * Math.sin(a)]);
  }
  return ring;
}
