/** Escape text for safe insertion into HTML. */
export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Unique query terms, longest first (whole query if no spaces). */
export function queryTerms(query: string): string[] {
  const raw = query.trim();
  if (!raw) return [];
  const parts = raw.split(/\s+/).filter(Boolean);
  const terms = parts.length > 1 ? parts : [raw];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const t of [...terms].sort((a, b) => b.length - a.length)) {
    const key = t.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(t);
  }
  return out;
}

export function findFirstMatchIndex(text: string, query: string): number {
  const terms = queryTerms(query);
  if (!terms.length) return -1;
  const lower = text.toLowerCase();
  let best = -1;
  for (const t of terms) {
    const i = lower.indexOf(t.toLowerCase());
    if (i !== -1 && (best === -1 || i < best)) best = i;
  }
  return best;
}

/** Truncate around the first match so the hit stays visible in the list row. */
export function sliceAroundMatch(text: string, query: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  const idx = findFirstMatchIndex(text, query);
  if (idx < 0) return `${text.slice(0, maxLen)}…`;

  const terms = queryTerms(query);
  const hitLen = terms[0]?.length ?? 0;
  const pad = Math.max(8, Math.floor((maxLen - hitLen) / 3));
  let start = Math.max(0, idx - pad);
  const end = Math.min(text.length, start + maxLen);
  if (end - start < maxLen) start = Math.max(0, end - maxLen);

  const prefix = start > 0 ? "…" : "";
  const suffix = end < text.length ? "…" : "";
  return `${prefix}${text.slice(start, end)}${suffix}`;
}

/** Wrap case-insensitive matches in `<mark class="search-hit">` (HTML-escaped). */
export function highlightSearchHtml(text: string, query: string): string {
  const terms = queryTerms(query);
  if (!terms.length || !text) return escapeHtml(text);

  const lower = text.toLowerCase();
  const ranges: Array<[number, number]> = [];
  for (const t of terms) {
    const needle = t.toLowerCase();
    if (!needle) continue;
    let from = 0;
    while (from < text.length) {
      const i = lower.indexOf(needle, from);
      if (i === -1) break;
      ranges.push([i, i + t.length]);
      from = i + Math.max(1, t.length);
    }
  }
  if (!ranges.length) return escapeHtml(text);

  ranges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const merged: Array<[number, number]> = [];
  for (const range of ranges) {
    const last = merged[merged.length - 1];
    if (last && range[0] <= last[1]) {
      last[1] = Math.max(last[1], range[1]);
    } else {
      merged.push([range[0], range[1]]);
    }
  }

  let html = "";
  let cursor = 0;
  for (const [start, end] of merged) {
    if (cursor < start) html += escapeHtml(text.slice(cursor, start));
    html += `<mark class="search-hit">${escapeHtml(text.slice(start, end))}</mark>`;
    cursor = end;
  }
  if (cursor < text.length) html += escapeHtml(text.slice(cursor));
  return html;
}

/** List-row preview: truncate around match, then highlight. */
export function highlightedPreview(content: string, query: string, maxLen = 80): string {
  const q = query.trim();
  if (!q) {
    if (content.length <= maxLen) return escapeHtml(content);
    return escapeHtml(`${content.slice(0, maxLen)}…`);
  }
  return highlightSearchHtml(sliceAroundMatch(content, q, maxLen), q);
}
