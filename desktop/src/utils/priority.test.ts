import { describe, it, expect } from "vitest";
import { priorityLevel, priorityBadge } from "./priority";

describe("priorityLevel", () => {
  it("buckets scores", () => {
    expect(priorityLevel(0)).toBe("LOW");
    expect(priorityLevel(20)).toBe("LOW");
    expect(priorityLevel(21)).toBe("NORMAL");
    expect(priorityLevel(50)).toBe("NORMAL");
    expect(priorityLevel(51)).toBe("HIGH");
    expect(priorityLevel(80)).toBe("HIGH");
    expect(priorityLevel(81)).toBe("CRITICAL");
    expect(priorityLevel(100)).toBe("CRITICAL");
  });
});

describe("priorityBadge", () => {
  it("returns a star for critical", () => {
    expect(priorityBadge(95)).toBe("★");
  });
  it("returns empty for normal", () => {
    expect(priorityBadge(40)).toBe("");
  });
});
