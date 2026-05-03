import NotificationCard from "./NotificationCard";
import EmptyState from "./EmptyState";
import { useFilteredNotifications } from "../hooks/useNotifications";
import { useNotificationStore } from "../stores/notificationStore";

export default function NotificationList() {
  const items = useFilteredNotifications();
  const setStatus = useNotificationStore((s) => s.setStatus);

  if (items.length === 0) return <EmptyState />;

  return (
    <div>
      {items.map((n) => (
        <NotificationCard
          key={n.id}
          notification={n}
          onIgnore={(id) => setStatus(id, "IGNORED")}
          onImportant={(id) => setStatus(id, "IMPORTANT")}
        />
      ))}
    </div>
  );
}
