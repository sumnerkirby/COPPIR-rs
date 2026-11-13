import { expect, it } from 'vitest';
import { elapsedMs, fmtClock, fmtMinutes } from './clock.svelte';

it('works out elapsed time from the server clock', () => {
  const since = '2026-01-01T10:00:00Z';
  expect(elapsedMs({ elapsed_ms: 60000, running_since: null }, Date.parse(since) + 999999)).toBe(60000);
  expect(elapsedMs({ elapsed_ms: 60000, running_since: since }, Date.parse(since) + 5000)).toBe(65000);
  expect(fmtClock(3723000)).toBe('01:02:03');
  expect(fmtMinutes(45)).toBe('45 min');
  expect(fmtMinutes(90)).toBe('1 h 30 min');
  expect(fmtMinutes(120)).toBe('2 h');
});
