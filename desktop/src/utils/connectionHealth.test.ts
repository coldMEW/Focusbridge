import { describe, expect, it } from "vitest";
import { desktopConnectionStateFromDiagnostics } from "./connectionHealth";

const now = Date.UTC(2026, 4, 5);

describe("desktop connection health", () => {
  it("keeps a connected state when a heartbeat is fresh", () => {
    expect(
      desktopConnectionStateFromDiagnostics(
        { connected: true, lastHeartbeatAt: now - 10_000 },
        now,
      ),
    ).toBe("CONNECTED");
  });

  it("downgrades stale heartbeat connections to disconnected", () => {
    expect(
      desktopConnectionStateFromDiagnostics(
        { connected: true, lastHeartbeatAt: now - 46_000 },
        now,
      ),
    ).toBe("DISCONNECTED");
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
        { connected: true, lastHeartbeatAt: null, connectedAt: now - 16_000 },
        now,
      ),
    ).toBe("DISCONNECTED");
  });
});
