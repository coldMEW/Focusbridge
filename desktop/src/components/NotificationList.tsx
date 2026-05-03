import { invoke } from "@tauri-apps/api/core";
import NotificationCard from "./NotificationCard";
import EmptyState from "./EmptyState";
import { useFilteredNotifications } from "../hooks/useNotifications";
import { useNotificationStore } from "../stores/notificationStore";
import type { NotificationStatus } from "../types";

export default function NotificationList() {
  const items = useFilteredNotifications();
  const setStatus = useNotificationStore((s) => s.setStatus);

  const mark = async (id: string, status: NotificationStatus) => {
    setStatus(id, status);
    try {
      await invoke(status === "IMPORTANT" ? "mark_important" : "mark_ignored", { id });
    } catch (error) {
      console.warn("Unable to persist notification action", error);
    }
  };

  if (items.length === 0) return <EmptyState />;

  return (
    <div className="space-y-3">
      {items.map((n, index) => (
        <NotificationCard
          key={n.id}
          notification={n}
          index={index}
          onIgnore={(id) => mark(id, "IGNORED")}
          onImportant={(id) => mark(id, "IMPORTANT")}
        />
      ))}
    </div>
  );
}
