import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import NotificationList from "./components/NotificationList";
import FilterPanel from "./components/FilterPanel";
import StudyModeToggle from "./components/StudyModeToggle";
import ConnectionStatus from "./components/ConnectionStatus";
import PairingQR from "./components/PairingQR";
import { useConnection } from "./hooks/useConnection";
import { useConnectionStore } from "./stores/connectionStore";
import { useNotificationStore } from "./stores/notificationStore";
import type { ConnectionState, Notification } from "./types";

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
  const { state } = useConnection();
  const setConnectionState = useConnectionStore((s) => s.setState);
  const upsert = useNotificationStore((s) => s.upsert);
  const remove = useNotificationStore((s) => s.remove);

  useEffect(() => {
    const unlisten = Promise.all([
      listen<ConnectionState>("focusbridge://connection", (event) => {
        setConnectionState(event.payload);
      }),
      listen<NativeNotificationRow>("focusbridge://notification", (event) => {
        upsert(fromNative(event.payload));
      }),
      listen<string>("focusbridge://dismissal", (event) => {
        remove(event.payload);
      }),
    ]);

    return () => {
      unlisten.then((listeners) => {
        listeners.forEach((dispose) => dispose());
      });
    };
  }, [remove, setConnectionState, upsert]);

  const showPairing = state === "DISCONNECTED";

  return (
    <div className="flex h-screen w-screen flex-col bg-bg-primary text-text-primary">
      <header className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
        <h1 className="text-sm font-medium tracking-wide text-text-secondary">
          FocusBridge
        </h1>
        <div className="flex items-center gap-3">
          <StudyModeToggle />
          <ConnectionStatus />
        </div>
      </header>
      <div className="flex flex-1 overflow-hidden">
        <aside className="w-[180px] border-r border-border-subtle">
          <FilterPanel />
        </aside>
        <main className="flex-1 overflow-y-auto p-4">
          {showPairing ? <PairingQR /> : <NotificationList />}
        </main>
      </div>
    </div>
  );
}
