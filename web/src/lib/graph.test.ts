import { expect, it } from 'vitest';
import { autoLayout } from './graph';
import type { Asset } from './types/Asset';

const asset = (id: string): Asset => ({
  id, name: id, type_id: 'asset', geo: null, graph: null, statuses: {}, history: [], fields: {}, tags: [], notes: '',
  color: null, visibility: { mode: 'all' }, created_at: 't',
});

it('puts providers above their dependants', () => {
  const pos = autoLayout(['grid', 'sub', 'hospital', 'loner'].map(asset), [
    { id: '1', from: 'grid', to: 'sub', kind: 'depends_on', label: '' },
    { id: '2', from: 'sub', to: 'hospital', kind: 'depends_on', label: '' },
    { id: '3', from: 'sub', to: 'gone', kind: 'depends_on', label: '' },
  ]);
  expect(Object.keys(pos).sort()).toEqual(['grid', 'hospital', 'loner', 'sub']);
  expect(pos.grid!.y).toBeLessThan(pos.sub!.y);
  expect(pos.sub!.y).toBeLessThan(pos.hospital!.y);
});
