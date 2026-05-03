import { useSettingsStore } from "../stores/settingsStore";

export function useStudyMode() {
  const on = useSettingsStore((s) => s.studyModeEnabled);
  const set = useSettingsStore((s) => s.setStudyMode);
  return { on, toggle: () => set(!on), set };
}
