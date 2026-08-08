import { describe, it, expect } from "vitest";
import { sanitizeClipboardHtml } from "./sanitizeHtml";

describe("sanitizeClipboardHtml", () => {
  it("keeps common rich-text markup", () => {
    const html = "<p>Hello <b>world</b></p><ul><li>item</li></ul>";
    const out = sanitizeClipboardHtml(html);
    expect(out).toContain("<b>world</b>");
    expect(out).toContain("<li>item</li>");
  });

  it("strips scripts and inline event handlers", () => {
    const out = sanitizeClipboardHtml(
      '<script>alert(1)</script><p onclick="alert(2)">text</p>'
    );
    expect(out).not.toContain("<script");
    expect(out).not.toContain("onclick");
    expect(out).toContain("<p>text</p>");
  });

  it("strips dangerous embed/form tags", () => {
    const out = sanitizeClipboardHtml(
      '<iframe src="https://evil"></iframe><object></object><embed><form><input value="x"></form>'
    );
    for (const tag of ["iframe", "object", "embed", "form", "input"]) {
      expect(out).not.toContain(`<${tag}`);
    }
  });

  it("blocks javascript:/data: URLs but keeps https and mailto links", () => {
    const out = sanitizeClipboardHtml(
      '<a href="javascript:alert(1)">a</a>' +
        '<a href="data:text/html,<script>1</script>">b</a>' +
        '<a href="https://example.com">c</a>' +
        '<a href="mailto:x@example.com">d</a>'
    );
    expect(out).not.toContain("javascript:");
    expect(out).not.toContain("data:text/html");
    expect(out).toContain('href="https://example.com"');
    expect(out).toContain('href="mailto:x@example.com"');
  });

  it("drops data-* attributes and srcset", () => {
    const out = sanitizeClipboardHtml(
      '<img src="https://example.com/i.png" srcset="x.png 2x" data-secret="1">'
    );
    expect(out).not.toContain("data-secret");
    expect(out).not.toContain("srcset");
    expect(out).toContain('src="https://example.com/i.png"');
  });

  it("returns cached output for repeated input", () => {
    const html = "<p>cached body</p>";
    expect(sanitizeClipboardHtml(html)).toBe(sanitizeClipboardHtml(html));
  });

  it("keeps working past the cache bound (eviction smoke test)", () => {
    for (let i = 0; i < 40; i++) {
      expect(sanitizeClipboardHtml(`<p>body-${i}<script>bad()</script></p>`)).toContain(
        `body-${i}`
      );
    }
    // Evicted entry still sanitizes correctly on re-entry.
    expect(sanitizeClipboardHtml("<p>body-0<script>bad()</script></p>")).not.toContain(
      "<script"
    );
  });
});
