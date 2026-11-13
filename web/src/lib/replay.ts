// After-action replay: the exercise at any moment of its run, rebuilt from
// the event log, and the marks that go on the review's timeline lanes.
//
// Positions are exercise-clock milliseconds, the same "T+" everyone saw.
// Setup before the clock started sits at T+0; a paused clock adds nothing.
// Rebuilding from the start for every frame would be slow, so the state is
// saved every few hundred events and a seek starts from the nearest save.

import { apply } from './reducer';
import type { Clock } from './types/Clock';
import type { Envelope } from './types/Envelope';
import type { Event } from './types/Event';
import type { Exercise } from './types/Exercise';
import type { Visibility } from './types/Visibility';

export type MarkKind = 'inject' | 'log' | 'vote' | 'poll' | 'status' | 'undo';

export interface Mark {
  t: number;
  lane: string;
  kind: MarkKind;
  label: string;
  color?: string;
}

export interface Lane {
  id: string;
  name: string;
  color: string;
}

export interface PhaseMark {
  t: number;
  name: string;
  color: string;
}

export interface Replay {
  events: Envelope[];
  /** Exercise-clock time of each event, never decreasing. */
  times: number[];
  /** The state after the first `index` events. */
  checkpoints: { index: number; ex: Exercise }[];
  marks: Mark[];
  phases: PhaseMark[];
  lanes: Lane[];
  /** When each inject last fired. */
  fired: Record<string, number>;
  end: number;
}

const EVERY = 200;
const SEVERITY: Record<string, string> = { info: '#3399ff', warning: '#ffaa00', critical: '#ff4d4d' };

export function elapsedAt(clock: Clock, at: string): number {
  if (!clock.running_since) return clock.elapsed_ms;
  return clock.elapsed_ms + Math.max(0, Date.parse(at) - Date.parse(clock.running_since));
}

function flatten(e: Event): Event[] {
  return e.type === 'batch' ? e.events.flatMap(flatten) : [e];
}

function teamLane(v: Visibility): string {
  return v.mode === 'teams' && v.teams[0] ? `team:${v.teams[0]}` : 'players';
}

export function buildReplay(events: Envelope[]): Replay {
  const first = events[0];
  if (first?.event.type !== 'created') throw new Error('The log does not start with the exercise being created');
  const ex = structuredClone(first.event.exercise);
  apply(ex, first);
  const r: Replay = { events, times: [0], checkpoints: [{ index: 1, ex: structuredClone(ex) }], marks: [], phases: [], lanes: [], fired: {}, end: 0 };
  let t = 0;
  let started = false;

  for (let i = 1; i < events.length; i++) {
    const env = events[i]!;
    t = Math.max(t, elapsedAt(ex.clock, env.at));
    r.times.push(t);
    const byActor = env.actor === 'engine' ? 'script' : env.actor.startsWith('player:') ? 'players' : 'facilitator';
    const mark = (m: Omit<Mark, 't'>) => r.marks.push({ t, ...m });

    for (const e of flatten(env.event)) {
      // Each change is read against the state just before it.
      switch (e.type) {
        case 'inject_fired':
          r.fired[e.inject.id] = t;
          mark({ lane: byActor === 'script' ? 'script' : 'facilitator', kind: 'inject', label: e.inject.title, color: SEVERITY[e.inject.severity] });
          break;
        case 'logged':
          if (e.entry.auto) break; // inject and impact lines: the inject mark covers them
          mark({
            lane: byActor === 'players' ? teamLane(e.entry.visibility) : byActor,
            kind: 'log',
            label: e.entry.author ? `${e.entry.author}: ${e.entry.text}` : e.entry.text,
          });
          break;
        case 'poll_upserted': {
          const old = ex.polls[e.poll.id];
          if (old?.state !== 'open' && e.poll.state === 'open') {
            mark({ lane: byActor, kind: 'poll', label: `Decision opened: ${e.poll.question}` });
          }
          if (old?.state === 'open' && e.poll.state === 'closed') {
            const max = Math.max(0, ...Object.values(e.poll.results));
            const won = e.poll.options.filter((o) => max > 0 && e.poll.results[o.id] === max).map((o) => o.label);
            mark({ lane: byActor, kind: 'poll', label: `Decided: ${won.join(' / ') || 'no answer'}` });
          }
          for (const resp of e.poll.responses) {
            if (old?.responses.some((o) => o.participant === resp.participant && o.at === resp.at)) continue;
            const choice = e.poll.options.find((o) => o.id === resp.option)?.label;
            mark({
              lane: resp.team ? `team:${resp.team}` : 'players',
              kind: 'vote',
              label: `${resp.name}: ${[choice, resp.text].filter(Boolean).join(', ')} (${e.poll.question})`,
            });
          }
          break;
        }
        case 'undone':
          mark({ lane: 'facilitator', kind: 'undo', label: 'Undo' });
          break;
        case 'phase_entered': {
          const p = ex.phases.find((x) => x.id === e.entry.phase);
          if (p) r.phases.push({ t, name: p.name, color: p.color });
          break;
        }
        case 'clock_changed':
          if (e.clock.running_since) started = true;
          break;
      }
      // Status changes to assets that already existed, once under way.
      const assets = e.type === 'assets_upserted' || e.type === 'taxonomy_replaced' || e.type === 'inject_fired' ? e.assets : [];
      if (started && assets.length) {
        const changed = assets.filter((a) => {
          const old = ex.assets[a.id];
          return old && Object.entries(a.statuses).some(([k, v]) => old.statuses[k] !== v);
        });
        if (changed.length) {
          const names = changed.slice(0, 3).map((a) => a.name).join(', ');
          mark({ lane: 'assets', kind: 'status', label: changed.length > 3 ? `${names} and ${changed.length - 3} more` : names });
        }
      }
      apply(ex, { ...env, event: e });
    }
    if ((i + 1) % EVERY === 0) r.checkpoints.push({ index: i + 1, ex: structuredClone(ex) });
  }

  // A little room after the last event, so its marks are not on the edge.
  r.end = Math.max(60_000, t + Math.max(60_000, t * 0.03));
  const used = new Set(r.marks.map((m) => m.lane));
  r.lanes = [
    { id: 'facilitator', name: 'Facilitator', color: '#9aa4ad' },
    { id: 'script', name: 'Script', color: '#b48cff' },
    ...ex.teams.map((team) => ({ id: `team:${team.id}`, name: team.name, color: team.color })),
    { id: 'players', name: 'Players', color: '#6fd3ff' },
    { id: 'assets', name: 'Assets', color: '#ffaa00' },
  ].filter((l) => used.has(l.id) || l.id.startsWith('team:'));
  return r;
}

/** How many events had happened by time `t`. */
export function countAt(times: number[], t: number): number {
  let lo = 0;
  let hi = times.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (times[mid]! <= t) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Apply events `from`..`to` to `ex` in place. */
export function advance(r: Replay, ex: Exercise, from: number, to: number): number {
  for (let i = from; i < to; i++) apply(ex, r.events[i]!);
  return Math.max(from, to);
}

/** A fresh copy of the exercise as it was at time `t`. */
export function stateAt(r: Replay, t: number): { ex: Exercise; index: number } {
  const n = Math.max(1, countAt(r.times, t));
  let cp = r.checkpoints[0]!;
  for (const c of r.checkpoints) if (c.index <= n) cp = c;
  const ex = structuredClone(cp.ex);
  return { ex, index: advance(r, ex, cp.index, n) };
}
