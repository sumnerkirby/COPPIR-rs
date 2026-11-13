import { describe, expect, it } from 'vitest';
import { buildReplay, countAt, stateAt } from './replay';
import type { Asset } from './types/Asset';
import type { Envelope } from './types/Envelope';
import type { Event } from './types/Event';
import type { Exercise } from './types/Exercise';
import type { Poll } from './types/Poll';

const blank = (): Exercise => ({
  id: 'x',
  name: 'Drill',
  description: '',
  taxonomy: { asset_types: [], status_sets: [], sectors: [], fields: [] },
  teams: [{ id: 'it', name: 'IT', color: '#00ff00' }],
  assets: {},
  links: {},
  injects: {},
  thresholds: [],
  log: [],
  annotations: {},
  metrics: [],
  clock: { elapsed_ms: 0, running_since: null },
  phases: [{ id: 'p1', name: 'Detect', color: '#3399ff', target_minutes: 10 }],
  phase_log: [],
  polls: {},
  impact_rules: [],
  branding: { theme: 'terminal', accent: null, org: '', logo: null, banner: null, map_pack: null },
  review: { summary: '', strengths: '', improvements: '' },
  created_at: 't0',
  updated_at: 't0',
});

const asset = (id: string, security = 'clean'): Asset => ({
  id, name: id, type_id: 'asset', geo: null, graph: null, statuses: { security }, history: [], fields: {}, tags: [], notes: '',
  color: null, visibility: { mode: 'all' }, created_at: 't0',
});

const poll = (state: Poll['state'], responses: Poll['responses'] = [], results = {}): Poll => ({
  id: 'q', question: 'Pay?', options: [{ id: 'y', label: 'Yes' }, { id: 'n', label: 'No' }], allow_text: false,
  audience: { mode: 'all' }, state, duration_minutes: null, opened_at: null, closes_at: null, closed_at: null, responses, results,
});

// Minute m after the start, at 10:00.
const at = (m: number) => new Date(Date.UTC(2026, 0, 1, 10, 0) + m * 60_000).toISOString();
let seq = 0;
const env = (m: number, event: Event, actor = 'facilitator'): Envelope => ({ seq: ++seq, at: at(m), actor, event });

function log(): Envelope[] {
  const inject = {
    id: 'i1', title: 'Ransom note', body: '', severity: 'critical' as const, audience: { mode: 'all' as const }, effects: [],
    notes: '', created_at: 't0', fired_at: at(5), trigger: { type: 'manual' as const }, armed: true, expected: [],
  };
  const entry = (id: string, text: string, auto: boolean, visibility: Exercise['log'][0]['visibility'], author = '') => ({
    id, at: '', text, asset: null, asset_name: null, notes: '', auto, author, visibility,
  });
  const vote = { participant: 'p1', name: 'Dana', team: 'it', option: 'n', text: '', at: at(12) };
  return [
    env(-60, { type: 'created', exercise: blank() }),
    env(-30, { type: 'assets_upserted', assets: [asset('a'), asset('b')] }),
    env(-20, { type: 'assets_upserted', assets: [asset('a', 'suspicious')] }), // setup: not a mark
    env(0, { type: 'clock_changed', clock: { elapsed_ms: 0, running_since: at(0) } }),
    env(1, { type: 'phase_entered', entry: { phase: 'p1', at: at(1), elapsed_ms: 60_000 } }),
    env(5, { type: 'inject_fired', inject, assets: [asset('b', 'compromised')], entry: entry('l1', 'INJECT', true, { mode: 'all' }) }, 'engine'),
    // Paused from minute 6 to minute 36: thirty minutes that do not count.
    env(6, { type: 'clock_changed', clock: { elapsed_ms: 360_000, running_since: null } }),
    env(36, { type: 'clock_changed', clock: { elapsed_ms: 360_000, running_since: at(36) } }),
    env(40, { type: 'logged', entry: entry('l2', 'Isolated the server', false, { mode: 'teams', teams: ['it'] }, 'Dana (IT)') }, 'player:p1'),
    env(41, { type: 'poll_upserted', poll: poll('open') }),
    env(42, { type: 'poll_upserted', poll: poll('open', [vote]) }, 'player:p1'),
    env(43, { type: 'batch', events: [{ type: 'undone', seqs: [3] }, { type: 'poll_upserted', poll: poll('closed', [vote], { n: 1 }) }] }),
  ];
}

describe('replay', () => {
  it('places events on the exercise clock, not the wall clock', () => {
    const r = buildReplay(log());
    expect(r.times.slice(0, 4)).toEqual([0, 0, 0, 0]);
    expect(r.times[5]).toBe(5 * 60_000);
    expect(r.times[8]).toBe(10 * 60_000); // 40 minutes in, less the 30 paused
    expect(r.end).toBe(14 * 60_000); // the last event, plus a minute
    expect(r.fired).toEqual({ i1: 5 * 60_000 });
    expect(r.phases).toEqual([{ t: 60_000, name: 'Detect', color: '#3399ff' }]);
  });

  it('puts each mark in its lane', () => {
    const r = buildReplay(log());
    const marks = r.marks.map((m) => [m.lane, m.kind, m.label]);
    expect(marks).toEqual([
      ['script', 'inject', 'Ransom note'],
      ['assets', 'status', 'b'],
      ['team:it', 'log', 'Dana (IT): Isolated the server'],
      ['facilitator', 'poll', 'Decision opened: Pay?'],
      ['team:it', 'vote', 'Dana: No (Pay?)'],
      ['facilitator', 'undo', 'Undo'],
      ['facilitator', 'poll', 'Decided: No'],
    ]);
    expect(r.lanes.map((l) => l.id)).toEqual(['facilitator', 'script', 'team:it', 'assets']);
  });

  it('rebuilds the state at any moment, from any checkpoint', () => {
    // Enough events to need checkpoints.
    const events = log().slice(0, 4);
    for (let i = 0; i < 450; i++) {
      events.push(env(1 + i / 100, { type: 'assets_upserted', assets: [asset('a', i % 2 ? 'clean' : 'compromised')] }));
    }
    const r = buildReplay(events);
    expect(r.checkpoints.map((c) => c.index)).toEqual([1, 200, 400]);
    const full = stateAt(r, r.end);
    expect(full.index).toBe(events.length);
    expect(full.ex.assets.a?.statuses.security).toBe('clean');
    expect(stateAt(r, 0).ex.assets.a?.statuses.security).toBe('suspicious');
    const mid = stateAt(r, r.times[300]!);
    expect(mid.index).toBe(countAt(r.times, r.times[300]!));
    // Seeking never disturbs the saved states.
    expect(r.checkpoints[1]!.ex.assets.a?.statuses.security).toBe(stateAt(r, r.times[199]!).ex.assets.a?.statuses.security);
  });
});
