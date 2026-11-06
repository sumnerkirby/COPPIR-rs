import { describe, expect, it } from 'vitest';
import { centroid, circle, distanceMi, formatMi, hull, type Position } from './geo';

describe('geo', () => {
  it('measures great-circle distance', () => {
    // Washington Monument to the Capitol: about 1.4 miles.
    expect(distanceMi([-77.0353, 38.8895], [-77.0091, 38.8899])).toBeCloseTo(1.41, 1);
    expect(formatMi(0.05)).toBe('264 ft');
    expect(formatMi(3.14159)).toBe('3.14 mi');
  });

  it('builds a closed convex hull and ignores interior points', () => {
    const pts: Position[] = [[0, 0], [2, 0], [2, 2], [0, 2], [1, 1], [1, 1]];
    const ring = hull(pts);
    expect(ring[0]).toEqual(ring[ring.length - 1]);
    expect(ring).toHaveLength(5);
    expect(ring).not.toContainEqual([1, 1]);
    expect(hull([[0, 0], [1, 1]])).toEqual([]);
  });

  it('centroid and circle', () => {
    expect(centroid([[0, 0], [2, 2]])).toEqual([1, 1]);
    const ring = circle([-77, 38.9], 1);
    expect(ring[0]).toEqual(ring[ring.length - 1]);
    for (const p of ring) expect(distanceMi([-77, 38.9], p)).toBeCloseTo(1, 1);
  });
});
