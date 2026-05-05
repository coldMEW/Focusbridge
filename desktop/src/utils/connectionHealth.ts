import type { ConnectionState } from "../types";

export const STALE_HEARTBEAT_MS = 45_000;
export const FIRST_HEARTBEAT_GRACE_MS = 15_000;

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
