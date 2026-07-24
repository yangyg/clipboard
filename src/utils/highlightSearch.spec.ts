import { describe, it, expect } from "vitest";
import {
  escapeHtml,
  findFirstMatchIndex,
  highlightSearchHtml,
  highlightedPreview,
  queryTerms,
  sliceAroundMatch,
} from "./highlightSearch";

describe("highlightSearch", () => {
  it("escapes HTML", () => {
    expect(escapeHtml(`a<b>&"c`)).toBe("a&lt;b&gt;&amp;&quot;c");
  });

  it("splits multi-word queries", () => {
    expect(queryTerms("  foo  bar ")).toEqual(["foo", "bar"]);
    expect(queryTerms("hello")).toEqual(["hello"]);
  });

  it("highlights case-insensitively", () => {
    expect(highlightSearchHtml("Hello World", "world")).toBe(
      'Hello <mark class="search-hit">World</mark>',
    );
  });

  it("does not inject HTML from content or query", () => {
    expect(highlightSearchHtml("<script>x</script>", "script")).toBe(
      '&lt;<mark class="search-hit">script</mark>&gt;x&lt;/<mark class="search-hit">script</mark>&gt;',
    );
  });

  it("slices around the first match", () => {
    const text = `${"a".repeat(40)}NEEDLE${"b".repeat(40)}`;
    const sliced = sliceAroundMatch(text, "NEEDLE", 20);
    expect(sliced.includes("NEEDLE")).toBe(true);
    expect(sliced.startsWith("…") || sliced.endsWith("…")).toBe(true);
    expect(findFirstMatchIndex(text, "NEEDLE")).toBe(40);
  });

  it("builds a highlighted preview", () => {
    const html = highlightedPreview("prefix KEYWORD suffix more text", "keyword", 40);
    expect(html).toContain('<mark class="search-hit">KEYWORD</mark>');
    expect(html).not.toContain("<script");
  });
});
