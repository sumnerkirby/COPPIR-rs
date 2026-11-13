// Applies a server event to the local copy of an exercise.
//
// Events carry resulting state (whole assets, whole links), so this is upserts
// and removals only — the same as `Exercise::apply` in src/exercise/event.rs.
// Keep the two in step; `reducer.test.ts` covers every event type.

import type { Envelope } from './types/Envelope';
import type { Exercise } from './types/Exercise';
import type { Asset } from './types/Asset';

function upsertAssets(ex: Exercise, assets: Asset[]) {
  for (const a of assets) ex.assets[a.id] = a;
}

/** Mutates `ex` in place; returns it for convenience. */
export function apply(ex: Exercise, env: Envelope): Exercise {
  ex.updated_at = env.at;
  const e = env.event;
  switch (e.type) {
    case 'created':
      Object.assign(ex, structuredClone(e.exercise));
      ex.updated_at = env.at;
      break;
    case 'renamed':
      ex.name = e.name;
      ex.description = e.description;
      break;
    case 'taxonomy_replaced':
      ex.taxonomy = e.taxonomy;
      upsertAssets(ex, e.assets);
      break;
    case 'assets_upserted':
      upsertAssets(ex, e.assets);
      break;
    case 'assets_removed':
      for (const id of e.ids) delete ex.assets[id];
      for (const id of e.links) delete ex.links[id];
      break;
    case 'link_upserted':
      ex.links[e.link.id] = e.link;
      break;
    case 'link_removed':
      delete ex.links[e.id];
      break;
    case 'inject_upserted':
      ex.injects[e.inject.id] = e.inject;
      break;
    case 'inject_removed':
      delete ex.injects[e.id];
      break;
    case 'inject_fired':
      ex.injects[e.inject.id] = e.inject;
      upsertAssets(ex, e.assets);
      ex.log.push(e.entry);
      break;
    case 'threshold_upserted': {
      const i = ex.thresholds.findIndex((t) => t.id === e.threshold.id);
      if (i >= 0) ex.thresholds[i] = e.threshold;
      else ex.thresholds.push(e.threshold);
      break;
    }
    case 'threshold_removed':
      ex.thresholds = ex.thresholds.filter((t) => t.id !== e.id);
      break;
    case 'logged':
      ex.log.push(e.entry);
      break;
    case 'log_removed':
      ex.log = ex.log.filter((l) => l.id !== e.id);
      break;
    case 'annotation_upserted':
      ex.annotations[e.annotation.id] = e.annotation;
      break;
    case 'annotations_removed':
      for (const id of e.ids) delete ex.annotations[id];
      break;
    case 'metric_upserted': {
      const i = ex.metrics.findIndex((m) => m.id === e.metric.id);
      if (i >= 0) ex.metrics[i] = e.metric;
      else ex.metrics.push(e.metric);
      break;
    }
    case 'metric_removed':
      ex.metrics = ex.metrics.filter((m) => m.id !== e.id);
      break;
    case 'team_upserted': {
      const i = ex.teams.findIndex((t) => t.id === e.team.id);
      if (i >= 0) ex.teams[i] = e.team;
      else ex.teams.push(e.team);
      break;
    }
    case 'team_removed':
      ex.teams = ex.teams.filter((t) => t.id !== e.id);
      break;
    case 'clock_changed':
      ex.clock = e.clock;
      break;
    case 'phases_replaced':
      ex.phases = e.phases;
      break;
    case 'phase_entered':
      ex.phase_log.push(e.entry);
      break;
    case 'phase_log_replaced':
      ex.phase_log = e.log;
      break;
    case 'poll_upserted':
      ex.polls[e.poll.id] = e.poll;
      break;
    case 'poll_removed':
      delete ex.polls[e.id];
      break;
    case 'impact_rules_replaced':
      ex.impact_rules = e.rules;
      break;
    case 'batch':
      for (const event of e.events) apply(ex, { ...env, event });
      break;
    case 'undone':
      break;
    default: {
      // A new event type on the server without a case here fails type-checking.
      const unhandled: never = e;
      throw new Error(`unknown event ${JSON.stringify(unhandled)}`);
    }
  }
  return ex;
}
