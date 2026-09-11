import type { ConnectionState } from "../types";

// The backend probes every 15s and gives up on the phone after 90s of complete
// silence. This window has to sit outside that, or the interface reports a
// disconnection the backend never made -- which is how a perfectly live session
// showed as dropped after one late pong.
export const STALE_HEARTBEAT_MS = 100_000;
export const FIRST_HEARTBEAT_GRACE_MS = 30_000;

export interface ConnectionHealthSnapshot {
  connected: boolean;
  lastHeartbeatAt?: number | null;
  connectedAt?: number | null;
}

export function desktopConnectionStateFromDiagnostics(
  diagnostics: ConnectionHealthSnapshot,
  now = Date.now(),
): ConnectionState {
  if (!diagnostics.connected) return "DISCONNECTED";

  if (typeof diagnostics.lastHeartbeatAt === "number") {
    return now - diagnostics.lastHeartbeatAt <= STALE_HEARTBEAT_MS
      ? "CONNECTED"
      : "DISCONNECTED";
  }

  if (typeof diagnostics.connectedAt === "number") {
    return now - diagnostics.connectedAt <= FIRST_HEARTBEAT_GRACE_MS
      ? "CONNECTING"
      : "DISCONNECTED";
  }

  return "CONNECTING";
}
