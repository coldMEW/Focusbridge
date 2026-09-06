import type { ConnectionState } from "../types";

// Backend probes allow 3s idle + 6s response; leave room for timer and IPC scheduling.
export const STALE_HEARTBEAT_MS = 12_000;
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
