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
// is no backend, so resolve every command to `undefined`. Smoke tests exercise
// synchronous store logic and never depend on a command's return shape.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));
