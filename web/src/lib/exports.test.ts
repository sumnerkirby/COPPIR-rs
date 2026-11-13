import { describe, expect, it } from 'vitest';
import { briefingHtml, csvField, logCsv } from './exports';
import type { Exercise } from './types/Exercise';

const ex = (): Exercise => ({
  id: 'x',
  name: 'Drill <script>',
  description: '',
  taxonomy: {
    asset_types: [{ id: 'asset', name: 'Asset', icon: 'server', color: '#888888' }],
    status_sets: [
      {
        id: 'security', name: 'Security', default: 'clean', display: 'fill',
        values: [{ id: 'clean', label: 'Clean', color: '#33cc33', glyph: '✓', score: 100 }],
      },
    ],
    sectors: [],
    fields: [],
  },
  teams: [],
  assets: {},
  links: {},
  injects: {},
  thresholds: [],
  log: [
    { id: '1', at: '2026-01-01T10:00:00Z', text: 'Said "hi", then left', asset: null, asset_name: null, notes: '=SUM(A1)', auto: false, author: '', visibility: { mode: 'all' } },
  ],
  annotations: {},
  metrics: [],
  clock: { elapsed_ms: 0, running_since: null },
  phases: [],
  phase_log: [],
  polls: {},
  impact_rules: [],
  created_at: 't',
  updated_at: 't',
});

describe('exports', () => {
  it('quotes CSV fields and neutralises formulas', () => {
    expect(csvField('plain')).toBe('plain');
    expect(csvField('a,b')).toBe('"a,b"');
    expect(csvField('=1+1')).toBe("'=1+1");
    expect(csvField(null)).toBe('');
    expect(logCsv(ex())).toBe(
      'Timestamp,Entry,Asset,Notes,Automatic\r\n2026-01-01T10:00:00Z,"Said ""hi"", then left",,\'=SUM(A1),no\r\n',
    );
  });

  it('escapes user text in the briefing', () => {
    const html = briefingHtml(ex());
    expect(html).toContain('Drill &lt;script&gt;');
    expect(html).not.toContain('<script>');
  });
});
