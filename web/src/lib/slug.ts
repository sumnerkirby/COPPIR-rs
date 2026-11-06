/** Lowercase ASCII with underscores, the same rule as the server's `slug`. */
export function slug(name: string): string {
  const s = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
  return s || 'item';
}

/** `slug(name)`, with `_` appended until it is not in `taken`. */
export function uniqueSlug(name: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  let id = slug(name);
  while (used.has(id)) id += '_';
  return id;
}
