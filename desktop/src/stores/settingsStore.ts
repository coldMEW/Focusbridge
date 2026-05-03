import { create } from "zustand";
import type { FilterKind, Settings } from "../types";

interface SettingsState extends Settings {
  activeFilter: FilterKind;
  setStudyMode: (on: boolean) => void;
  setTwoFaMode: (on: boolean) => void;
  setFilter: (k: FilterKind) => void;
  replace: (s: Partial<Settings>) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  studyModeEnabled: false,
  blockedApps: [],
  priorityApps: [],
  favoriteContacts: [],
  priorityKeywords: ["urgent", "asap", "emergency"],
  twoFaModeEnabled: false,
  syncMode: "LOCAL",
  activeFilter: "ALL",
  setStudyMode: (on) => set({ studyModeEnabled: on }),
  setTwoFaMode: (on) => set({ twoFaModeEnabled: on }),
  setFilter: (k) => set({ activeFilter: k }),
  replace: (s) => set((prev) => ({ ...prev, ...s })),
}));
