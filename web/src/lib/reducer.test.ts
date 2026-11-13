import { describe, expect, it } from 'vitest';
import { apply } from './reducer';
import type { Asset } from './types/Asset';
import type { Envelope } from './types/Envelope';
import type { Event } from './types/Event';
import type { Exercise } from './types/Exercise';

const blank = (): Exercise => ({
  id: 'x',
  name: 'Drill',
  description: '',
  taxonomy: { asset_types: [], status_sets: [], sectors: [], fields: [] },
  teams: [],
  assets: {},
  links: {},
  injects: {},
  thresholds: [],
  log: [],
  annotations: {},
  metrics: [],
  created_at: 't0',
  updated_at: 't0',
});

const asset = (id: string, security = 'clean'): Asset => ({
  id,
  name: id,
  type_id: 'asset',
  geo: { lat: 0, lon: 0 },
  graph: null,
  statuses: { security },
  history: [],
  fields: {},
  tags: [],
  notes: '',
  color: null,
  visibility: { mode: 'all' },
  created_at: 't0',
});

let seq = 0;
const env = (event: Event): Envelope => ({ seq: ++seq, at: `t${seq}`, actor: 'test', event });

describe('apply', () => {
  it('handles every event type', () => {
    const ex = blank();
    apply(ex, env({ type: 'created', exercise: blank() }));
    apply(ex, env({ type: 'renamed', name: 'New', description: 'd' }));
    apply(ex, env({ type: 'assets_upserted', assets: [asset('a'), asset('b')] }));
    apply(ex, env({ type: 'link_upserted', link: { id: 'l', from: 'a', to: 'b', kind: 'depends_on', label: '' } }));
    apply(ex, env({ type: 'assets_upserted', assets: [asset('a', 'compromised')] }));
    expect(ex.assets.a?.statuses.security).toBe('compromised');

    const inject = {
      id: 'i', title: 't', body: '', severity: 'info' as const, audience: { mode: 'all' as const },
      effects: [], notes: '', created_at: 't', fired_at: null,
    };
    apply(ex, env({ type: 'inject_upserted', inject }));
    const entry = { id: 'e', at: 't', text: 'fired', asset: null, asset_name: null, notes: '', auto: true, author: '', visibility: { mode: 'all' as const } };
    apply(ex, env({ type: 'inject_fired', inject: { ...inject, fired_at: 'now' }, assets: [asset('c')], entry }));
    expect(ex.injects.i?.fired_at).toBe('now');
    expect(Object.keys(ex.assets)).toEqual(['a', 'b', 'c']);

    apply(ex, env({ type: 'threshold_upserted', threshold: { id: 't', name: 'T', sector: null, below_pct: 50, severity: 'warning' } }));
    apply(ex, env({ type: 'threshold_upserted', threshold: { id: 't', name: 'T2', sector: null, below_pct: 40, severity: 'warning' } }));
    expect(ex.thresholds.map((t) => t.name)).toEqual(['T2']);
    apply(ex, env({ type: 'logged', entry: { ...entry, id: 'e2', auto: false } }));
    apply(ex, env({ type: 'assets_removed', ids: ['a'], links: ['l'] }));
    apply(ex, env({ type: 'inject_removed', id: 'i' }));
    apply(ex, env({ type: 'threshold_removed', id: 't' }));
    apply(ex, env({ type: 'link_removed', id: 'nope' }));
    apply(ex, env({ type: 'taxonomy_replaced', taxonomy: { ...ex.taxonomy, fields: [] }, assets: [asset('b', 'monitored')] }));

    apply(ex, env({ type: 'team_upserted', team: { id: 'it', name: 'IT', color: '#3399ff' } }));
    apply(ex, env({ type: 'team_upserted', team: { id: 'it', name: 'IT Ops', color: '#3399ff' } }));
    expect(ex.teams.map((t) => t.name)).toEqual(['IT Ops']);
    apply(ex, env({ type: 'batch', events: [{ type: 'team_removed', id: 'it' }, { type: 'renamed', name: 'New', description: 'd' }] }));
    expect(ex.teams).toEqual([]);
    expect(ex.name).toBe('New');
    expect(Object.keys(ex.assets)).toEqual(['b', 'c']);
    expect(ex.assets.b?.statuses.security).toBe('monitored');
    expect(ex.links).toEqual({});
    expect(ex.injects).toEqual({});
    expect(ex.thresholds).toEqual([]);
    expect(ex.log.map((l) => l.id)).toEqual(['e', 'e2']);
    expect(ex.updated_at).toBe(`t${seq}`);
  });
});
