import { describe, expect, it } from "vitest";
import {
  ACCOUNT_SESSION_DAYS,
  accountSessionExpired,
  accountSessionLabel,
  accountSessionValid,
  type AccountSession,
} from "./accountSession";

const now = Date.UTC(2026, 4, 5);
const day = 24 * 60 * 60 * 1000;

describe("account session", () => {
  it("accepts firebase sessions younger than 90 days", () => {
    const session: AccountSession = {
      mode: "firebase",
      email: "person@example.com",
      uid: "uid-1",
      lastLoginAt: now - (ACCOUNT_SESSION_DAYS - 1) * day,
    };

    expect(accountSessionValid(session, now)).toBe(true);
    expect(accountSessionExpired(session, now)).toBe(false);
    expect(accountSessionLabel(session)).toBe("person@example.com");
  });

  it("expires firebase sessions at 90 days", () => {
    const session: AccountSession = {
      mode: "firebase",
      email: "person@example.com",
      uid: "uid-1",
      lastLoginAt: now - ACCOUNT_SESSION_DAYS * day - 1,
    };

    expect(accountSessionValid(session, now)).toBe(false);
    expect(accountSessionExpired(session, now)).toBe(true);
  });

  it("accepts guest sessions younger than 90 days", () => {
    const session: AccountSession = {
      mode: "guest",
      lastLoginAt: now - 7 * day,
    };

    expect(accountSessionValid(session, now)).toBe(true);
    expect(accountSessionLabel(session)).toBe("Guest mode");
  });

  it("rejects missing or malformed sessions", () => {
    expect(accountSessionValid(null, now)).toBe(false);
    expect(accountSessionValid({ mode: "firebase" }, now)).toBe(false);
    expect(accountSessionValid({ mode: "guest", lastLoginAt: "bad" }, now)).toBe(false);
  });
});
