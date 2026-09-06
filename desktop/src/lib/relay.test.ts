import { describe, expect, it } from "vitest";
import { describeTransport, relayErrorMessage, summarizeRelay, type RelayStatus } from "./relay";

const NOW = 1_700_000_000_000;

function status(overrides: Partial<RelayStatus> = {}): RelayStatus {
  return {
    configured: true,
    relayUrl: "https://relay.example",
    pairId: "b".repeat(32),
    expiresAt: NOW + 30 * 86_400_000,
    phoneEnrolled: true,
    autoConnect: true,
    ...overrides,
  };
}

describe("summarizeRelay", () => {
  it("presents the LAN-only state as a working default, not a failure", () => {
    for (const value of [null, status({ configured: false })]) {
      const summary = summarizeRelay(value, NOW);
      expect(summary.tone).toBe("off");
      expect(summary.detail).toContain("local network");
    }
  });

  it("does not claim the phone is reachable before it has enrolled", () => {
    const summary = summarizeRelay(status({ phoneEnrolled: false }), NOW);
    expect(summary.tone).toBe("waiting");
    expect(summary.headline).toBe("Waiting for your phone");
  });

  it("reports a ready link and keeps the local network as the preferred path", () => {
    const summary = summarizeRelay(status(), NOW);
    expect(summary.tone).toBe("ready");
    expect(summary.detail).toContain("prefers the local network");
    expect(summary.detail).toContain("30 days");
  });

  it("rounds the remaining lifetime up and singularises the last day", () => {
    expect(summarizeRelay(status({ expiresAt: NOW + 86_400_000 }), NOW).detail).toContain("1 day.");
    expect(summarizeRelay(status({ expiresAt: NOW + 100 }), NOW).detail).toContain("1 day.");
  });

  it("reports expiry rather than a link that would silently fail", () => {
    const summary = summarizeRelay(status({ expiresAt: NOW }), NOW);
    expect(summary.tone).toBe("expired");
    expect(summary.detail).toContain("expired");
  });

  it("tolerates a pair with no recorded expiry", () => {
    const summary = summarizeRelay(status({ expiresAt: null }), NOW);
    expect(summary.tone).toBe("ready");
    expect(summary.detail).not.toContain("Renews");
  });
});

describe("relayErrorMessage", () => {
  it("turns known relay failures into an action the user can take", () => {
    expect(relayErrorMessage("the relay rejected this account; sign in again and retry")).toContain(
      "Sign in again",
    );
    expect(
      relayErrorMessage("this account already has the maximum number of paired devices"),
    ).toContain("device limit");
    expect(relayErrorMessage("too many pairing attempts on this account; try again later")).toContain(
      "Wait an hour",
    );
    expect(relayErrorMessage(new Error("reach the FocusBridge relay"))).toContain(
      "Internet connection",
    );
  });

  it("keeps an unexpected message instead of hiding it", () => {
    expect(relayErrorMessage("relay returned a malformed pair")).toBe(
      "relay returned a malformed pair",
    );
    expect(relayErrorMessage(undefined)).toContain("could not be changed");
  });
});

describe("describeTransport", () => {
  it("names the route actually in use", () => {
    expect(describeTransport(true, "relay")).toBe("Cross-network relay");
    expect(describeTransport(true, "wss")).toBe("Local network");
    expect(describeTransport(true, "ws_legacy")).toBe("Local network (legacy pairing)");
  });

  it("never claims a route while disconnected", () => {
    expect(describeTransport(false, "relay")).toBe("Not connected");
    expect(describeTransport(false, "wss")).toBe("Not connected");
  });

  it("does not invent a transport it was not told about", () => {
    expect(describeTransport(true, null)).toBe("Connected");
    expect(describeTransport(true, "")).toBe("Connected");
    expect(describeTransport(true, "something-new")).toBe("Connected");
  });
});
