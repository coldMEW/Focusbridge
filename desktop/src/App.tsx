import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import NotificationList from "./components/NotificationList";
import FilterPanel from "./components/FilterPanel";
import StudyModeToggle from "./components/StudyModeToggle";
import ConnectionStatus from "./components/ConnectionStatus";
import PairingQR from "./components/PairingQR";
import SettingsPanel from "./components/SettingsPanel";
import AuthGate from "./components/AuthGate";
import AppRulesPanel from "./components/AppRulesPanel";
import logo from "./assets/logo.png";
import { useConnection } from "./hooks/useConnection";
import { useConnectionStore } from "./stores/connectionStore";
import { useAppRulesStore } from "./stores/appRulesStore";
import { useNotificationStore } from "./stores/notificationStore";
import { useSettingsStore } from "./stores/settingsStore";
import type { AppRule, ConnectionState, Notification } from "./types";

interface NativeNotificationRow {
  id: string;
  app_name: string;
  package_name: string;
  sender: string;
  message: string;
  timestamp: number;
  received_at: number;
  status: Notification["status"];
  priority: number;
  content_hidden: number;
}

interface NativeSettingsSnapshot {
  study_mode_enabled?: boolean;
  two_fa_mode_enabled?: boolean;
  blocked_apps?: string[];
  priority_apps?: string[];
  study_safe_apps?: string[];
  favorite_contacts?: string[];
  priority_keywords?: string[];
  blocked_keywords?: string[];
  sync_mode?: "LOCAL" | "CLOUD";
}

function fromNative(row: NativeNotificationRow): Notification {
  return {
    id: row.id,
    appName: row.app_name,
    packageName: row.package_name,
    sender: row.sender,
    message: row.message,
    timestamp: row.timestamp,
    receivedAt: row.received_at,
    status: row.status,
    priority: row.priority,
    contentHidden: row.content_hidden !== 0,
  };
}

export default function App() {
  const [closePromptOpen, setClosePromptOpen] = useState(false);
  const { state } = useConnection();
  const activeFilter = useSettingsStore((s) => s.activeFilter);
  const setConnectionState = useConnectionStore((s) => s.setState);
  const upsert = useNotificationStore((s) => s.upsert);
  const remove = useNotificationStore((s) => s.remove);
  const replaceAll = useNotificationStore((s) => s.replaceAll);
  const notifications = useNotificationStore((s) => s.items);
  const replaceSettings = useSettingsStore((s) => s.replace);
  const replaceAppRules = useAppRulesStore((s) => s.replaceAll);
  const setAppRuleLists = useSettingsStore((s) => s.setAppRuleLists);

  useEffect(() => {
    invoke<NativeNotificationRow[]>("list_notifications", { limit: 150 })
      .then((rows) => replaceAll(rows.map(fromNative)))
      .catch((error) => console.warn("Unable to hydrate notifications", error));
    invoke<NativeSettingsSnapshot>("get_settings")
      .then((settings) =>
        replaceSettings({
          studyModeEnabled: Boolean(settings.study_mode_enabled),
          twoFaModeEnabled: Boolean(settings.two_fa_mode_enabled),
          blockedApps: settings.blocked_apps ?? [],
          priorityApps: settings.priority_apps ?? [],
          studySafeApps: settings.study_safe_apps ?? [],
          favoriteContacts: settings.favorite_contacts ?? [],
          priorityKeywords: settings.priority_keywords?.length
            ? settings.priority_keywords
            : ["urgent", "asap", "emergency"],
          blockedKeywords: settings.blocked_keywords ?? [],
          syncMode: settings.sync_mode ?? "LOCAL",
        }),
      )
      .catch((error) => console.warn("Unable to hydrate settings", error));
    invoke<AppRule[]>("list_app_rules")
      .then((rules) => {
        replaceAppRules(rules);
        setAppRuleLists(rules);
      })
      .catch((error) => console.warn("Unable to hydrate app rules", error));
  }, [replaceAll, replaceAppRules, replaceSettings, setAppRuleLists]);

  useEffect(() => {
    const unlisten = Promise.all([
      listen<ConnectionState>("focusbridge://connection", (event) => {
        setConnectionState(event.payload);
      }),
      listen<NativeNotificationRow>("focusbridge://notification", (event) => {
        upsert(fromNative(event.payload));
      }),
      listen<AppRule[]>("focusbridge://app-rules", (event) => {
        replaceAppRules(event.payload);
        setAppRuleLists(event.payload);
      }),
      listen<string>("focusbridge://dismissal", (event) => {
        remove(event.payload);
      }),
      listen("focusbridge://close-requested", () => {
        setClosePromptOpen(true);
      }),
    ]);

    return () => {
      unlisten.then((listeners) => {
        listeners.forEach((dispose) => dispose());
      });
    };
  }, [remove, replaceAppRules, setAppRuleLists, setConnectionState, upsert]);

  const showPairing = state === "DISCONNECTED";
  const newCount = notifications.filter((n) => n.status === "NEW").length;
  const importantCount = notifications.filter((n) => n.status === "IMPORTANT").length;
  const securityCount = notifications.filter((n) => n.priority >= 100).length;
  const title = {
    ALL: "Attention inbox",
    IMPORTANT: "Priority lane",
    STUDY: "Study-safe feed",
    TWOFA: "Security codes",
    APP_CONTROL: "Phone app control",
  }[activeFilter];
  const showingAppControl = activeFilter === "APP_CONTROL";

  return (
    <AuthGate>
      <div className="app-shell h-screen w-screen overflow-hidden bg-bg-primary text-text-primary">
      <div className="ambient-orb ambient-orb-one" />
      <div className="ambient-orb ambient-orb-two" />
      <div className="relative z-10 flex h-full p-4">
        <aside className="glass-panel flex w-[248px] shrink-0 flex-col overflow-hidden rounded-[28px] p-4">
          <div className="mb-7">
            <img
              src={logo}
              alt="FocusBridge logo"
              className="mb-4 h-14 w-14 rounded-2xl shadow-soft"
            />
            <div className="text-[11px] font-semibold uppercase tracking-[0.28em] text-accent-study">
              FocusBridge
            </div>
            <h1 className="mt-3 text-3xl font-semibold tracking-[-0.04em]">
              Quiet command.
            </h1>
            <p className="mt-2 text-sm leading-5 text-text-secondary">
              One focused bridge between your phone and desktop.
            </p>
          </div>
          <FilterPanel />
          <div className="mt-auto rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
            <div className="text-xs uppercase tracking-[0.22em] text-text-muted">
              Device status
            </div>
            <div className="mt-3">
              <ConnectionStatus />
            </div>
            <p className="mt-3 text-xs leading-5 text-text-secondary">
              Green means your phone is actively linked. Red means pair or restart sync.
            </p>
          </div>
        </aside>

        <main className="ml-4 grid min-w-0 flex-1 grid-cols-[minmax(0,1fr)_340px] gap-4">
          <section className="glass-panel flex min-w-0 flex-col overflow-hidden rounded-[32px]">
            <header className="border-b border-border-subtle px-6 py-5">
              <div className="flex flex-wrap items-center justify-between gap-4">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.24em] text-text-muted">
                    Desktop triage
                  </p>
                  <h2 className="mt-1 text-3xl font-semibold tracking-[-0.035em]">
                    {title}
                  </h2>
                </div>
                <div className="flex items-center gap-3">
                  <StudyModeToggle />
                  <ConnectionStatus />
                </div>
              </div>
              <div className="mt-5 grid grid-cols-3 gap-3">
                <MetricCard label="New" value={newCount} tone="fresh" />
                <MetricCard label="Pinned" value={importantCount} tone="warm" />
                <MetricCard label="2FA" value={securityCount} tone="cool" />
              </div>
            </header>
            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
              {showingAppControl ? <AppRulesPanel fullPage /> : <NotificationList />}
            </div>
          </section>

          <aside className="flex min-h-0 flex-col gap-4 overflow-y-auto">
            {showPairing ? <PairingQR /> : <SettingsPanel />}
            {!showPairing && <PairingQR compact />}
          </aside>
        </main>
      </div>
      {closePromptOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#17221e]/45 p-6 backdrop-blur-sm">
          <section className="w-full max-w-md rounded-[30px] border border-border-subtle bg-bg-primary p-6 shadow-2xl">
            <p className="text-xs font-semibold uppercase tracking-[0.24em] text-accent-study">
              Keep FocusBridge running?
            </p>
            <h2 className="mt-3 text-2xl font-semibold tracking-[-0.035em]">
              Closing stops phone notification sync.
            </h2>
            <p className="mt-3 text-sm leading-6 text-text-secondary">
              Run in tray to keep receiving phone notifications in the background, or quit fully if
              you want to stop FocusBridge for now.
            </p>
            <div className="mt-6 grid gap-3 sm:grid-cols-2">
              <button
                onClick={() => {
                  setClosePromptOpen(false);
                  void invoke("minimize_to_tray");
                }}
                className="rounded-full bg-text-primary px-4 py-3 text-sm font-semibold text-bg-primary transition hover:bg-accent-study"
              >
                Run in tray
              </button>
              <button
                onClick={() => {
                  setClosePromptOpen(false);
                  void invoke("quit_app");
                }}
                className="rounded-full border border-border-subtle px-4 py-3 text-sm font-semibold text-text-secondary transition hover:border-border-hover hover:text-text-primary"
              >
                Quit FocusBridge
              </button>
            </div>
            <button
              onClick={() => setClosePromptOpen(false)}
              className="mt-4 w-full text-center text-xs font-semibold uppercase tracking-[0.18em] text-text-muted transition hover:text-text-primary"
            >
              Cancel
            </button>
          </section>
        </div>
      )}
      </div>
    </AuthGate>
  );
}

function MetricCard({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone: "fresh" | "warm" | "cool";
}) {
  return (
    <div className={`metric-card metric-${tone}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
