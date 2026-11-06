// Typed client for /api/v2. Every request and response type here is generated
// from the Rust server (see src/lib/types), so a server change that breaks the
// frontend fails `npm run check` instead of failing at runtime.

import type { Applied } from './types/Applied';
import type { Command } from './types/Command';
import type { ExerciseSummary } from './types/ExerciseSummary';
import type { Snapshot } from './types/Snapshot';

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
};

export function socketUrl(id: string, after?: number): string {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const q = after === undefined ? '' : `?after=${after}`;
  return `${proto}://${location.host}${base}/${id}/ws${q}`;
}
