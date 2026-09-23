import { describe, expect, it } from "vitest";
import { desktopConnectionStateFromDiagnostics } from "./connectionHealth";

const now = Date.UTC(2026, 4, 5);

describe("desktop connection health", () => {
  it("shows a dropped connection as reconnecting while the backend says so", () => {
    expect(desktopConnectionStateFromDiagnostics(
      { connected: false, reconnectingSince: now - 5_000 }, now,
    )).toBe("RECONNECTING");
  });

  it("stops believing in a reconnect once the grace has run out", () => {
    expect(desktopConnectionStateFromDiagnostics(
      { connected: false, reconnectingSince: now - 90_000 }, now,
    )).toBe("DISCONNECTED");
  });

  it("a drop the backend does not call a reconnect is a disconnection", () => {
    expect(desktopConnectionStateFromDiagnostics(
      { connected: false, reconnectingSince: null }, now,
    )).toBe("DISCONNECTED");
  });

  it("keeps a connected state when a heartbeat is fresh", () => {
    expect(
      desktopConnectionStateFromDiagnostics(
        { connected: true, lastHeartbeatAt: now - 2_000 },
        now,
      ),
    ).toBe("CONNECTED");
  });

  it("downgrades stale heartbeat connections to disconnected", () => {
    expect(
      desktopConnectionStateFromDiagnostics(
        { connected: true, lastHeartbeatAt: now - 121_000 },
        now,
      ),
    ).toBe("DISCONNECTED");
  });

  it("allows a valid delayed Pong and backend scheduling slack", () => {
    expect(
      desktopConnectionStateFromDiagnostics(
        { connected: true, lastHeartbeatAt: now - 10_000 },
        now,
      ),
    ).toBe("CONNECTED");
  });

  it("expires only once the backend has given up too", () => {
    // The window must stay outside the backend's own 90s silence budget: a
    // narrower one reported a disconnection nothing had actually made.
    expect(desktopConnectionStateFromDiagnostics(
      { connected: true, lastHeartbeatAt: now - 89_000 }, now,
    )).toBe("CONNECTED");
    expect(desktopConnectionStateFromDiagnostics(
      { connected: true, lastHeartbeatAt: now - 100_000 }, now,
    )).toBe("CONNECTED");
    expect(desktopConnectionStateFromDiagnostics(
      { connected: true, lastHeartbeatAt: now - 100_001 }, now,
    )).toBe("DISCONNECTED");
  });

  it("honors backend disconnect immediately even with a recent Pong", () => {
    expect(desktopConnectionStateFromDiagnostics(
      { connected: false, lastHeartbeatAt: now }, now,
    )).toBe("DISCONNECTED");
  });

  it("treats connected sockets without first heartbeat as connecting briefly", () => {
    expect(
      desktopConnectionStateFromDiagnostics(
        { connected: true, lastHeartbeatAt: null, connectedAt: now - 8_000 },
        now,
      ),
    ).toBe("CONNECTING");
  });

  it("downgrades connected sockets with no heartbeat after grace window", () => {
    expect(
      desktopConnectionStateFromDiagnostics(
        { connected: true, lastHeartbeatAt: null, connectedAt: now - 31_000 },
        now,
      ),
    ).toBe("DISCONNECTED");
  });
});
