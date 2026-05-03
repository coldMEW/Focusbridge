import { useSettingsStore } from "../stores/settingsStore";

export default function SettingsPanel() {
  const studyMode = useSettingsStore((s) => s.studyModeEnabled);
  const twoFaMode = useSettingsStore((s) => s.twoFaModeEnabled);
  const setTwoFaMode = useSettingsStore((s) => s.setTwoFaMode);
  const priorityKeywords = useSettingsStore((s) => s.priorityKeywords);
  const syncMode = useSettingsStore((s) => s.syncMode);

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
