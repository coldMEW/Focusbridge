import { create } from "zustand";
import type { Notification, NotificationStatus } from "../types";

interface NotificationState {
  items: Notification[];
  upsert: (n: Notification) => void;
  remove: (id: string) => void;
  setStatus: (id: string, status: NotificationStatus) => void;
  clear: () => void;
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
  setStatus: (id, status) =>
    set((s) => ({
      items: s.items.map((it) => (it.id === id ? { ...it, status } : it)),
    })),
  clear: () => set({ items: [] }),
  replaceAll: (items) => set({ items }),
}));
