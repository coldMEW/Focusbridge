export const ACCOUNT_SESSION_DAYS = 90;

export type AccountSession =
  | {
      mode: "firebase";
      email: string;
      uid: string;
      lastLoginAt: number;
    }
  | {
      mode: "guest";
      lastLoginAt: number;
    };

const sessionMs = ACCOUNT_SESSION_DAYS * 24 * 60 * 60 * 1000;

export function accountSessionValid(
  value: unknown,
  now = Date.now(),
): value is AccountSession {
  if (!value || typeof value !== "object") return false;
  const session = value as Partial<AccountSession>;
  if (session.mode !== "firebase" && session.mode !== "guest") return false;
  if (typeof session.lastLoginAt !== "number" || !Number.isFinite(session.lastLoginAt)) {
    return false;
  }
  if (now - session.lastLoginAt >= sessionMs) return false;
  if (session.mode === "firebase") {
    return Boolean(session.email && session.uid);
  }
  return true;
}

export function accountSessionExpired(value: unknown, now = Date.now()): boolean {
  if (!value || typeof value !== "object") return false;
  const session = value as Partial<AccountSession>;
  if (typeof session.lastLoginAt !== "number" || !Number.isFinite(session.lastLoginAt)) {
    return false;
  }
  return now - session.lastLoginAt >= sessionMs;
}

export function accountSessionLabel(session: AccountSession): string {
  return session.mode === "firebase" ? session.email : "Guest mode";
}

export function readAccountSession(storage: Storage): AccountSession | null {
  const raw = storage.getItem("focusbridge.account.session");
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    return accountSessionValid(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function writeAccountSession(storage: Storage, session: AccountSession): void {
  storage.setItem("focusbridge.account.session", JSON.stringify(session));
}

export function clearAccountSession(storage: Storage): void {
  storage.removeItem("focusbridge.account.session");
}
