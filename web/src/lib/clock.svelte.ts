// The exercise clock as the browser shows it. The server keeps the clock;
// this only works out "now" from it, ticking locally between updates.

import type { Clock } from './types/Clock';
import type { Exercise } from './types/Exercise';
import type { Inject } from './types/Inject';
import type { Phase } from './types/Phase';

class Ticker {
  now = $state(Date.now());
  constructor() {
    setInterval(() => (this.now = Date.now()), 250);
  }
}

/** One shared ticker for every clock on the page. */
export const ticker = new Ticker();

export function elapsedMs(clock: Clock, now = ticker.now): number {
  if (!clock.running_since) return clock.elapsed_ms;
  return clock.elapsed_ms + Math.max(0, now - Date.parse(clock.running_since));
}

/** T+01:02:03 */
export function fmtClock(ms: number): string {
  const s = Math.floor(ms / 1000);
  return [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60].map((n) => String(n).padStart(2, '0')).join(':');
}

/** "45 min", "1 h 30 min" */
export function fmtMinutes(m: number): string {
  const r = Math.round(m);
  if (r < 60) return `${r} min`;
  return `${Math.floor(r / 60)} h${r % 60 ? ` ${r % 60} min` : ''}`;
}

export function currentPhase(ex: Exercise): Phase | undefined {
  const last = ex.phase_log.at(-1);
  return last ? ex.phases.find((p) => p.id === last.phase) : undefined;
}

/** Where each phase sits on the planned timeline, in minutes from the start. */
export function plannedPhases(ex: Exercise): { phase: Phase; start: number; end: number }[] {
  let t = 0;
  return ex.phases.map((phase) => {
    const start = t;
    t += phase.target_minutes;
    return { phase, start, end: t };
  });
}

/**
 * When an inject is planned to fire, in exercise minutes, or null when it
 * waits for a condition or for the facilitator. A phase trigger counts from
 * when the phase actually began if it has, or from its planned start.
 */
export function plannedMinute(ex: Exercise, i: Inject): number | null {
  const t = i.trigger;
  if (t.type === 'at') return t.minutes;
  if (t.type === 'phase') {
    const began = [...ex.phase_log].reverse().find((e) => e.phase === t.phase);
    const start = began ? began.elapsed_ms / 60000 : plannedPhases(ex).find((p) => p.phase.id === t.phase)?.start;
    return start === undefined ? null : start + t.delay_minutes;
  }
  return null;
}
