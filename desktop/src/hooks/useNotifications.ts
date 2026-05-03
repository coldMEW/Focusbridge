import { useMemo } from "react";
import { useNotificationStore } from "../stores/notificationStore";
import { useSettingsStore } from "../stores/settingsStore";
import { priorityLevel } from "../utils/priority";
import type { Notification } from "../types";

export function useFilteredNotifications(): Notification[] {
  const items = useNotificationStore((s) => s.items);
  const filter = useSettingsStore((s) => s.activeFilter);
  const blockedApps = useSettingsStore((s) => s.blockedApps);
  const priorityApps = useSettingsStore((s) => s.priorityApps);
  const studySafeApps = useSettingsStore((s) => s.studySafeApps);

  return useMemo(() => {
    const visible = items.filter((n) => !blockedApps.includes(n.packageName));
    switch (filter) {
      case "IMPORTANT":
        return visible.filter((n) => n.status === "IMPORTANT" || priorityApps.includes(n.packageName));
      case "STUDY":
        return visible.filter(
          (n) =>
            studySafeApps.includes(n.packageName) ||
            priorityApps.includes(n.packageName) ||
            priorityLevel(n.priority) === "HIGH" ||
            priorityLevel(n.priority) === "CRITICAL",
        );
      case "TWOFA":
        return visible.filter((n) => n.priority >= 100);
      case "ALL":
      default:
        return visible;
    }
  }, [items, filter, blockedApps, priorityApps, studySafeApps]);
}
