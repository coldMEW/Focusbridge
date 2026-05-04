import { create } from "zustand";
import type { Notification, NotificationStatus } from "../types";

interface NotificationState {
  items: Notification[];
  upsert: (n: Notification) => void;
  remove: (id: string) => void;
  removeBetween: (startMs: number, endMs: number) => void;
  setStatus: (id: string, status: NotificationStatus) => void;
  clear: () => void;
  clearOlderThan: (cutoffMs: number) => void;
  replaceAll: (items: Notification[]) => void;
}

export const useNotificationStore = create<NotificationState>((set) => ({
  items: [],
  upsert: (n) =>
    set((s) => {
      const idx = s.items.findIndex((it) => it.id === n.id);
      if (idx === -1) return { items: [n, ...s.items] };
      const next = s.items.slice();
      next[idx] = n;
      return { items: next };
    }),
  remove: (id) =>
    set((s) => ({ items: s.items.filter((it) => it.id !== id) })),
  removeBetween: (startMs, endMs) =>
    set((s) => ({
      items: s.items.filter((it) => {
        const at = Math.min(it.timestamp, it.receivedAt);
        return at < startMs || at >= endMs;
      }),
    })),
  setStatus: (id, status) =>
    set((s) => ({
      items: s.items.map((it) => (it.id === id ? { ...it, status } : it)),
    })),
  clear: () => set({ items: [] }),
  clearOlderThan: (cutoffMs) =>
    set((s) => ({ items: s.items.filter((it) => it.receivedAt >= cutoffMs) })),
  replaceAll: (items) => set({ items }),
}));
