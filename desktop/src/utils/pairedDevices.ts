import type { ConnectionState } from "../types";

export interface PairedDeviceLike {
  deviceName?: string | null;
  endpoint?: string | null;
  isActive: number;
  createdAt: number;
  lastConnectedAt?: number | null;
}

export function summarizePairedDevice(device: PairedDeviceLike, state: ConnectionState) {
  const lastSeen = device.lastConnectedAt ?? device.createdAt;
  return {
    name: device.deviceName?.trim() || "Android phone",
    endpoint: device.endpoint?.trim() || "LAN pairing",
    connected: state === "CONNECTED" && device.isActive !== 0,
    lastSeen,
    lastSeenLabel: relativeTime(lastSeen),
  };
}

export function relativeTime(timestamp?: number | null): string {
  if (!timestamp) return "Never connected";
  const diff = Date.now() - timestamp;
  if (diff < 60_000) return "Connected just now";
  if (diff < 3_600_000) return `${Math.max(1, Math.round(diff / 60_000))} min ago`;
  if (diff < 86_400_000) return `${Math.max(1, Math.round(diff / 3_600_000))} hr ago`;
  return `${Math.max(1, Math.round(diff / 86_400_000))} day ago`;
}
