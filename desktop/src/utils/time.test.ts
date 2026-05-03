import { describe, it, expect } from "vitest";
import { relativeTime } from "./time";

describe("relativeTime", () => {
  const now = 1_700_000_000_000;
  it("just now under a minute", () => {
    expect(relativeTime(now - 5_000, now)).toBe("just now");
  });
  it("minutes", () => {
    expect(relativeTime(now - 120_000, now)).toBe("2 min ago");
  });
  it("hours", () => {
    expect(relativeTime(now - 2 * 3600_000, now)).toBe("2 hr ago");
  });
  it("days", () => {
    expect(relativeTime(now - 3 * 86400_000, now)).toBe("3 d ago");
  });
});
