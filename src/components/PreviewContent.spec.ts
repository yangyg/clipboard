import { describe, expect, it } from "vitest";
import { mountWithPlugins } from "../test/mount";
import type { ClipboardRecord } from "../types";
import PreviewContent from "./PreviewContent.vue";

const record: ClipboardRecord = {
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
};

const baseProps = {
  record,
  showHtmlPreview: false,
  sanitizedHtml: "",
  clipboardColor: null,
  plainContentHtml: "hello",
  safeLinkHref: null,
  openableLinkUrl: null,
  linkTitle: "Link",
  imageSrc: null,
};

describe("PreviewContent", () => {
  it("renders escaped plain content in the text branch", () => {
    const wrapper = mountWithPlugins(PreviewContent, { props: baseProps });

    expect(wrapper.find(".content-box").html()).toContain("hello");
  });

  it("emits image open for keyboard activation", async () => {
    const wrapper = mountWithPlugins(PreviewContent, {
      props: { ...baseProps, record: { ...record, content_type: "image" }, imageSrc: "asset://image" },
    });

    await wrapper.find(".image-thumb").trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("open-image")).toHaveLength(1);
  });

  it("intercepts http link click and emits open-link instead of navigating", async () => {
    const wrapper = mountWithPlugins(PreviewContent, {
      props: {
        ...baseProps,
        record: { ...record, content_type: "link", content: "https://example.com" },
        plainContentHtml: "https://example.com",
        safeLinkHref: "https://example.com",
        openableLinkUrl: "https://example.com",
        linkTitle: "Web link",
      },
    });

    const anchor = wrapper.find("a.link-url");
    expect(anchor.exists()).toBe(true);

    await anchor.trigger("click");

    expect(wrapper.emitted("open-link")).toHaveLength(1);
  });
});
