export type PriorityLevel = "LOW" | "NORMAL" | "HIGH" | "CRITICAL";

export function priorityLevel(score: number): PriorityLevel {
  if (score <= 20) return "LOW";
  if (score <= 50) return "NORMAL";
  if (score <= 80) return "HIGH";
  return "CRITICAL";
}

export function priorityBadge(score: number): string {
  const level = priorityLevel(score);
  switch (level) {
    case "CRITICAL":
      return "★";
    case "HIGH":
      return "·";
    default:
      return "";
  }
}
