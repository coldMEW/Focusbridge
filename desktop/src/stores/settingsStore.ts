import { create } from "zustand";
import type { FilterKind, Settings } from "../types";

interface SettingsState extends Settings {
  activeFilter: FilterKind;
  setStudyMode: (on: boolean) => void;
  setTwoFaMode: (on: boolean) => void;
  setFilter: (k: FilterKind) => void;
  setAppRuleLists: (rules: { packageName: string; muted: number; priority: number; studySafe: number }[]) => void;
  replace: (s: Partial<Settings>) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  studyModeEnabled: false,
  blockedApps: [],
  priorityApps: [],
  studySafeApps: [],
  favoriteContacts: [],
  priorityKeywords: ["urgent", "asap", "emergency"],
  blockedKeywords: [],
  twoFaModeEnabled: false,
  syncMode: "LOCAL",
  lockTimeoutMinutes: 0,
  activeFilter: "ALL",
  setStudyMode: (on) => set({ studyModeEnabled: on }),
  setTwoFaMode: (on) => set({ twoFaModeEnabled: on }),
  setFilter: (k) => set({ activeFilter: k }),
  setAppRuleLists: (rules) =>
    set({
      blockedApps: rules.filter((rule) => rule.muted !== 0).map((rule) => rule.packageName),
      priorityApps: rules.filter((rule) => rule.priority !== 0).map((rule) => rule.packageName),
      studySafeApps: rules.filter((rule) => rule.studySafe !== 0).map((rule) => rule.packageName),
    }),
  replace: (s) => set((prev) => ({ ...prev, ...s })),
}));
