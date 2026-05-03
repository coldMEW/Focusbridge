export function relativeTime(tsMillis: number, now: number = Date.now()): string {
  const delta = Math.max(0, Math.floor((now - tsMillis) / 1000));
  if (delta < 60) return "just now";
  if (delta < 3600) {
    const m = Math.floor(delta / 60);
    return `${m} min ago`;
  }
  if (delta < 86400) {
    const h = Math.floor(delta / 3600);
    return `${h} hr ago`;
  }
  const d = Math.floor(delta / 86400);
  return `${d} d ago`;
}
