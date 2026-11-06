import { expect, it } from 'vitest';
import { slug, uniqueSlug } from './slug';

it('matches the server slug rule', () => {
  expect(slug('Government Buildings')).toBe('government_buildings');
  expect(slug('  Wi-Fi / LAN  ')).toBe('wi_fi_lan');
  expect(slug('!!!')).toBe('item');
  expect(uniqueSlug('Asset', ['asset', 'asset_'])).toBe('asset__');
});
