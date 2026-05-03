import { create } from "zustand";
import type { AppRule } from "../types";

interface AppRulesState {
  items: AppRule[];
  replaceAll: (items: AppRule[]) => void;
  upsert: (item: AppRule) => void;
}

export const useAppRulesStore = create<AppRulesState>((set) => ({
  items: [],
  replaceAll: (items) => set({ items }),
  upsert: (item) =>
    set((state) => {
      const index = state.items.findIndex((existing) => existing.packageName === item.packageName);
      if (index === -1) {
        return { items: [...state.items, item].sort(sortRules) };
      }
      const next = state.items.slice();
      next[index] = item;
      return { items: next.sort(sortRules) };
    }),
}));

function sortRules(a: AppRule, b: AppRule): number {
  return (
    Number(a.muted !== 0) - Number(b.muted !== 0) ||
    Number(b.priority !== 0) - Number(a.priority !== 0) ||
    b.notificationsSeen - a.notificationsSeen ||
    a.label.localeCompare(b.label)
  );
}
