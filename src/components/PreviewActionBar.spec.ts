import { describe, expect, it } from "vitest";
import { mountWithPlugins } from "../test/mount";
import type { ClipboardRecord } from "../types";
import PreviewActionBar from "./PreviewActionBar.vue";

function makeRecord(overrides: Partial<ClipboardRecord> = {}): ClipboardRecord {
  return {
    id: 1,
    content: "hello",
    content_type: "text",
    source_app: "test.exe",
    source_window: "Test",
    hash: "hash",
    copy_count: 0,
    is_favorite: false,
    is_pinned: false,
    is_sensitive: false,
    is_trashed: false,
    auto_expire_at: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    tags: [],
    ...overrides,
  };
}

describe("PreviewActionBar", () => {
  it("emits paste and pin actions for active records", async () => {
    const wrapper = mountWithPlugins(PreviewActionBar, {
      props: { record: makeRecord(), pinnedDisplay: false },
    });

    await wrapper.find(".action-primary").trigger("click");
    await wrapper.find(".action-pin").trigger("click");

    expect(wrapper.emitted("paste")).toHaveLength(1);
    expect(wrapper.emitted("pin")).toHaveLength(1);
  });

  it("renders restore actions for trashed records", async () => {
    const wrapper = mountWithPlugins(PreviewActionBar, {
      props: { record: makeRecord({ is_trashed: true }), pinnedDisplay: false },
    });

    await wrapper.find(".action-primary").trigger("click");

    expect(wrapper.emitted("restore")).toHaveLength(1);
    expect(wrapper.find(".trash-actions").exists()).toBe(true);
  });
});
