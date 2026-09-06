/**
 * Presentation logic for cross-network sync.
 *
 * The relay is opt-in and additive: LAN pairing works with no account at all, so
 * every state here has to read as an optional upgrade rather than a broken setup.
 */

export interface RelayStatus {
  configured: boolean;
  relayUrl: string;
  pairId: string | null;
  expiresAt: number | null;
  phoneEnrolled: boolean;
  autoConnect: boolean;
}

export type RelayTone = "off" | "waiting" | "ready" | "expired";

export interface RelaySummary {
  tone: RelayTone;
  headline: string;
  detail: string;
}

const DAY_MS = 86_400_000;

/**
 * Describes the relay state without ever claiming the phone is connected: a
 * provisioned pair is only a route, and an enrolled phone is only a pairing.
 * Live connection state comes from the connection diagnostics, not from here.
 */
export function summarizeRelay(status: RelayStatus | null, now: number = Date.now()): RelaySummary {
  if (!status || !status.configured) {
    return {
      tone: "off",
      headline: "Same network only",
      detail:
        "FocusBridge is syncing over your local network. Turn on cross-network sync to reach this PC from mobile data or any other Wi-Fi.",
    };
  }
  if (status.expiresAt !== null && status.expiresAt <= now) {
    return {
      tone: "expired",
      headline: "Cross-network sync expired",
      detail: "This device link has expired. Turn cross-network sync on again to renew it.",
    };
  }
  const renew = status.expiresAt === null ? "" : ` Renews in ${daysUntil(status.expiresAt, now)}.`;
  if (!status.phoneEnrolled) {
    return {
      tone: "waiting",
      headline: "Waiting for your phone",
      detail: `Scan the pairing QR on your phone to finish setting up cross-network sync.${renew}`,
    };
  }
  return {
    tone: "ready",
    headline: "Cross-network sync is on",
    detail: `Your phone can reach this PC from any network. FocusBridge still prefers the local network when both are on it.${renew}`,
  };
}

function daysUntil(expiresAt: number, now: number): string {
  const days = Math.max(0, Math.ceil((expiresAt - now) / DAY_MS));
  return days === 1 ? "1 day" : `${days} days`;
}

/**
 * Turns a backend or Firebase failure into something a person can act on. The
 * raw text is kept as a fallback so a genuinely unexpected error is not hidden.
 */
export function relayErrorMessage(error: unknown): string {
  const raw = typeof error === "string" ? error : error instanceof Error ? error.message : "";
  const text = raw.toLowerCase();
  if (text.includes("sign in again") || text.includes("rejected this account")) {
    return "Your sign-in has expired. Sign in again, then retry.";
  }
  if (text.includes("maximum number of paired devices")) {
    return "This account has reached its device limit. Remove a device, then retry.";
  }
  if (text.includes("too many pairing attempts")) {
    return "Too many pairing attempts on this account. Wait an hour, then retry.";
  }
  if (text.includes("reach the focusbridge relay") || text.includes("timed out")) {
    return "Could not reach the relay. Check this PC's Internet connection and retry.";
  }
  return raw || "Cross-network sync could not be changed. Please retry.";
}

/**
 * How the phone is actually reaching this PC right now.
 *
 * This reads the live transport the server recorded, not a stored preference:
 * the pairing carries both a local address and a relay route, so which one is
 * in use is a fact about the current connection and can change without any
 * setting changing.
 */
export function describeTransport(
  connected: boolean,
  activeTransport: string | null | undefined,
): string {
  if (!connected) return "Not connected";
  switch ((activeTransport ?? "").toLowerCase()) {
    case "relay":
      return "Cross-network relay";
    case "wss":
      return "Local network";
    case "ws_legacy":
      return "Local network (legacy pairing)";
    default:
      return "Connected";
  }
}
