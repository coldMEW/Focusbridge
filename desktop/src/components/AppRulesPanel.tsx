import { invoke } from "@tauri-apps/api/core";
import { useAppRulesStore } from "../stores/appRulesStore";
import { useSettingsStore } from "../stores/settingsStore";
import type { AppRule } from "../types";

const FLAGS = [
  { key: "muted", label: "Mute" },
  { key: "priority", label: "Priority" },
  { key: "study_safe", label: "Study" },
] as const;

export default function AppRulesPanel() {
  const rules = useAppRulesStore((s) => s.items);
  const upsert = useAppRulesStore((s) => s.upsert);
  const setAppRuleLists = useSettingsStore((s) => s.setAppRuleLists);

  const updateRule = async (
    rule: AppRule,
    flag: (typeof FLAGS)[number]["key"],
    enabled: boolean,
  ) => {
    const updated = await invoke<AppRule>("set_app_rule", {
      packageName: rule.packageName,
      flag,
      enabled,
    });
    upsert(updated);
    setAppRuleLists(
      rules.map((item) => (item.packageName === updated.packageName ? updated : item)),
    );
  };

  return (
    <div className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
      <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
        Phone app controls
      </div>
      <p className="mt-2 text-sm leading-5 text-text-secondary">
        Apps appear after your phone connects. Mute hides an app on desktop; Priority and Study
        place it into focused lanes.
      </p>
      <div className="mt-4 max-h-[360px] space-y-2 overflow-y-auto pr-1">
        {rules.length === 0 && (
          <div className="rounded-2xl bg-bg-primary/70 px-3 py-3 text-sm text-text-muted">
            Connect your phone to import its app list.
          </div>
        )}
        {rules.map((rule) => (
          <div
            key={rule.packageName}
            className="rounded-2xl border border-border-subtle bg-bg-primary/70 p-3"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="truncate text-sm font-semibold text-text-primary">{rule.label}</div>
                <div className="truncate text-[11px] text-text-muted">{rule.packageName}</div>
              </div>
              <span className="rounded-full bg-bg-secondary px-2 py-1 text-[10px] font-bold uppercase tracking-[0.14em] text-text-secondary">
                {rule.category.replace("_", " ")}
              </span>
            </div>
            <div className="mt-3 grid grid-cols-3 gap-2">
              {FLAGS.map((flag) => {
                const enabled = flag.key === "muted"
                  ? rule.muted !== 0
                  : flag.key === "priority"
                    ? rule.priority !== 0
                    : rule.studySafe !== 0;
                return (
                  <button
                    key={flag.key}
                    onClick={() => void updateRule(rule, flag.key, !enabled)}
                    className={
                      "rounded-full px-2 py-1.5 text-xs font-semibold transition active:scale-95 " +
                      (enabled
                        ? "bg-text-primary text-bg-primary"
                        : "border border-border-subtle text-text-secondary hover:border-border-hover hover:text-text-primary")
                    }
                  >
                    {flag.label}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
