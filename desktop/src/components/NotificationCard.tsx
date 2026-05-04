import { useState } from "react";
import type { Notification } from "../types";
import { relativeTime } from "../utils/time";
import { priorityBadge, priorityLevel } from "../utils/priority";

interface Props {
  notification: Notification;
  index?: number;
  onIgnore: (id: string) => void;
  onImportant: (id: string) => void;
  onDelete: (id: string) => void;
}

export default function NotificationCard({
  notification,
  index = 0,
  onIgnore,
  onImportant,
  onDelete,
}: Props) {
  const [peekVisible, setPeekVisible] = useState(false);
  const is2fa = notification.priority >= 100;
  const isImportant = notification.status === "IMPORTANT";
  const level = priorityLevel(notification.priority);
  const initials = notification.appName.slice(0, 2).toUpperCase();
  const shouldMask = notification.contentHidden && !peekVisible;
  const body = notification.message || "New notification";

  return (
    <article
      data-testid="notification-card"
      style={{ animationDelay: `${Math.min(index, 8) * 45}ms` }}
      className={
        "notification-card animate-rise-in rounded-[24px] border px-4 py-4 transition-all duration-200 " +
        (isImportant
          ? "border-accent-important/70 bg-[#fff7e8]"
          : "border-border-subtle bg-bg-secondary/85") +
        (notification.status === "IGNORED" ? " opacity-40" : "")
      }
    >
      <div className="flex items-start gap-3">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-bg-surface text-sm font-semibold text-accent-study">
          {initials || "FB"}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-baseline gap-2 text-xs text-text-secondary">
            <span className="font-semibold text-text-primary">{notification.appName}</span>
            <span>{notification.sender || "Unknown sender"}</span>
          </div>
          <p className="mt-1 line-clamp-2 text-[15px] leading-6 text-text-primary">
            {shouldMask ? (
              <button
                type="button"
                onClick={() => setPeekVisible(true)}
                onFocus={() => setPeekVisible(true)}
                onMouseEnter={() => setPeekVisible(true)}
                className="rounded-full border border-border-subtle bg-bg-primary/60 px-3 py-1 text-left text-sm italic text-text-secondary transition hover:border-border-hover hover:text-text-primary"
                aria-label="Masked message. Hover, focus, or click to peek."
              >
                Masked message - hover or tap to peek
              </button>
            ) : (
              <span
                onMouseLeave={() => setPeekVisible(false)}
                className={notification.contentHidden ? "rounded-lg bg-[#fff0c7] px-1.5 py-0.5" : ""}
              >
                {body}
              </span>
            )}
          </p>
        </div>
        <span className="rounded-full border border-border-subtle bg-bg-primary/40 px-2.5 py-1 text-[11px] uppercase tracking-wide text-text-secondary">
          {priorityBadge(notification.priority)} {level}
        </span>
      </div>

      <div className="mt-4 flex items-center justify-between gap-3 text-xs text-text-muted">
        <div className="flex flex-wrap items-center gap-2">
          <span>{relativeTime(notification.timestamp)}</span>
          {is2fa && (
            <span className="rounded-full bg-[#e7f3ff] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-[#275b7a]">
              security
            </span>
          )}
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => onDelete(notification.id)}
            className="rounded-full border border-[#f0b8aa] px-3 py-1.5 font-medium text-[#8f3324] hover:bg-[#fff0eb]"
          >
            Delete
          </button>
          <button
            onClick={() => onIgnore(notification.id)}
            className="rounded-full border border-border-subtle px-3 py-1.5 font-medium text-text-secondary hover:border-border-hover hover:text-text-primary"
          >
            Ignore
          </button>
          <button
            onClick={() => onImportant(notification.id)}
            className="rounded-full bg-text-primary px-3 py-1.5 font-medium text-bg-primary hover:bg-accent-study hover:text-white"
          >
            Pin
          </button>
        </div>
      </div>
    </article>
  );
}
