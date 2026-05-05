import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNotificationStore } from "../stores/notificationStore";
import { useSettingsStore } from "../stores/settingsStore";
import {
  lockTimeoutLabel,
  lockTimeoutMinutesFrom,
  type LockTimeoutUnit,
} from "../utils/lockTimeout";

interface DiagnosticsSnapshot {
  connected: boolean;
  activeTransport: string;
  lanPort: number;
  endpointCandidates: string[];
  certificateFingerprint: string;
  pairingActive: boolean;
  lastHeartbeatAt?: number | null;
  lastAuthFailure?: string | null;
  lastDisconnectReason?: string | null;
}

export default function SettingsPanel() {
  const [customDays, setCustomDays] = useState("14");
  const [lockValue, setLockValue] = useState("15");
  const [lockUnit, setLockUnit] = useState<LockTimeoutUnit>("minute");
  const [clearMessage, setClearMessage] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsSnapshot | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [windowsSetupMessage, setWindowsSetupMessage] = useState<string | null>(null);
  const studyMode = useSettingsStore((s) => s.studyModeEnabled);
  const twoFaMode = useSettingsStore((s) => s.twoFaModeEnabled);
  const setTwoFaMode = useSettingsStore((s) => s.setTwoFaMode);
  const syncMode = useSettingsStore((s) => s.syncMode);
  const lockTimeoutMinutes = useSettingsStore((s) => s.lockTimeoutMinutes);
  const replaceSettings = useSettingsStore((s) => s.replace);
  const clearAll = useNotificationStore((s) => s.clear);
  const clearOlderThan = useNotificationStore((s) => s.clearOlderThan);

  const refreshDiagnostics = () => {
    invoke<DiagnosticsSnapshot>("get_connection_diagnostics")
      .then((snapshot) => {
        setDiagnostics(snapshot);
        setDiagnosticsError(null);
      })
      .catch((error) => setDiagnosticsError(String(error)));
  };

  useEffect(() => {
    refreshDiagnostics();
    const timer = window.setInterval(refreshDiagnostics, 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const clearHistory = async (days: number) => {
    const cutoffMs = Date.now() - days * 24 * 60 * 60 * 1000;
    try {
      const deleted = await invoke<number>("clear_notifications_older_than", { cutoffMs });
      clearOlderThan(cutoffMs);
      setClearMessage(`Cleared ${deleted} notifications older than ${days} day${days === 1 ? "" : "s"}.`);
    } catch (error) {
      setClearMessage(`Clear failed: ${String(error)}`);
    }
  };

  const clearEverything = async () => {
    try {
      const deleted = await invoke<number>("clear_all_notifications");
      clearAll();
      setClearMessage(`Cleared ${deleted} desktop notifications.`);
    } catch (error) {
      setClearMessage(`Clear failed: ${String(error)}`);
    }
  };

  const saveLockTimeout = (minutes: number) => {
    replaceSettings({ lockTimeoutMinutes: minutes });
    window.localStorage.setItem("focusbridge.lockTimeoutMinutes", String(minutes));
    window.dispatchEvent(new Event("focusbridge://lock-timeout-updated"));
    void invoke("set_lock_timeout_minutes", { minutes }).catch((error) => {
      setWindowsSetupMessage(`Lock timeout update failed: ${String(error)}`);
    });
  };

  const saveCustomLockTimeout = () => {
    const value = Number.parseFloat(lockValue);
    saveLockTimeout(lockTimeoutMinutesFrom(value, lockUnit));
  };

  return (
    <section className="glass-panel rounded-[32px] p-5">
      <p className="text-xs font-semibold uppercase tracking-[0.24em] text-text-muted">
        Focus rules
      </p>
      <h3 className="mt-2 text-2xl font-semibold tracking-[-0.035em]">
        Make the phone quieter, not invisible.
      </h3>

      <div className="mt-5 space-y-3">
        <RuleRow
          label="Study Mode"
          value={studyMode ? "Filtering low priority alerts" : "Ready when you are"}
          active={studyMode}
        />
        <button
          className="rule-row w-full text-left"
          onClick={() => setTwoFaMode(!twoFaMode)}
        >
          <span>
            <strong>2FA fast lane</strong>
            <small>{twoFaMode ? "Security codes always surface" : "Tap to prioritize codes"}</small>
          </span>
          <span className={twoFaMode ? "toggle-dot active" : "toggle-dot"} />
        </button>
        <RuleRow label="Sync mode" value={syncMode === "LOCAL" ? "Local LAN only" : "Cloud relay"} active />
      </div>

      <div className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
              Connection diagnostics
            </div>
            <p className="mt-2 text-sm leading-5 text-text-secondary">
              Local mode uses port 9173. If university Wi-Fi blocks device-to-device traffic, use
              hotspot until relay mode ships.
            </p>
          </div>
          <button
            onClick={refreshDiagnostics}
            className="rounded-full border border-border-subtle px-3 py-2 text-xs font-semibold text-text-secondary transition hover:border-border-hover hover:text-text-primary"
          >
            Refresh
          </button>
        </div>
        {diagnosticsError && <p className="mt-3 text-xs text-[#9b4b3d]">{diagnosticsError}</p>}
        {diagnostics && (
          <div className="mt-4 space-y-3 text-xs text-text-secondary">
            <DiagnosticLine
              label="State"
              value={diagnostics.connected ? "Connected" : "Disconnected"}
              good={diagnostics.connected}
            />
            <DiagnosticLine label="Transport" value={diagnostics.activeTransport} />
            <DiagnosticLine label="LAN port" value={String(diagnostics.lanPort)} />
            <DiagnosticLine
              label="Last heartbeat"
              value={
                diagnostics.lastHeartbeatAt
                  ? new Date(diagnostics.lastHeartbeatAt).toLocaleTimeString()
                  : "Waiting for phone ping"
              }
            />
            {diagnostics.lastAuthFailure && (
              <DiagnosticLine label="Auth failure" value={diagnostics.lastAuthFailure} bad />
            )}
            {diagnostics.lastDisconnectReason && (
              <DiagnosticLine label="Disconnect reason" value={diagnostics.lastDisconnectReason} />
            )}
            <div>
              <div className="font-semibold text-text-primary">Desktop endpoints</div>
              <div className="mt-2 max-h-24 overflow-auto rounded-2xl bg-bg-primary/70 p-3 font-mono text-[11px] leading-5">
                {diagnostics.endpointCandidates.map((endpoint) => (
                  <div key={endpoint}>{endpoint}</div>
                ))}
              </div>
            </div>
            <div>
              <div className="font-semibold text-text-primary">Certificate fingerprint</div>
              <div className="mt-2 max-h-20 overflow-auto break-all rounded-2xl bg-bg-primary/70 p-3 font-mono text-[11px] leading-5">
                {diagnostics.certificateFingerprint}
              </div>
            </div>
          </div>
        )}
      </div>

      <div className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
              App lock
            </div>
            <p className="mt-2 text-sm leading-5 text-text-secondary">
              Choose how long FocusBridge stays unlocked after activity. Default is unlimited.
            </p>
          </div>
          <div className="rounded-2xl bg-bg-primary/80 px-3 py-2 text-right">
            <div className="text-[10px] font-black uppercase tracking-[0.18em] text-text-muted">
              Current
            </div>
            <div className="text-sm font-black text-text-primary">
              {lockTimeoutLabel(lockTimeoutMinutes)}
            </div>
          </div>
        </div>
        <div className="mt-4 flex gap-2 overflow-x-auto pb-2">
          {[
            [0, "Unlimited"],
            [5, "5 min"],
            [15, "15 min"],
            [60, "1 hr"],
            [240, "4 hr"],
            [1_440, "1 day"],
            [10_080, "7 days"],
            [43_200, "1 month"],
          ].map(([minutes, label]) => (
            <button
              key={minutes}
              onClick={() => saveLockTimeout(Number(minutes))}
              className={
                "shrink-0 rounded-full border px-3 py-2 text-xs font-black transition " +
                (lockTimeoutMinutes === Number(minutes)
                  ? "border-text-primary bg-text-primary text-bg-primary"
                  : "border-border-subtle bg-bg-primary/70 text-text-secondary hover:border-border-hover hover:text-text-primary")
              }
            >
              {label}
            </button>
          ))}
        </div>
        <div className="mt-3 rounded-3xl border border-border-subtle bg-bg-primary/60 p-3">
          <div className="flex gap-2">
            <input
              value={lockValue}
              onChange={(event) => setLockValue(event.target.value)}
              className="min-w-0 flex-1 rounded-2xl border border-border-subtle bg-bg-secondary/80 px-4 py-3 text-sm text-text-primary outline-none transition focus:border-border-hover"
              inputMode="decimal"
              placeholder="Custom"
            />
            <button
              onClick={saveCustomLockTimeout}
              className="rounded-2xl bg-text-primary px-4 py-3 text-sm font-black text-bg-primary transition hover:bg-accent-study active:scale-95"
            >
              Set
            </button>
          </div>
          <div className="mt-3 grid grid-cols-4 gap-2">
            {(["minute", "hour", "day", "month"] as const).map((unit) => (
              <button
                key={unit}
                onClick={() => setLockUnit(unit)}
                className={
                  "rounded-full border px-2 py-2 text-[11px] font-black uppercase tracking-[0.12em] transition " +
                  (lockUnit === unit
                    ? "border-accent-study bg-[#dfeee6] text-accent-study"
                    : "border-border-subtle bg-bg-secondary/80 text-text-muted hover:border-border-hover hover:text-text-primary")
                }
              >
                {unit === "minute" ? "min" : unit === "hour" ? "hr" : unit === "day" ? "day" : "mo"}
              </button>
            ))}
          </div>
          <p className="mt-3 text-xs leading-5 text-text-muted">
            Custom value saves as {lockTimeoutLabel(lockTimeoutMinutesFrom(Number.parseFloat(lockValue), lockUnit))}.
            Maximum is 12 months.
          </p>
        </div>
        <p className="mt-3 text-xs leading-5 text-text-muted">
          Local unlock now accepts either an 8+ character password or a 4+ digit PIN. Native
          biometric unlock needs Windows Hello/macOS Touch ID integration and is tracked separately.
        </p>
      </div>

      <div className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
        <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
          Windows setup
        </div>
        <p className="mt-2 text-sm leading-5 text-text-secondary">
          Add a Windows firewall allow rule for FocusBridge local sync. Installed builds use the
          FocusBridge app identity for native toasts; dev launches from PowerShell can still show
          shell identity because Windows ties toast identity to the packaged shortcut.
        </p>
        <button
          onClick={() => {
            setWindowsSetupMessage("Requesting Windows permission...");
            invoke<string>("run_windows_first_run_setup")
              .then(setWindowsSetupMessage)
              .catch((error) => setWindowsSetupMessage(`Windows setup failed: ${String(error)}`));
          }}
          className="mt-4 w-full rounded-full bg-text-primary px-4 py-2 text-sm font-semibold text-bg-primary transition hover:bg-accent-study active:scale-95"
        >
          Allow FocusBridge through Windows Firewall
        </button>
        {windowsSetupMessage && (
          <p className="mt-3 text-xs leading-5 text-text-muted">{windowsSetupMessage}</p>
        )}
      </div>

      <div className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
        <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
          Clear history
        </div>
        <p className="mt-2 text-sm leading-5 text-text-secondary">
          Delete old notification records from desktop storage. This does not affect your phone apps.
        </p>
        <div className="mt-4 grid grid-cols-3 gap-2">
          {[1, 7, 30].map((days) => (
            <button
              key={days}
              onClick={() => void clearHistory(days)}
              className="rounded-full border border-border-subtle bg-bg-primary/60 px-3 py-2 text-xs font-semibold text-text-secondary transition hover:-translate-y-0.5 hover:border-border-hover hover:text-text-primary active:translate-y-0"
            >
              {days === 30 ? "1 month" : `${days} day${days === 1 ? "" : "s"}`}
            </button>
          ))}
        </div>
        <button
          onClick={() => void clearEverything()}
          className="mt-3 w-full rounded-full border border-[#f0b8aa] bg-[#fff0eb] px-4 py-2 text-sm font-semibold text-[#8f3324] transition hover:-translate-y-0.5 hover:bg-[#ffe4dc] active:translate-y-0 active:scale-95"
        >
          Clear all desktop messages
        </button>
        <div className="mt-3 flex gap-2">
          <input
            value={customDays}
            onChange={(event) => setCustomDays(event.target.value)}
            className="min-w-0 flex-1 rounded-full border border-border-subtle bg-bg-primary/80 px-4 py-2 text-sm outline-none transition focus:border-border-hover"
            inputMode="numeric"
            placeholder="Custom days"
          />
          <button
            onClick={() => {
              const days = Number.parseInt(customDays, 10);
              if (Number.isFinite(days) && days > 0) void clearHistory(days);
            }}
            className="rounded-full bg-text-primary px-4 py-2 text-sm font-semibold text-bg-primary transition hover:bg-accent-study active:scale-95"
          >
            Clear
          </button>
        </div>
        {clearMessage && <p className="mt-3 text-xs text-text-muted">{clearMessage}</p>}
      </div>
    </section>
  );
}

function DiagnosticLine({
  label,
  value,
  good,
  bad,
}: {
  label: string;
  value: string;
  good?: boolean;
  bad?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="font-semibold text-text-primary">{label}</span>
      <span className={good ? "text-[#2f8f61]" : bad ? "text-[#9b4b3d]" : "text-text-secondary"}>
        {value}
      </span>
    </div>
  );
}

function RuleRow({
  label,
  value,
  active,
}: {
  label: string;
  value: string;
  active?: boolean;
}) {
  return (
    <div className="rule-row">
      <span>
        <strong>{label}</strong>
        <small>{value}</small>
      </span>
      <span className={active ? "toggle-dot active" : "toggle-dot"} />
    </div>
  );
}
