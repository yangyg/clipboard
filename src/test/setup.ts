import { vi } from "vitest";

// Stub matchMedia (jsdom does not implement it; settings.applyTheme uses it).
if (!window.matchMedia) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

// The stores talk to the Rust backend through Tauri's `invoke`. Under test there
// is no backend, so resolve every command to a minimal shape. List-returning
// commands get an empty page so store code that reads `.records` / `.has_more`
// does not crash on `undefined`; everything else resolves to `undefined`.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === "get_records" || cmd === "search_records") {
      return Promise.resolve({ records: [], has_more: false });
    }
    return Promise.resolve(undefined);
  }),
}));

// Same for backend event subscriptions: record listeners, resolve a no-op
// unlisten. Tests can pull registered handlers off the mock when needed.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
