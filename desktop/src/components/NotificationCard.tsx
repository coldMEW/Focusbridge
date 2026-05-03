import type { Notification } from "../types";
import { relativeTime } from "../utils/time";
import { priorityBadge, priorityLevel } from "../utils/priority";

interface Props {
  notification: Notification;
  onIgnore: (id: string) => void;
  onImportant: (id: string) => void;
}

export default function NotificationCard({ notification, onIgnore, onImportant }: Props) {
  const is2fa = notification.priority >= 100;
  const isImportant = notification.status === "IMPORTANT";
  const level = priorityLevel(notification.priority);

  return (
    <article
      data-testid="notification-card"
      className={
        "mb-2 rounded-sm border px-3 py-2 transition-opacity duration-200 " +
        (isImportant
          ? "border-accent-important bg-bg-surface"
          : "border-border-subtle bg-bg-secondary") +
        (notification.status === "IGNORED" ? " opacity-40" : "")
      }
    >
      <div className="flex items-baseline gap-2 text-xs text-text-secondary">
        <span>[{notification.appName}]</span>
        <span className="text-text-primary">{notification.sender}</span>
        {is2fa && (
          <span className="rounded border border-border-subtle px-1 text-[10px] uppercase tracking-wide">
            2FA
          </span>
        )}
        <span className="ml-auto text-text-muted">
          {priorityBadge(notification.priority)} {level.toLowerCase()}
        </span>
      </div>
      <p className="mt-1 line-clamp-2 text-sm">
        {notification.contentHidden ? (
          <em className="text-text-secondary">New message</em>
        ) : (
          notification.message
        )}
      </p>
      <div className="mt-2 flex items-center justify-between text-xs text-text-muted">
        <span>{relativeTime(notification.timestamp)}</span>
        <div className="flex gap-2">
          <button
            onClick={() => onIgnore(notification.id)}
            className="rounded border border-border-subtle px-2 py-0.5 hover:border-border-hover"
          >
            Ignore
          </button>
          <button
            onClick={() => onImportant(notification.id)}
            className="rounded border border-border-subtle px-2 py-0.5 hover:border-border-hover"
          >
            Important
          </button>
        </div>
      </div>
    </article>
  );
}
