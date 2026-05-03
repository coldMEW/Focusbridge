import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../stores/settingsStore";
import type { FilterKind } from "../types";

const ITEMS: { key: FilterKind; label: string; hint: string }[] = [
  { key: "ALL", label: "Inbox", hint: "Everything not dismissed" },
  { key: "IMPORTANT", label: "Priority", hint: "Pinned and urgent" },
  { key: "STUDY", label: "Study lane", hint: "Low-interruption mode" },
  { key: "TWOFA", label: "Security", hint: "Codes and sign-ins" },
];

export default function FilterPanel() {
  const active = useSettingsStore((s) => s.activeFilter);
  const setFilter = useSettingsStore((s) => s.setFilter);
  const studyMode = useSettingsStore((s) => s.studyModeEnabled);
  const setStudyMode = useSettingsStore((s) => s.setStudyMode);
  const blocked = useSettingsStore((s) => s.blockedApps);

  const selectFilter = (filter: FilterKind) => {
    setFilter(filter);
    if (filter === "STUDY") {
      setStudyMode(true);
      void invoke("set_study_mode", { on: true }).catch((error) => {
        console.warn("Unable to persist Study Mode", error);
      });
    }
  };

  return (
    <nav className="flex-1 text-sm">
      <ul className="space-y-2">
        {ITEMS.map((it) => (
          <li key={it.key}>
            <button
              onClick={() => selectFilter(it.key)}
              className={
                "w-full rounded-2xl px-3 py-3 text-left transition-all " +
                (active === it.key
                  ? "bg-text-primary text-bg-primary shadow-soft"
                  : "text-text-secondary hover:bg-bg-secondary hover:text-text-primary")
              }
            >
              <span className="block font-semibold">{it.label}</span>
              <span
                className={
                  "mt-0.5 block text-xs " +
                  (active === it.key ? "text-bg-secondary" : "text-text-muted")
                }
              >
                {it.hint}
              </span>
              {it.key === "STUDY" && (
                <span
                  className={
                    "mt-2 inline-flex rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.16em] " +
                    (studyMode
                      ? active === it.key
                        ? "bg-bg-primary/20 text-bg-primary"
                        : "bg-accent-study/15 text-accent-study"
                      : active === it.key
                        ? "bg-bg-primary/15 text-bg-primary"
                        : "bg-bg-secondary text-text-muted")
                  }
                >
                  {studyMode ? "On" : "Tap to enable"}
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>

      <hr className="my-5 border-border-subtle" />
      <div className="text-xs uppercase tracking-[0.22em] text-text-muted">Muted apps</div>
      <ul className="mt-3 space-y-2 text-xs text-text-secondary">
        {blocked.length === 0 && (
          <li className="rounded-2xl bg-bg-secondary/70 px-3 py-2 text-text-muted">
            No blocked apps yet
          </li>
        )}
        {blocked.map((pkg) => (
          <li key={pkg} className="rounded-2xl bg-bg-secondary px-3 py-2">
            {pkg}
          </li>
        ))}
      </ul>
    </nav>
  );
}
