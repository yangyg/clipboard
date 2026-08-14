import { describe, expect, it } from "vitest";
import { makeRecord } from "../test/factories";
import {
  isExpired,
  needsExpiryConfirm,
  sensitiveExpireDisplay,
} from "./sensitiveExpiry";

const past = "2000-01-01T00:00:00Z";
const future = new Date(Date.now() + 60_000).toISOString();

describe("isExpired", () => {
  it("is false without an auto_expire_at", () => {
    expect(isExpired(makeRecord({ auto_expire_at: null }))).toBe(false);
  });

  it("is false for a future timestamp and true for a past one", () => {
    expect(isExpired(makeRecord({ auto_expire_at: future }))).toBe(false);
    expect(isExpired(makeRecord({ auto_expire_at: past }))).toBe(true);
  });

  it("is false for an unparsable timestamp", () => {
    expect(isExpired(makeRecord({ auto_expire_at: "not-a-date" }))).toBe(false);
  });
});

describe("needsExpiryConfirm", () => {
  it("is false for non-sensitive records even when expired", () => {
    expect(
      needsExpiryConfirm(makeRecord({ is_sensitive: false, auto_expire_at: past }), "pin"),
    ).toBe(false);
  });

  it("is false when the record has not expired yet", () => {
    expect(
      needsExpiryConfirm(makeRecord({ is_sensitive: true, auto_expire_at: future }), "pin"),
    ).toBe(false);
  });

  it("is true when un-pinning an expired sensitive record with no favorite shield", () => {
    expect(
      needsExpiryConfirm(
        makeRecord({ is_sensitive: true, auto_expire_at: past, is_pinned: true, is_favorite: false }),
        "pin",
      ),
    ).toBe(true);
  });

  it("is false when un-pinning but the record is still favorited", () => {
    expect(
      needsExpiryConfirm(
        makeRecord({ is_sensitive: true, auto_expire_at: past, is_pinned: true, is_favorite: true }),
        "pin",
      ),
    ).toBe(false);
  });

  it("is true when un-favoriting an expired sensitive record with no pin shield", () => {
    expect(
      needsExpiryConfirm(
        makeRecord({ is_sensitive: true, auto_expire_at: past, is_pinned: false, is_favorite: true }),
        "favorite",
      ),
    ).toBe(true);
  });

  it("is false when un-favoriting but the record is still pinned", () => {
    expect(
      needsExpiryConfirm(
        makeRecord({ is_sensitive: true, auto_expire_at: past, is_pinned: true, is_favorite: true }),
        "favorite",
      ),
    ).toBe(false);
  });
});

describe("sensitiveExpireDisplay", () => {
  it("treats 0 / negative / non-finite as never", () => {
    expect(sensitiveExpireDisplay(0)).toEqual({ kind: "never" });
    expect(sensitiveExpireDisplay(-10)).toEqual({ kind: "never" });
    expect(sensitiveExpireDisplay(Number.NaN)).toEqual({ kind: "never" });
  });

  it("shows seconds below one minute", () => {
    expect(sensitiveExpireDisplay(10)).toEqual({ kind: "seconds", seconds: 10 });
    expect(sensitiveExpireDisplay(50)).toEqual({ kind: "seconds", seconds: 50 });
  });

  it("shows whole minutes without a seconds remainder", () => {
    expect(sensitiveExpireDisplay(60)).toEqual({ kind: "minutes", minutes: 1 });
    expect(sensitiveExpireDisplay(600)).toEqual({ kind: "minutes", minutes: 10 });
  });

  it("compounds minutes and leftover seconds", () => {
    expect(sensitiveExpireDisplay(70)).toEqual({
      kind: "compound",
      minutes: 1,
      seconds: 10,
    });
  });
});
