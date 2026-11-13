import { describe, expect, it } from 'vitest';
import { formatBytes, tileCount } from './maps';

describe('map packs', () => {
  it('counts tiles the way the server does', () => {
    expect(tileCount([-180, -85, 180, 85], 2)).toBe(1 + 4 + 16);
    // Washington DC to zoom 14: the same count as `tile_count` in src/maps.rs.
    expect(tileCount([-77.12, 38.8, -76.91, 38.99], 14)).toBe(185);
  });
  it('formats sizes', () => {
    expect(formatBytes(2048)).toBe('2 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(formatBytes(250 * 1024 * 1024)).toBe('250 MB');
  });
});
