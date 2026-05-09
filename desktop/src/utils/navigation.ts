import type { FilterKind } from "../types";

export interface NavigationItem {
  key: FilterKind;
  label: string;
  hint: string;
}

export const MAIN_NAV_ITEMS: NavigationItem[] = [
  { key: "ALL", label: "Inbox", hint: "Everything not dismissed" },
  { key: "IMPORTANT", label: "Priority", hint: "Pinned and urgent" },
  { key: "STUDY", label: "Study lane", hint: "Low-interruption mode" },
  { key: "TWOFA", label: "Security", hint: "Codes and sign-ins" },
  { key: "APP_CONTROL", label: "App Control", hint: "Words, apps, and toggles" },
];

export const SETTINGS_NAV_ITEM: NavigationItem = {
  key: "SETTINGS",
  label: "Settings",
  hint: "Lock, cleanup, diagnostics",
};
