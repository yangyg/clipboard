import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SearchHistoryEntry } from "../types";

/**
 * Recent-search history backing the autocomplete dropdown, persisted in SQLite
 * (`search_history` table) for durability. Local-only: never exported/synced.
 *
 * The backend caps storage at 50 rows; this module mirrors that list in memory
 * (loaded once) so the dropdown stays synchronous. Writes are fire-and-forget
 * with an optimistic local update — a failed DB write is silently dropped,
 * matching the previous localStorage best-effort behaviour.
 */
const STORE_LIMIT = 50;

export function useSearchHistory() {
  const history = ref<string[]>([]);

  async function loadHistory() {
    try {
      const entries = await invoke<SearchHistoryEntry[]>("get_search_history", {
        limit: STORE_LIMIT,
      });
      history.value = entries.map((e) => e.query);
    } catch {
      // Best-effort: dropdown just starts empty.
    }
  }

  async function recordHistory(term: string) {
    const t = term.trim();
    if (!t) return;
    history.value = [t, ...history.value.filter((h) => h !== t)].slice(0, STORE_LIMIT);
    try {
      await invoke("record_search_history", { query: t });
    } catch {
      // Best-effort.
    }
  }

  async function removeHistory(term: string) {
    history.value = history.value.filter((h) => h !== term);
    try {
      await invoke("remove_search_history", { query: term });
    } catch {
      // Best-effort.
    }
  }

  async function clearHistory() {
    history.value = [];
    try {
      await invoke("clear_search_history");
    } catch {
      // Best-effort.
    }
  }

  return { history, loadHistory, recordHistory, removeHistory, clearHistory };
}
