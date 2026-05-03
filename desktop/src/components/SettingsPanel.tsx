import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNotificationStore } from "../stores/notificationStore";
import { useSettingsStore } from "../stores/settingsStore";

export default function SettingsPanel() {
  const [customDays, setCustomDays] = useState("14");
  const [clearMessage, setClearMessage] = useState<string | null>(null);
  const studyMode = useSettingsStore((s) => s.studyModeEnabled);
  const twoFaMode = useSettingsStore((s) => s.twoFaModeEnabled);
  const setTwoFaMode = useSettingsStore((s) => s.setTwoFaMode);
  const priorityKeywords = useSettingsStore((s) => s.priorityKeywords);
  const syncMode = useSettingsStore((s) => s.syncMode);
  const clearOlderThan = useNotificationStore((s) => s.clearOlderThan);

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
        <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
          Priority words
        </div>
        <div className="mt-3 flex flex-wrap gap-2">
          {priorityKeywords.map((word) => (
            <span key={word} className="rounded-full bg-bg-surface px-3 py-1 text-xs text-text-secondary">
              {word}
            </span>
          ))}
        </div>
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
