import { useStudyMode } from "../hooks/useStudyMode";

export default function StudyModeToggle() {
  const { on, toggle } = useStudyMode();
  return (
    <button
      onClick={toggle}
      data-testid="study-toggle"
      aria-pressed={on}
      className={
        "rounded-full px-4 py-2 text-xs font-semibold transition-all " +
        (on
          ? "bg-accent-study text-white shadow-soft"
          : "border border-border-subtle bg-bg-secondary text-text-secondary hover:text-text-primary")
      }
    >
      {on ? "Study mode on" : "Study mode"}
    </button>
  );
}
