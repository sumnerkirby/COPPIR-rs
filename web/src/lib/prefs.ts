// Per-browser conveniences (legend open, active tab). Storage can be missing
// or throw (private windows, blocked site data), so every access is guarded
// and every caller has a default.

export function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(`coppir:${key}`);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

export function save(key: string, value: unknown) {
  try {
    localStorage.setItem(`coppir:${key}`, JSON.stringify(value));
  } catch {
    /* not available; the preference just will not stick */
  }
}
