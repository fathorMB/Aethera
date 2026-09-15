import { describe, expect, it } from "vitest";
import { duration, getPath, sameValue, setPath } from "./format";

describe("format", () => {
  it("durations read like the mockup", () => {
    expect(duration(42)).toBe("42 s");
    expect(duration(4380)).toBe("1 h 13 m");
    expect(duration(2 * 86400 + 4 * 3600)).toBe("2 g 4 h");
  });

  it("setPath does not touch the original", () => {
    const base = { server: { ubatch: 4096 }, speculative: { type: "none" } };
    const next = setPath(base, "server.ubatch", 2048);
    expect(getPath(next, "server.ubatch")).toBe(2048);
    expect(base.server.ubatch).toBe(4096);
  });

  it("absent equals null, not zero", () => {
    expect(sameValue(undefined, null)).toBe(true);
    expect(sameValue(undefined, [])).toBe(true);
    expect(sameValue(0, null)).toBe(false);
  });
});
