export type NotificationStatus = "NEW" | "READ" | "IGNORED" | "IMPORTANT";

export type ConnectionState =
  | "DISCONNECTED"
  | "CONNECTING"
  | "CONNECTED"
  | "PAUSED";

export interface Notification {
  id: string;
  appName: string;
  packageName: string;
  sender: string;
  message: string;
  timestamp: number;
  receivedAt: number;
  status: NotificationStatus;
  priority: number;
  contentHidden: boolean;
  batchId?: string | null;
}

export interface PairingInfo {
  deviceId: string;
  deviceName?: string | null;
  endpoint: string;
  pairingKey: string;
  certFingerprint: string;
  mode: "LOCAL" | "CLOUD";
}

export interface Settings {
  studyModeEnabled: boolean;
  blockedApps: string[];
  priorityApps: string[];
  favoriteContacts: string[];
  priorityKeywords: string[];
  twoFaModeEnabled: boolean;
  syncMode: "LOCAL" | "CLOUD";
}

export type FilterKind = "ALL" | "IMPORTANT" | "STUDY" | "TWOFA";
