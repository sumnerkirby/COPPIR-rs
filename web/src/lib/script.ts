// Plain-language descriptions of the script: what sets an inject off and what
// it does. Shared by the script list and the timeline.

import { fmtMinutes } from './clock.svelte';
import type { Condition } from './types/Condition';
import type { Effect } from './types/Effect';
import type { Exercise } from './types/Exercise';
import type { Trigger } from './types/Trigger';
import type { Visibility } from './types/Visibility';

const assetName = (ex: Exercise, id: string) => ex.assets[id]?.name ?? '(removed asset)';

function statusLabel(ex: Exercise, set: string, value: string): string {
  const s = ex.taxonomy.status_sets.find((x) => x.id === set);
  return `${s?.name ?? set} ${s?.values.find((v) => v.id === value)?.label ?? value}`;
}

export function describeCondition(ex: Exercise, c: Condition): string {
  switch (c.type) {
    case 'asset_status':
      return `${assetName(ex, c.asset)} is ${statusLabel(ex, c.set, c.value)}`;
    case 'inject_fired':
      return `“${ex.injects[c.inject]?.title ?? '(removed inject)'}” has fired`;
    case 'threshold_breached':
      return `${ex.thresholds.find((t) => t.id === c.threshold)?.name ?? '(removed threshold)'} is breached`;
    case 'poll_choice': {
      const p = ex.polls[c.poll];
      return `the poll “${p?.question ?? '?'}” chose ${p?.options.find((o) => o.id === c.option)?.label ?? '?'}`;
    }
  }
}

export function describeTrigger(ex: Exercise, t: Trigger): string {
  switch (t.type) {
    case 'manual':
      return 'When you fire it';
    case 'at':
      return `At T+${fmtMinutes(t.minutes)}`;
    case 'phase': {
      const name = ex.phases.find((p) => p.id === t.phase)?.name ?? '(removed phase)';
      return t.delay_minutes ? `${fmtMinutes(t.delay_minutes)} into ${name}` : `When ${name} begins`;
    }
    case 'when':
      return `When ${describeCondition(ex, t.condition)}`;
  }
}

export function describeVisibility(ex: Exercise, v: Visibility): string {
  if (v.mode === 'all') return 'everyone';
  if (v.mode === 'facilitator') return 'the facilitator only';
  return v.teams.map((t) => ex.teams.find((x) => x.id === t)?.name ?? '?').join(', ');
}

export function describeEffect(ex: Exercise, e: Effect): string {
  switch (e.type) {
    case 'set_status':
      return `${assetName(ex, e.asset)} → ${statusLabel(ex, e.set, e.value)}`;
    case 'create_asset':
      return `Creates ${e.asset.name || 'an asset'}`;
    case 'reveal_asset':
      return `Shows ${assetName(ex, e.asset)} to ${describeVisibility(ex, e.visibility)}`;
    case 'open_poll':
      return `Opens the poll “${ex.polls[e.poll]?.question ?? '?'}”`;
    case 'enter_phase':
      return `Begins ${ex.phases.find((p) => p.id === e.phase)?.name ?? '(removed phase)'}`;
  }
}
