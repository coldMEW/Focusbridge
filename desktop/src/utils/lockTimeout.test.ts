import { describe, expect, it } from "vitest";
import {
  MAX_LOCK_TIMEOUT_MINUTES,
  lockTimeoutLabel,
  lockTimeoutMinutesFrom,
} from "./lockTimeout";

describe("lock timeout utility", () => {
  it("converts minutes, hours, days, and months into minutes", () => {
    expect(lockTimeoutMinutesFrom(15, "minute")).toBe(15);
    expect(lockTimeoutMinutesFrom(4, "hour")).toBe(240);
    expect(lockTimeoutMinutesFrom(7, "day")).toBe(10_080);
    expect(lockTimeoutMinutesFrom(1, "month")).toBe(43_200);
  });

  it("clamps invalid and huge values", () => {
    expect(lockTimeoutMinutesFrom(0, "hour")).toBe(0);
    expect(lockTimeoutMinutesFrom(-4, "day")).toBe(0);
    expect(lockTimeoutMinutesFrom(99, "month")).toBe(MAX_LOCK_TIMEOUT_MINUTES);
  });

  it("formats stored minutes into readable labels", () => {
    expect(lockTimeoutLabel(0)).toBe("Unlimited");
    expect(lockTimeoutLabel(1)).toBe("1 minute");
    expect(lockTimeoutLabel(60)).toBe("1 hour");
    expect(lockTimeoutLabel(240)).toBe("4 hours");
    expect(lockTimeoutLabel(1_440)).toBe("1 day");
    expect(lockTimeoutLabel(43_200)).toBe("1 month");
    expect(lockTimeoutLabel(86_400)).toBe("2 months");
  });
});
