import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../stores/settingsStore";

export function useStudyMode() {
  const on = useSettingsStore((s) => s.studyModeEnabled);
  const set = useSettingsStore((s) => s.setStudyMode);
  const persist = (next: boolean) => {
    set(next);
    void invoke("set_study_mode", { on: next }).catch((error) => {
      console.warn("Unable to persist Study Mode", error);
    });
  };
  return { on, toggle: () => persist(!on), set: persist };
}
