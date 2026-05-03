import { useMemo } from "react";
import { useNotificationStore } from "../stores/notificationStore";
import { useSettingsStore } from "../stores/settingsStore";
import { priorityLevel } from "../utils/priority";
import type { Notification } from "../types";

export function useFilteredNotifications(): Notification[] {
  const items = useNotificationStore((s) => s.items);
  const filter = useSettingsStore((s) => s.activeFilter);
  const blockedApps = useSettingsStore((s) => s.blockedApps);

  return useMemo(() => {
    const visible = items.filter((n) => !blockedApps.includes(n.packageName));
    switch (filter) {
      case "IMPORTANT":
        return visible.filter((n) => n.status === "IMPORTANT");
      case "STUDY":
        return visible.filter(
          (n) => priorityLevel(n.priority) === "HIGH" || priorityLevel(n.priority) === "CRITICAL",
        );
      case "TWOFA":
        return visible.filter((n) => n.priority >= 100);
      case "ALL":
      default:
        return visible;
    }
  }, [items, filter, blockedApps]);
}
