import { useSettingsStore } from "../stores/settingsStore";
import type { FilterKind } from "../types";

const ITEMS: { key: FilterKind; label: string }[] = [
  { key: "ALL", label: "All" },
  { key: "IMPORTANT", label: "Important" },
  { key: "STUDY", label: "Study" },
  { key: "TWOFA", label: "2FA" },
];

export default function FilterPanel() {
  const active = useSettingsStore((s) => s.activeFilter);
  const setFilter = useSettingsStore((s) => s.setFilter);
  const blocked = useSettingsStore((s) => s.blockedApps);

  return (
    <nav className="p-3 text-sm">
      <ul className="space-y-1">
        {ITEMS.map((it) => (
          <li key={it.key}>
            <button
              onClick={() => setFilter(it.key)}
              className={
                "w-full rounded px-2 py-1 text-left " +
                (active === it.key
                  ? "bg-bg-secondary text-text-primary"
                  : "text-text-secondary hover:text-text-primary")
              }
            >
              {it.label}
            </button>
          </li>
        ))}
      </ul>

      <hr className="my-3 border-border-subtle" />
      <div className="text-xs uppercase tracking-wide text-text-muted">Blocked</div>
      <ul className="mt-1 space-y-1 text-xs text-text-secondary">
        {blocked.length === 0 && <li className="text-text-muted">None</li>}
        {blocked.map((pkg) => (
          <li key={pkg}>{pkg}</li>
        ))}
      </ul>
    </nav>
  );
}
