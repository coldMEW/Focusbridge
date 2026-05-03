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
  const blocked = useSettingsStore((s) => s.blockedApps);

  return (
    <nav className="flex-1 text-sm">
      <ul className="space-y-2">
        {ITEMS.map((it) => (
          <li key={it.key}>
            <button
              onClick={() => setFilter(it.key)}
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
