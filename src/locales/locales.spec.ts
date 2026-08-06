import { describe, expect, it } from "vitest";
import zhCN from "./zh-CN";
import enUS from "./en-US";

function leafPaths(value: unknown, prefix = ""): string[] {
  if (typeof value !== "object" || value === null) return [prefix];
  return Object.entries(value).flatMap(([key, child]) =>
    leafPaths(child, prefix ? `${prefix}.${key}` : key),
  );
}

function placeholders(value: unknown, prefix = ""): Map<string, string[]> {
  const output = new Map<string, string[]>();
  if (typeof value === "string") {
    output.set(prefix, [...value.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort());
    return output;
  }
  if (typeof value !== "object" || value === null) return output;
  for (const [key, child] of Object.entries(value)) {
    for (const [path, names] of placeholders(child, prefix ? `${prefix}.${key}` : key)) {
      output.set(path, names);
    }
  }
  return output;
}

describe("locale parity", () => {
  it("keeps the same leaf keys in both locales", () => {
    expect(leafPaths(enUS).sort()).toEqual(leafPaths(zhCN).sort());
  });

  it("keeps named interpolation placeholders aligned", () => {
    expect([...placeholders(enUS).entries()]).toEqual([...placeholders(zhCN).entries()]);
  });
});
