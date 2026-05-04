import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppRulesStore } from "../stores/appRulesStore";
import { useSettingsStore } from "../stores/settingsStore";
import type { AppRule } from "../types";

const FLAGS = [
  { key: "muted", label: "Mute" },
  { key: "priority", label: "Priority" },
  { key: "study_safe", label: "Study" },
] as const;

const CATEGORY_LABELS: Record<string, string> = {
  messaging: "Messages",
  social: "Social media",
  learning: "Learning",
  email: "Mail",
  school_work: "School and work",
  finance: "Finance",
  shopping: "Shopping",
  media: "Media",
  other: "Other apps",
};

export default function AppRulesPanel() {
  const rules = useAppRulesStore((s) => s.items);
  const upsert = useAppRulesStore((s) => s.upsert);
  const setAppRuleLists = useSettingsStore((s) => s.setAppRuleLists);
  const priorityKeywords = useSettingsStore((s) => s.priorityKeywords);
  const blockedKeywords = useSettingsStore((s) => s.blockedKeywords);
  const replaceSettings = useSettingsStore((s) => s.replace);
  const [priorityText, setPriorityText] = useState(priorityKeywords.join(", "));
  const [blockedText, setBlockedText] = useState(blockedKeywords.join(", "));
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => setPriorityText(priorityKeywords.join(", ")), [priorityKeywords]);
  useEffect(() => setBlockedText(blockedKeywords.join(", ")), [blockedKeywords]);

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
    setAppRuleLists(rules.map((item) => (item.packageName === updated.packageName ? updated : item)));
  };

  const saveWords = async (key: "priority_keywords" | "blocked_keywords", raw: string) => {
    const values = raw
      .split(/[,\n]/)
      .map((value) => value.trim())
      .filter(Boolean);
    await invoke("set_rule_text", { key, values });
    replaceSettings(
      key === "priority_keywords"
        ? { priorityKeywords: values }
        : { blockedKeywords: values },
    );
    setStatus("Synced rules to phone.");
  };

  const grouped = groupRules(rules);

  return (
    <section className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
            Phone app controls
          </div>
          <p className="mt-2 text-sm leading-5 text-text-secondary">
            Control what the phone sends before it reaches desktop.
          </p>
        </div>
        <span className="rounded-full bg-bg-primary px-3 py-1 text-xs font-bold text-text-secondary">
          {rules.length} apps
        </span>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-2">
        <WordEditor
          label="Priority words"
          value={priorityText}
          onChange={setPriorityText}
          onSave={() => void saveWords("priority_keywords", priorityText)}
        />
        <WordEditor
          label="Blocked words"
          value={blockedText}
          onChange={setBlockedText}
          onSave={() => void saveWords("blocked_keywords", blockedText)}
        />
      </div>
      {status && <p className="mt-3 text-xs text-text-muted">{status}</p>}

      <div className="mt-5 max-h-[520px] space-y-5 overflow-y-auto pr-1">
        {rules.length === 0 && (
          <div className="rounded-2xl bg-bg-primary/70 px-3 py-3 text-sm text-text-muted">
            Connect your phone to import its app list.
          </div>
        )}
        {grouped.map(([category, items]) => (
          <div key={category}>
            <div className="mb-2 flex items-center justify-between">
              <h4 className="text-sm font-black text-text-primary">
                {CATEGORY_LABELS[category] ?? category}
              </h4>
              <span className="text-xs text-text-muted">{items.length}</span>
            </div>
            <div className="space-y-2">
              {items.map((rule) => (
                <AppRuleCard key={rule.packageName} rule={rule} onToggle={updateRule} />
              ))}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function WordEditor({
  label,
  value,
  onChange,
  onSave,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  onSave: () => void;
}) {
  return (
    <div className="rounded-2xl border border-border-subtle bg-bg-primary/70 p-3">
      <label className="text-xs font-bold uppercase tracking-[0.18em] text-text-muted">
        {label}
      </label>
      <textarea
        value={value}
        onChange={(event) => onChange(event.target.value)}
        rows={3}
        className="mt-2 w-full resize-none rounded-2xl border border-border-subtle bg-bg-secondary/70 px-3 py-2 text-sm text-text-primary outline-none transition focus:border-border-hover"
      />
      <button
        onClick={onSave}
        className="mt-2 rounded-full bg-text-primary px-4 py-2 text-xs font-bold text-bg-primary transition hover:bg-accent-study active:scale-95"
      >
        Save
      </button>
    </div>
  );
}

function AppRuleCard({
  rule,
  onToggle,
}: {
  rule: AppRule;
  onToggle: (
    rule: AppRule,
    flag: (typeof FLAGS)[number]["key"],
    enabled: boolean,
  ) => Promise<void>;
}) {
  return (
    <div className="rounded-2xl border border-border-subtle bg-bg-primary/70 p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          {rule.iconDataUrl ? (
            <img
              src={rule.iconDataUrl}
              alt=""
              className="h-10 w-10 shrink-0 rounded-xl bg-bg-secondary object-contain p-1 shadow-sm"
            />
          ) : (
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-bg-secondary text-xs font-black text-accent-study">
              {rule.label.slice(0, 2).toUpperCase()}
            </div>
          )}
          <div className="min-w-0">
            <div className="truncate text-sm font-semibold text-text-primary">{rule.label}</div>
            <div className="truncate text-[11px] text-text-muted">{rule.packageName}</div>
          </div>
        </div>
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
              onClick={() => void onToggle(rule, flag.key, !enabled)}
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
  );
}

function groupRules(rules: AppRule[]): [string, AppRule[]][] {
  const hidden = new Set(["system"]);
  const grouped = rules
    .filter((rule) => !hidden.has(rule.category))
    .reduce<Record<string, AppRule[]>>((acc, rule) => {
      const key = rule.category || "other";
      acc[key] = [...(acc[key] ?? []), rule];
      return acc;
    }, {});
  const order = ["messaging", "social", "learning", "email", "school_work", "finance", "shopping", "media", "other"];
  return order
    .filter((key) => grouped[key]?.length)
    .map((key) => [key, grouped[key].sort((a, b) => a.label.localeCompare(b.label))]);
}
