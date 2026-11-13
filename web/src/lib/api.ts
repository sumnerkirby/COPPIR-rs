// Typed client for /api/v2. Every request and response type here is generated
// from the Rust server (see src/lib/types), so a server change that breaks the
// frontend fails `npm run check` instead of failing at runtime.

import type { Applied } from './types/Applied';
import type { Command } from './types/Command';
import type { ExerciseSummary } from './types/ExerciseSummary';
import type { Snapshot } from './types/Snapshot';
import type { OsmResult } from './types/OsmResult';
import type { Place } from './types/Place';
import type { Taxonomy } from './types/Taxonomy';
import type { JoinInfo } from './types/JoinInfo';
import type { Me } from './types/Me';
import type { OpenRoom } from './types/OpenRoom';
import type { RoomStatus } from './types/RoomStatus';
import type { TemplateSummary } from './types/TemplateSummary';

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

type Detail = string | { loc?: string[]; msg?: string }[] | undefined;

/** One readable line from the server's `{detail}` error body. */
export function detailText(detail: Detail, fallback: string): string {
  if (typeof detail === 'string') return detail;
  if (!Array.isArray(detail)) return fallback;
  return (
    detail
      .map((d) => {
        const field = d.loc?.filter((p) => p !== 'body').join('.');
        return field ? `${field}: ${d.msg ?? ''}` : (d.msg ?? '');
      })
      .join('; ') || fallback
  );
}

async function request<T>(method: string, url: string, body?: unknown): Promise<T> {
  const init: RequestInit = { method };
  if (body !== undefined) {
    init.headers = { 'Content-Type': 'application/json' };
    init.body = JSON.stringify(body);
  }
  const r = await fetch(url, init);
  if (!r.ok) {
    const err = (await r.json().catch(() => ({}))) as { detail?: Detail };
    throw new ApiError(r.status, detailText(err.detail, r.statusText));
  }
  return r.status === 204 ? (undefined as T) : ((await r.json()) as T);
}

const base = '/api/v2/exercises';

export const api = {
  list: () => request<ExerciseSummary[]>('GET', base),
  create: (name: string, description?: string) => request<Snapshot>('POST', base, { name, description }),
  importV1: (name: string, scenario: unknown) => request<Snapshot>('POST', `${base}/import/v1`, { name, scenario }),
  get: (id: string) => request<Snapshot>('GET', `${base}/${id}`),
  remove: (id: string) => request<void>('DELETE', `${base}/${id}`),
  send: (id: string, command: Command) => request<Applied>('POST', `${base}/${id}/commands`, command),
  importFile: (file: unknown, name?: string) => request<Snapshot>('POST', `${base}/import`, { file, name }),
  duplicate: (id: string, name?: string) => request<Snapshot>('POST', `${base}/${id}/duplicate`, { name }),
  exportUrl: (id: string) => `${base}/${id}/export`,
  legacyScenarios: () => request<string[]>('GET', '/api/v2/legacy/scenarios'),
  importLegacy: (name: string) => request<Snapshot>('POST', '/api/v2/legacy/scenarios/import', { name }),
  defaultTaxonomy: () => request<Taxonomy>('GET', '/api/v2/taxonomy/default'),
  geocode: (query: string) => request<Place[]>('POST', '/api/v2/geocode', { query }),
  osmCategories: () => request<string[]>('GET', '/api/v2/osm/categories'),
  osmSearch: (lat: number, lon: number, radius_mi: number, category: string) =>
    request<OsmResult[]>('POST', '/api/v2/osm/search', { lat, lon, radius_mi, category }),

  templates: () => request<TemplateSummary[]>('GET', '/api/v2/templates'),
  fromTemplate: (id: string, name?: string) => request<Snapshot>('POST', `/api/v2/templates/${id}`, { name }),

  // Who am I, and joining
  me: () => request<Me>('GET', '/api/v2/me'),
  joinInfo: (code: string) => request<JoinInfo>('GET', `/api/v2/join?code=${encodeURIComponent(code)}`),
  join: (code: string, name: string, team?: string) => request<Me>('POST', '/api/v2/join', { code, name, team }),
  leave: () => request<void>('POST', '/api/v2/leave'),

  // The room, for the facilitator
  room: () => request<RoomStatus>('GET', '/api/v2/room'),
  openRoom: (exercise_id: string) => request<OpenRoom>('POST', '/api/v2/room/open', { exercise_id }),
  closeRoom: () => request<void>('POST', '/api/v2/room/close'),
  rotateCodes: () => request<OpenRoom>('POST', '/api/v2/room/codes'),
  moveParticipant: (id: string, team: string | null) => request<void>('PATCH', `/api/v2/room/participants/${id}`, { team }),
  kick: (id: string) => request<void>('DELETE', `/api/v2/room/participants/${id}`),
  qrUrl: (role: 'player' | 'observer', bust: string) => `/api/v2/room/qr.svg?role=${role}&v=${encodeURIComponent(bust)}`,
};

export function socketUrl(id: string, after?: number): string {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const q = after === undefined ? '' : `?after=${after}`;
  return `${proto}://${location.host}${base}/${id}/ws${q}`;
}
