import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { firebaseRelayToken, firebaseSendVerificationEmail } from "../lib/firebaseAuth";
import { relayErrorMessage, summarizeRelay, type RelayStatus, type RelayTone } from "../lib/relay";

const TONE_DOT: Record<RelayTone, string> = {
  off: "bg-text-muted",
  waiting: "bg-accent-study",
  ready: "bg-emerald-400",
  expired: "bg-rose-400",
};

/**
 * Cross-network sync: reach this PC from mobile data or any other Wi-Fi.
 *
 * Enabling it provisions one revocable link for the signed-in account. The relay
 * only routes sealed traffic; notification content stays readable to this PC and
 * the paired phone alone.
 */
export default function CrossNetworkPanel() {
  const [status, setStatus] = useState<RelayStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<RelayStatus>("relay_status"));
    } catch (error) {
      setMessage(relayErrorMessage(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (command: "relay_enable" | "relay_disable") => {
      setBusy(true);
      setMessage(null);
      try {
        const token = await firebaseRelayToken();
        if (!token.ok) {
          setMessage(
            token.reason === "signed-out"
              ? "Sign in to your FocusBridge account to use cross-network sync."
              : "Confirm your email address first — cross-network sync needs a verified account.",
          );
          return;
        }
        setStatus(await invoke<RelayStatus>(command, { idToken: token.idToken }));
        setMessage(
          command === "relay_enable"
            ? "Cross-network sync is on. Open the pairing screen and scan the QR on your phone."
            : "Cross-network sync is off. Local network sync still works.",
        );
      } catch (error) {
        setMessage(relayErrorMessage(error));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const resendVerification = useCallback(async () => {
    setBusy(true);
    try {
      await firebaseSendVerificationEmail();
      setMessage("Verification email sent. Open the link, then turn cross-network sync on again.");
    } catch (error) {
      setMessage(relayErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }, []);

  const summary = summarizeRelay(status);
  const enabled = status?.configured === true;

  return (
    <div className="rounded-[32px] border border-border-subtle bg-bg-secondary/70 p-6 shadow-soft">
      <div className="text-xs uppercase tracking-[0.22em] text-text-muted">Cross-network sync</div>
      <div className="mt-3 flex items-center gap-2">
        <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${TONE_DOT[summary.tone]}`} />
        <span className="text-sm font-semibold text-text-primary">{summary.headline}</span>
      </div>
      <p className="mt-2 text-sm leading-5 text-text-secondary">{summary.detail}</p>

      <div className="mt-4 flex flex-wrap gap-3">
        <button
          onClick={() => void run(enabled ? "relay_disable" : "relay_enable")}
          disabled={busy}
          className="rounded-full bg-text-primary px-4 py-2 text-sm font-semibold text-bg-primary transition hover:bg-accent-study disabled:cursor-not-allowed disabled:opacity-50 active:scale-95"
        >
          {busy
            ? "Working…"
            : enabled
              ? "Turn off cross-network sync"
              : "Turn on cross-network sync"}
        </button>
        <button
          onClick={() => void resendVerification()}
          disabled={busy}
          className="rounded-full border border-border-subtle px-4 py-2 text-sm font-semibold text-text-secondary transition hover:border-border-hover disabled:cursor-not-allowed disabled:opacity-50 active:scale-95"
        >
          Resend verification email
        </button>
      </div>

      {message && <p className="mt-3 text-xs leading-5 text-text-muted">{message}</p>}

      <p className="mt-4 text-xs leading-5 text-text-muted">
        Traffic is sealed between this PC and your phone, so the relay routes it without being able
        to read it. Turning this off revokes the link immediately. Your local network stays the
        preferred path whenever both devices are on it.
      </p>
    </div>
  );
}
