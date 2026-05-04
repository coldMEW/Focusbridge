import { invoke } from "@tauri-apps/api/core";
import NotificationCard from "./NotificationCard";
import EmptyState from "./EmptyState";
import { useFilteredNotifications } from "../hooks/useNotifications";
import { useNotificationStore } from "../stores/notificationStore";
import type { Notification, NotificationStatus } from "../types";

const HOUR_MS = 60 * 60 * 1000;
const DAY_MS = 24 * HOUR_MS;

interface NotificationSection {
  key: string;
  label: string;
  startMs: number;
  endMs: number;
  items: Notification[];
}

export default function NotificationList() {
  const items = useFilteredNotifications();
  const setStatus = useNotificationStore((s) => s.setStatus);
  const remove = useNotificationStore((s) => s.remove);
  const removeBetween = useNotificationStore((s) => s.removeBetween);

  const mark = async (id: string, status: NotificationStatus) => {
    setStatus(id, status);
    try {
      await invoke(status === "IMPORTANT" ? "mark_important" : "mark_ignored", { id });
    } catch (error) {
      console.warn("Unable to persist notification action", error);
    }
  };

  const deleteOne = async (id: string) => {
    remove(id);
    try {
      await invoke<number>("delete_notification", { id });
    } catch (error) {
      console.warn("Unable to delete notification", error);
    }
  };

  const deleteSection = async (section: NotificationSection) => {
    removeBetween(section.startMs, section.endMs);
    try {
      await invoke<number>("clear_notifications_between", {
        startMs: section.startMs,
        endMs: section.endMs,
      });
    } catch (error) {
      console.warn("Unable to delete notification section", error);
    }
  };

  if (items.length === 0) return <EmptyState />;
  const sections = groupByAge(items);

  return (
    <div className="space-y-5">
      {sections.map((section) => (
        <section key={section.key} className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h3 className="text-sm font-black text-text-primary">{section.label}</h3>
              <p className="text-xs text-text-muted">{section.items.length} notification{section.items.length === 1 ? "" : "s"}</p>
            </div>
            <button
              onClick={() => void deleteSection(section)}
              className="rounded-full border border-[#f0b8aa] bg-[#fff7f3] px-3 py-1.5 text-xs font-bold text-[#8f3324] transition hover:-translate-y-0.5 hover:bg-[#ffe4dc] active:translate-y-0"
            >
              Delete section
            </button>
          </div>
          {section.items.map((n, index) => (
            <NotificationCard
              key={n.id}
              notification={n}
              index={index}
              onIgnore={(id) => mark(id, "IGNORED")}
              onImportant={(id) => mark(id, "IMPORTANT")}
              onDelete={deleteOne}
            />
          ))}
        </section>
      ))}
    </div>
  );
}

function groupByAge(items: Notification[]): NotificationSection[] {
  const now = Date.now();
  const buckets = [
    { key: "just_now", label: "Just now", startMs: now - 5 * 60 * 1000, endMs: Number.MAX_SAFE_INTEGER },
    { key: "last_hour", label: "Last hour", startMs: now - HOUR_MS, endMs: now - 5 * 60 * 1000 },
    { key: "today", label: "Today", startMs: now - DAY_MS, endMs: now - HOUR_MS },
    { key: "week", label: "This week", startMs: now - 7 * DAY_MS, endMs: now - DAY_MS },
    { key: "month", label: "This month", startMs: now - 30 * DAY_MS, endMs: now - 7 * DAY_MS },
    { key: "older", label: "Older", startMs: 0, endMs: now - 30 * DAY_MS },
  ];

  return buckets
    .map((bucket) => ({
      ...bucket,
      items: items.filter((item) => {
        const at = Math.min(item.timestamp, item.receivedAt);
        return at >= bucket.startMs && at < bucket.endMs;
      }),
    }))
    .filter((section) => section.items.length > 0);
}
