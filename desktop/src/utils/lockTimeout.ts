export type LockTimeoutUnit = "minute" | "hour" | "day" | "month";

export const MAX_LOCK_TIMEOUT_MINUTES = 12 * 30 * 24 * 60;

const unitMinutes: Record<LockTimeoutUnit, number> = {
  minute: 1,
  hour: 60,
  day: 24 * 60,
  month: 30 * 24 * 60,
};

export function lockTimeoutMinutesFrom(value: number, unit: LockTimeoutUnit): number {
  if (!Number.isFinite(value) || value <= 0) return 0;
  return Math.min(Math.round(value * unitMinutes[unit]), MAX_LOCK_TIMEOUT_MINUTES);
}

export function lockTimeoutLabel(minutes: number): string {
  if (minutes <= 0) return "Unlimited";
  const units: Array<[LockTimeoutUnit, number]> = [
    ["month", unitMinutes.month],
    ["day", unitMinutes.day],
    ["hour", unitMinutes.hour],
    ["minute", unitMinutes.minute],
  ];
  const [unit, size] = units.find(([, size]) => minutes >= size && minutes % size === 0) ?? [
    "minute",
    1,
  ];
  const value = minutes / size;
  return `${value} ${unit}${value === 1 ? "" : "s"}`;
}
