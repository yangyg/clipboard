import { describe, it, expect } from "vitest";
import { formatWebDavResult } from "./webdavResult";
import type { WebDavSyncResult } from "../types";

function stubT(key: string, params?: Record<string, unknown>): string {
  const named = params ? `|${JSON.stringify(params)}` : "";
  return `${key}${named}`;
}

const base: WebDavSyncResult = {
  pulled: 0,
  pushed: 0,
  merged: 0,
  tags_pulled: 0,
  tags_pushed: 0,
  media_downloaded: 0,
  media_uploaded: 0,
  media_skipped: 0,
};

describe("formatWebDavResult", () => {
  it("renders content counts for pull", () => {
    const r = { ...base, pulled: 12, merged: 3 };
    const msg = formatWebDavResult(r, "pull", stubT);
    expect(msg).toContain('settings.sync.pullResult|{"pulled":12,"merged":3}');
  });

  it("renders content counts for push", () => {
    const msg = formatWebDavResult({ ...base, pushed: 5 }, "push", stubT);
    expect(msg).toContain('settings.sync.pushResult|{"pushed":5}');
  });

  it("renders content counts for sync combining both directions", () => {
    const msg = formatWebDavResult({ ...base, pulled: 2, merged: 1, pushed: 4 }, "sync", stubT);
    expect(msg).toContain('settings.sync.syncResult|{"pulled":2,"merged":1,"pushed":4}');
  });

  it("omits tag clause when count is zero", () => {
    const msg = formatWebDavResult(base, "pull", stubT);
    expect(msg).not.toContain("resultTags");
  });

  it("adds tag clause with the action-specific direction", () => {
    const pullMsg = formatWebDavResult({ ...base, tags_pulled: 4, tags_pushed: 9 }, "pull", stubT);
    expect(pullMsg).toContain('resultTags|{"count":4}');
    expect(pullMsg).not.toContain('{"count":9}');

    const pushMsg = formatWebDavResult({ ...base, tags_pulled: 4, tags_pushed: 9 }, "push", stubT);
    expect(pushMsg).toContain('resultTags|{"count":9}');
    expect(pushMsg).not.toContain('{"count":4}');

    const syncMsg = formatWebDavResult({ ...base, tags_pulled: 4, tags_pushed: 9 }, "sync", stubT);
    expect(syncMsg).toContain('resultTags|{"count":13}');
  });

  it("omits each media clause when its counter is zero", () => {
    const msg = formatWebDavResult(base, "sync", stubT);
    expect(msg).not.toContain("resultMediaDownload");
    expect(msg).not.toContain("resultMediaUpload");
    expect(msg).not.toContain("resultMediaSkip");
  });

  it("adds only the non-zero media clauses", () => {
    const msg = formatWebDavResult(
      { ...base, media_downloaded: 0, media_uploaded: 1, media_skipped: 7 },
      "push",
      stubT,
    );
    expect(msg).toContain('resultMediaUpload|{"count":1}');
    expect(msg).toContain('resultMediaSkip|{"count":7}');
    expect(msg).not.toContain("resultMediaDownload");
  });
});
