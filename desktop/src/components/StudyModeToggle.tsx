import { useStudyMode } from "../hooks/useStudyMode";

export default function StudyModeToggle() {
  const { on, toggle } = useStudyMode();
  return (
    <button
      onClick={toggle}
      data-testid="study-toggle"
      aria-pressed={on}
      className={
        "rounded px-2 py-1 text-xs " +
        (on
          ? "border border-accent-study text-accent-study"
          : "border border-border-subtle text-text-secondary")
      }
    >
      Study {on ? "ON" : "OFF"}
    </button>
  );
}
