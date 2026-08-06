import { describe, expect, it } from "vitest";
import { mountWithPlugins } from "../test/mount";
import type { ClipboardRecord } from "../types";
import RecordListItem from "./RecordListItem.vue";

const record: ClipboardRecord = {
  id: 7,
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
};

const props = {
  record,
  batchMode: false,
  checked: false,
  selected: false,
  tabbable: true,
  trashFilter: false,
  pinned: false,
  isNew: false,
  isLeaving: false,
  searchQuery: "",
  sourceOverrides: {},
};

describe("RecordListItem", () => {
  it("keeps listbox semantics and emits keyboard activation", async () => {
    const wrapper = mountWithPlugins(RecordListItem, { props });

    expect(wrapper.attributes("role")).toBe("option");
    expect(wrapper.attributes("id")).toBe("record-option-7");
    await wrapper.trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("activate")).toEqual([[7]]);
  });

  it("emits the record on context menu", async () => {
    const wrapper = mountWithPlugins(RecordListItem, { props });

    await wrapper.trigger("contextmenu");

    expect(wrapper.emitted("context-menu")?.[0]?.[1]).toEqual(record);
  });

  it("shows an alias icon prefix and the alias title when the record has an alias", () => {
    const wrapper = mountWithPlugins(RecordListItem, {
      props: { ...props, record: { ...record, alias: "my alias" } },
    });

    const mark = wrapper.find(".alias-mark");
    expect(mark.exists()).toBe(true);
    expect(wrapper.find(".record-title").text()).toContain("my alias");
    expect(wrapper.find(".record-title").attributes("title")).toBe("hello");
  });

  it("hides the alias icon when no alias is set", () => {
    const wrapper = mountWithPlugins(RecordListItem, { props });

    expect(wrapper.find(".alias-mark").exists()).toBe(false);
    expect(wrapper.find(".record-title").attributes("title")).toBeUndefined();
  });
});
