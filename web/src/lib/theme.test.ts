import { expect, it } from 'vitest';
import { THEMES, inkFor, tokensFor } from './theme';

it('every theme defines every token', () => {
  const keys = Object.keys(THEMES.terminal.tokens).sort();
  for (const t of Object.values(THEMES)) expect(Object.keys(t.tokens).sort()).toEqual(keys);
});

it('an accent override brings a readable ink', () => {
  expect(inkFor('#ffff00')).toBe('#000000');
  expect(inkFor('#102030')).toBe('#ffffff');
  const t = tokensFor({ theme: 'daylight', accent: '#ffcc00' });
  expect([t.accent, t['accent-ink'], t.bg]).toEqual(['#ffcc00', '#000000', THEMES.daylight.tokens.bg]);
});
