import { describe, expect, it } from 'vitest';
import { markerId, parseMarkerId, type MarkerSpec } from './markers';
import { hasIcon, iconNames, iconNode } from './icons';

describe('marker ids', () => {
  it('round-trip every field, including awkward glyphs', () => {
    const spec: MarkerSpec = {
      icon: 'radio-tower', fill: '#ff3333', ring: '#ffd700', ringStyle: 'dashed', glyph: '✕|?', selected: true,
    };
    expect(parseMarkerId(markerId(spec))).toEqual(spec);
    expect(parseMarkerId('some-style-sprite')).toBeNull();
  });
});

describe('icons', () => {
  it('resolve kebab-case names and fall back to a circle', () => {
    expect(iconNode('radio-tower').length).toBeGreaterThan(1);
    expect(iconNode('no-such-icon')).toEqual(iconNode('circle'));
    const names = iconNames();
    expect(names).toContain('graduation-cap');
    expect(names).toContain('map-pin');
    // Every listed name maps back to its icon.
    const broken = names.filter((n) => !hasIcon(n));
    expect(broken).toEqual([]);
  });
});
