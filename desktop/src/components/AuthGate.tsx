import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import logo from "../assets/logo.png";
import {
  clearAccountSession,
  readAccountSession,
  writeAccountSession,
  type AccountSession,
} from "../lib/accountSession";
import {
  firebaseCurrentUser,
  firebaseEmailSignIn,
  firebaseEmailSignUp,
  firebaseSendPasswordReset,
} from "../lib/firebaseAuth";

interface AuthStatus {
  configured: boolean;
  relayEmail?: string | null;
  lockTimeoutMinutes: number;
  recoveryConfigured: boolean;
  recoveryQuestion?: string | null;
}

interface GoogleSignInResult {
  email: string;
  userId: string;
}

type AccountMode = "login" | "signup" | "guest";
type LockMode = "unlock" | "setup" | "recover";

const SECURITY_QUESTIONS = [
  "What city were you born in?",
  "What was the name of your first school?",
  "What was your childhood nickname?",
  "What is the name of your favorite teacher?",
  "What was the model of your first phone?",
  "Custom question",
];

export default function AuthGate({ children }: { children: React.ReactNode }) {
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [relayEmail, setRelayEmail] = useState<string | null>(null);
  const [relayUrl, setRelayUrl] = useState("http://127.0.0.1:8443");
  const [accountSession, setAccountSession] = useState<AccountSession | null>(null);
  const [accountMode, setAccountMode] = useState<AccountMode>("login");
  const [lockMode, setLockMode] = useState<LockMode>("unlock");
  const [unlocked, setUnlocked] = useState(false);
  const [password, setPassword] = useState("");
  const [securityQuestion, setSecurityQuestion] = useState("");
  const [securityAnswer, setSecurityAnswer] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [recoveryQuestion, setRecoveryQuestion] = useState<string | null>(null);
  const [email, setEmail] = useState("");
  const [firebasePassword, setFirebasePassword] = useState("");
  const [firebaseBusy, setFirebaseBusy] = useState(false);
  const [relayPassword, setRelayPassword] = useState("");
  const [relayOtp, setRelayOtp] = useState("");
  const [relayOtpSent, setRelayOtpSent] = useState(false);
  const [relayBusy, setRelayBusy] = useState(false);
  const [googleBusy, setGoogleBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [lockTimeoutMinutes, setLockTimeoutMinutes] = useState(0);
  const [lastActivity, setLastActivity] = useState(Date.now());

  useEffect(() => {
    const session = readAccountSession(window.localStorage);
    if (session) {
      setAccountSession(session);
      if (session.mode === "firebase") {
        setRelayEmail(session.email);
      }
    } else {
      clearAccountSession(window.localStorage);
    }

    invoke<AuthStatus>("auth_status")
      .then((status) => {
        setConfigured(status.configured);
        setRelayEmail((current) => current ?? status.relayEmail ?? null);
        setLockTimeoutMinutes(status.lockTimeoutMinutes ?? 0);
        setRecoveryQuestion(status.recoveryQuestion ?? null);
        setLockMode(status.configured ? "unlock" : "setup");
        window.localStorage.setItem("focusbridge.lockTimeoutMinutes", String(status.lockTimeoutMinutes ?? 0));
      })
      .catch((err) => setError(String(err)));

    firebaseCurrentUser()
      .then((user) => {
        if (user?.email && !session) {
          setEmail(user.email);
        }
      })
      .catch(() => {
        // Firebase startup should not block guest/local use.
      });
  }, []);

  useEffect(() => {
    const syncTimeout = () => {
      const value = Number.parseInt(
        window.localStorage.getItem("focusbridge.lockTimeoutMinutes") ?? "0",
        10,
      );
      setLockTimeoutMinutes(Number.isFinite(value) ? value : 0);
    };
    window.addEventListener("focusbridge://lock-timeout-updated", syncTimeout);
    window.addEventListener("storage", syncTimeout);
    return () => {
      window.removeEventListener("focusbridge://lock-timeout-updated", syncTimeout);
      window.removeEventListener("storage", syncTimeout);
    };
  }, []);

  useEffect(() => {
    if (!unlocked || lockTimeoutMinutes <= 0) return;
    const markActivity = () => setLastActivity(Date.now());
    const events = ["mousemove", "mousedown", "keydown", "touchstart", "focus"];
    events.forEach((event) => window.addEventListener(event, markActivity, { passive: true }));
    const timer = window.setInterval(() => {
      if (Date.now() - lastActivity >= lockTimeoutMinutes * 60_000) {
        setUnlocked(false);
        setPassword("");
      }
    }, 5_000);
    return () => {
      events.forEach((event) => window.removeEventListener(event, markActivity));
      window.clearInterval(timer);
    };
  }, [lastActivity, lockTimeoutMinutes, unlocked]);

  const acceptAccountSession = (session: AccountSession) => {
    writeAccountSession(window.localStorage, session);
    setAccountSession(session);
    setNotice(session.mode === "guest" ? "Guest mode enabled. Settings stay on this desktop." : `Signed in as ${session.email}.`);
    setError(null);
  };

  const submitFirebaseAccount = async () => {
    setError(null);
    setNotice(null);
    setFirebaseBusy(true);
    try {
      const result =
        accountMode === "signup"
          ? await firebaseEmailSignUp(email, firebasePassword)
          : await firebaseEmailSignIn(email, firebasePassword);
      const session: AccountSession = {
        mode: "firebase",
        email: result.email,
        uid: result.uid,
        lastLoginAt: Date.now(),
      };
      setRelayEmail(result.email);
      setFirebasePassword("");
      acceptAccountSession(session);
    } catch (err) {
      setError(firebaseErrorMessage(err));
    } finally {
      setFirebaseBusy(false);
    }
  };

  const continueAsGuest = () => {
    acceptAccountSession({ mode: "guest", lastLoginAt: Date.now() });
  };

  const sendPasswordReset = async () => {
    setError(null);
    setNotice(null);
    if (!email.trim()) {
      setError("Enter your email first, then request a password reset.");
      return;
    }
    setFirebaseBusy(true);
    try {
      await firebaseSendPasswordReset(email);
      setNotice("Password reset email sent. Check your inbox, then sign in again.");
    } catch (err) {
      setError(firebaseErrorMessage(err));
    } finally {
      setFirebaseBusy(false);
    }
  };

  const submitLocalLock = async () => {
    setError(null);
    setNotice(null);
    try {
      if (lockMode === "recover") {
        await invoke("auth_reset_password_with_recovery", {
          securityAnswer,
          newPassword,
        });
        setConfigured(true);
        setPassword("");
        setNewPassword("");
        setSecurityAnswer("");
        setLockMode("unlock");
        setNotice("Local PIN/password reset. Unlock with the new secret.");
        return;
      }

      if (configured) {
        await invoke("auth_login", { password });
      } else {
        await invoke("auth_register_with_recovery", {
          password,
          securityQuestion,
          securityAnswer,
        });
        setConfigured(true);
        setRecoveryQuestion(securityQuestion);
      }
      setUnlocked(true);
      setLastActivity(Date.now());
      setPassword("");
      setSecurityQuestion("");
      setSecurityAnswer("");
    } catch (err) {
      setError(String(err));
    }
  };

  const signInWithGoogle = async () => {
    setError(null);
    setGoogleBusy(true);
    try {
      const result = await invoke<GoogleSignInResult>("auth_google_sign_in", { relayUrl });
      setRelayEmail(result.email);
      acceptAccountSession({
        mode: "firebase",
        email: result.email,
        uid: result.userId,
        lastLoginAt: Date.now(),
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setGoogleBusy(false);
    }
  };

  const requestRelayOtp = async () => {
    setError(null);
    setRelayBusy(true);
    try {
      await invoke("auth_relay_otp_start", { relayUrl, email, password: relayPassword });
      setRelayOtpSent(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setRelayBusy(false);
    }
  };

  const verifyRelayOtp = async () => {
    setError(null);
    setRelayBusy(true);
    try {
      const result = await invoke<GoogleSignInResult>("auth_relay_otp_verify", {
        relayUrl,
        email,
        password: relayPassword,
        otp: relayOtp,
      });
      setRelayEmail(result.email);
      setRelayOtp("");
      setRelayOtpSent(false);
      acceptAccountSession({
        mode: "firebase",
        email: result.email,
        uid: result.userId,
        lastLoginAt: Date.now(),
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setRelayBusy(false);
    }
  };

  if (configured === null) {
    return <div className="grid min-h-screen place-items-center bg-bg-primary text-text-muted">Loading security...</div>;
  }

  if (unlocked) return <>{children}</>;

  const accountReady = Boolean(accountSession);
  const displayName =
    accountSession?.mode === "firebase" ? accountSession.email : accountSession?.mode === "guest" ? "Guest mode" : null;

  return (
    <main className="auth-shell min-h-screen overflow-hidden px-5 py-8 text-text-primary">
      <div className="auth-orb auth-orb-one" />
      <div className="auth-orb auth-orb-two" />
      <section className="auth-card mx-auto grid w-full max-w-5xl gap-6 rounded-[36px] border border-border-subtle bg-bg-primary/90 p-5 shadow-soft md:grid-cols-[0.9fr_1.1fr] md:p-7">
        <aside className="rounded-[30px] bg-text-primary p-6 text-bg-primary">
          <div className="flex items-center gap-3">
            <img src={logo} alt="" className="h-12 w-12 rounded-2xl bg-bg-primary/90 p-1" />
            <div>
              <p className="text-xs font-black uppercase tracking-[0.25em] text-bg-primary/60">FocusBridge</p>
              <h1 className="text-2xl font-black">Secure start</h1>
            </div>
          </div>
          <p className="mt-8 max-w-sm text-sm leading-6 text-bg-primary/72">
            Sign in for account-owned settings, or use guest mode for a local-only setup. Then unlock the local vault before your dashboard opens.
          </p>
          <div className="mt-8 space-y-3 text-sm">
            <Step active done={accountReady} label="Account" detail={displayName ?? "Login, sign up, or guest"} />
            <Step active={accountReady} done={unlocked} label="Local vault" detail={configured ? "Unlock PIN/password" : "Create PIN/password"} />
            <Step active={unlocked} done={unlocked} label="Dashboard" detail="Notifications, pairing, and rules" />
          </div>
        </aside>

        <div className="rounded-[30px] bg-bg-secondary/80 p-5 md:p-7">
          {!accountReady ? (
            <AccountPanel
              accountMode={accountMode}
              setAccountMode={setAccountMode}
              email={email}
              setEmail={setEmail}
              firebasePassword={firebasePassword}
              setFirebasePassword={setFirebasePassword}
              firebaseBusy={firebaseBusy}
              submitFirebaseAccount={submitFirebaseAccount}
              sendPasswordReset={sendPasswordReset}
              continueAsGuest={continueAsGuest}
              relayUrl={relayUrl}
              setRelayUrl={setRelayUrl}
              relayPassword={relayPassword}
              setRelayPassword={setRelayPassword}
              relayOtp={relayOtp}
              setRelayOtp={setRelayOtp}
              relayOtpSent={relayOtpSent}
              relayBusy={relayBusy}
              requestRelayOtp={requestRelayOtp}
              verifyRelayOtp={verifyRelayOtp}
              googleBusy={googleBusy}
              signInWithGoogle={signInWithGoogle}
              relayEmail={relayEmail}
            />
          ) : (
            <LocalLockPanel
              configured={configured}
              lockMode={lockMode}
              setLockMode={setLockMode}
              password={password}
              setPassword={setPassword}
              securityQuestion={securityQuestion}
              setSecurityQuestion={setSecurityQuestion}
              securityAnswer={securityAnswer}
              setSecurityAnswer={setSecurityAnswer}
              newPassword={newPassword}
              setNewPassword={setNewPassword}
              recoveryQuestion={recoveryQuestion}
              submitLocalLock={submitLocalLock}
              signOut={() => {
                clearAccountSession(window.localStorage);
                setAccountSession(null);
                setUnlocked(false);
                setPassword("");
                setError(null);
                setNotice(null);
              }}
            />
          )}

          {notice && <p className="mt-4 rounded-2xl bg-[#dfeee6] px-4 py-3 text-sm font-semibold text-accent-study">{notice}</p>}
          {error && <p className="mt-4 rounded-2xl bg-[#f2ded6] px-4 py-3 text-sm font-semibold text-[#8f3324]">{error}</p>}
        </div>
      </section>
    </main>
  );
}

function AccountPanel(props: {
  accountMode: AccountMode;
  setAccountMode: (mode: AccountMode) => void;
  email: string;
  setEmail: (value: string) => void;
  firebasePassword: string;
  setFirebasePassword: (value: string) => void;
  firebaseBusy: boolean;
  submitFirebaseAccount: () => Promise<void>;
  sendPasswordReset: () => Promise<void>;
  continueAsGuest: () => void;
  relayUrl: string;
  setRelayUrl: (value: string) => void;
  relayPassword: string;
  setRelayPassword: (value: string) => void;
  relayOtp: string;
  setRelayOtp: (value: string) => void;
  relayOtpSent: boolean;
  relayBusy: boolean;
  requestRelayOtp: () => Promise<void>;
  verifyRelayOtp: () => Promise<void>;
  googleBusy: boolean;
  signInWithGoogle: () => Promise<void>;
  relayEmail: string | null;
}) {
  return (
    <div className="animate-rise-in">
      <p className="text-xs font-black uppercase tracking-[0.24em] text-accent-study">Step 1</p>
      <h2 className="mt-2 text-3xl font-black tracking-[-0.04em]">Account login</h2>
      <p className="mt-2 text-sm leading-6 text-text-secondary">
        Use Firebase login for saved user identity, or continue as guest for local-only use.
      </p>

      <div className="mt-5 grid grid-cols-3 gap-2 rounded-2xl bg-bg-primary/70 p-1">
        {(["login", "signup", "guest"] as const).map((mode) => (
          <button
            key={mode}
            onClick={() => props.setAccountMode(mode)}
            className={
              "rounded-xl px-3 py-2 text-sm font-black capitalize transition " +
              (props.accountMode === mode ? "bg-text-primary text-bg-primary shadow-soft" : "text-text-secondary hover:bg-bg-secondary")
            }
          >
            {mode === "signup" ? "Sign up" : mode}
          </button>
        ))}
      </div>

      {props.accountMode === "guest" ? (
        <div className="mt-6 rounded-3xl border border-border-subtle bg-bg-primary/70 p-5">
          <h3 className="text-lg font-black">Use without an account</h3>
          <p className="mt-2 text-sm leading-6 text-text-secondary">
            Guest mode keeps settings and notifications on this desktop. You can sign in later when cloud settings sync is enabled.
          </p>
          <button onClick={props.continueAsGuest} className="mt-5 w-full rounded-full bg-text-primary px-5 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study active:scale-95">
            Continue as guest
          </button>
        </div>
      ) : (
        <div className="mt-6 space-y-3">
          <input value={props.email} onChange={(event) => props.setEmail(event.target.value)} className="auth-input" placeholder="Email address" />
          <input
            value={props.firebasePassword}
            onChange={(event) => props.setFirebasePassword(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void props.submitFirebaseAccount();
            }}
            type="password"
            className="auth-input"
            placeholder="Account password"
          />
          <button
            onClick={() => void props.submitFirebaseAccount()}
            disabled={props.firebaseBusy}
            className="w-full rounded-full bg-text-primary px-5 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study active:scale-95 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {props.firebaseBusy ? "Checking account..." : props.accountMode === "signup" ? "Create account" : "Log in"}
          </button>
          <button onClick={() => void props.sendPasswordReset()} disabled={props.firebaseBusy} className="w-full rounded-full border border-border-subtle bg-bg-primary px-5 py-3 text-sm font-bold text-text-primary transition hover:-translate-y-0.5 hover:border-border-hover disabled:opacity-60">
            Forgot password?
          </button>
        </div>
      )}

      <details className="mt-5 rounded-3xl border border-border-subtle bg-bg-primary/60 p-4">
        <summary className="cursor-pointer text-xs font-black uppercase tracking-[0.2em] text-text-muted">Advanced relay auth</summary>
        <input value={props.relayUrl} onChange={(event) => props.setRelayUrl(event.target.value)} className="auth-input mt-3" placeholder="http://127.0.0.1:8443" />
        <input value={props.relayPassword} onChange={(event) => props.setRelayPassword(event.target.value)} type="password" className="auth-input mt-3" placeholder="Relay password" />
        {props.relayOtpSent && <input value={props.relayOtp} onChange={(event) => props.setRelayOtp(event.target.value)} className="auth-input mt-3" placeholder="6-digit email code" />}
        <button onClick={() => void (props.relayOtpSent ? props.verifyRelayOtp() : props.requestRelayOtp())} disabled={props.relayBusy} className="mt-3 w-full rounded-full bg-text-primary px-5 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study disabled:opacity-60">
          {props.relayBusy ? "Checking email..." : props.relayOtpSent ? "Verify code" : "Send email code"}
        </button>
        <button onClick={() => void props.signInWithGoogle()} disabled={props.googleBusy} className="mt-3 w-full rounded-full border border-border-subtle bg-bg-primary px-5 py-3 text-sm font-bold text-text-primary transition hover:-translate-y-0.5 hover:border-border-hover disabled:opacity-60">
          {props.googleBusy ? "Waiting for Google..." : props.relayEmail ? "Reconnect Google account" : "Continue with Google"}
        </button>
      </details>
    </div>
  );
}

function LocalLockPanel(props: {
  configured: boolean;
  lockMode: LockMode;
  setLockMode: (mode: LockMode) => void;
  password: string;
  setPassword: (value: string) => void;
  securityQuestion: string;
  setSecurityQuestion: (value: string) => void;
  securityAnswer: string;
  setSecurityAnswer: (value: string) => void;
  newPassword: string;
  setNewPassword: (value: string) => void;
  recoveryQuestion: string | null;
  submitLocalLock: () => Promise<void>;
  signOut: () => void;
}) {
  const title = props.lockMode === "recover" ? "Recover local lock" : props.configured ? "Unlock local vault" : "Create local vault";
  return (
    <div className="animate-rise-in">
      <p className="text-xs font-black uppercase tracking-[0.24em] text-accent-study">Step 2</p>
      <h2 className="mt-2 text-3xl font-black tracking-[-0.04em]">{title}</h2>
      <p className="mt-2 text-sm leading-6 text-text-secondary">
        This protects FocusBridge on this computer even if your Firebase account remains signed in.
      </p>

      {props.lockMode === "recover" ? (
        <div className="mt-6 space-y-3">
          <div className="rounded-2xl border border-border-subtle bg-bg-primary/70 p-4 text-sm font-semibold text-text-secondary">
            {props.recoveryQuestion ?? "Security recovery is not configured on this desktop."}
          </div>
          <input value={props.securityAnswer} onChange={(event) => props.setSecurityAnswer(event.target.value)} className="auth-input" placeholder="Security answer" />
          <input value={props.newPassword} onChange={(event) => props.setNewPassword(event.target.value)} type="password" className="auth-input" placeholder="New PIN or password" />
          <button onClick={() => void props.submitLocalLock()} className="w-full rounded-full bg-text-primary px-5 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study active:scale-95">
            Reset local lock
          </button>
          <button onClick={() => props.setLockMode("unlock")} className="w-full rounded-full border border-border-subtle bg-bg-primary px-5 py-3 text-sm font-bold text-text-primary transition hover:border-border-hover">
            Back to unlock
          </button>
        </div>
      ) : (
        <div className="mt-6 space-y-3">
          <input
            value={props.password}
            onChange={(event) => props.setPassword(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void props.submitLocalLock();
            }}
            type="password"
            autoFocus
            className="auth-input"
            placeholder={props.configured ? "PIN or password" : "Create PIN or password"}
          />
          {!props.configured && (
            <>
              <select
                value={SECURITY_QUESTIONS.includes(props.securityQuestion) ? props.securityQuestion : "Custom question"}
                onChange={(event) => {
                  const value = event.target.value;
                  props.setSecurityQuestion(value === "Custom question" ? "" : value);
                }}
                className="auth-input"
              >
                <option value="" disabled>Choose a security question</option>
                {SECURITY_QUESTIONS.map((question) => (
                  <option key={question} value={question}>{question}</option>
                ))}
              </select>
              {(!props.securityQuestion || !SECURITY_QUESTIONS.includes(props.securityQuestion)) && (
                <input
                  value={props.securityQuestion}
                  onChange={(event) => props.setSecurityQuestion(event.target.value)}
                  className="auth-input"
                  placeholder="Write your custom security question"
                />
              )}
              <input value={props.securityAnswer} onChange={(event) => props.setSecurityAnswer(event.target.value)} className="auth-input" placeholder="Security answer" />
            </>
          )}
          <button onClick={() => void props.submitLocalLock()} className="w-full rounded-full bg-text-primary px-5 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study active:scale-95">
            {props.configured ? "Unlock dashboard" : "Create local lock"}
          </button>
          {props.configured && (
            <button
              onClick={() => props.setLockMode("recover")}
              className="w-full rounded-full border border-border-subtle bg-bg-primary px-5 py-3 text-sm font-bold text-text-primary transition hover:border-border-hover"
            >
              Forgot local PIN/password?
            </button>
          )}
        </div>
      )}

      <button onClick={props.signOut} className="mt-5 text-sm font-bold text-text-muted transition hover:text-text-primary">
        Use a different account
      </button>
    </div>
  );
}

function Step({ active, done, label, detail }: { active: boolean; done: boolean; label: string; detail: string }) {
  return (
    <div className={"rounded-2xl border p-4 transition " + (active ? "border-bg-primary/25 bg-bg-primary/10" : "border-bg-primary/10 bg-transparent opacity-60")}>
      <div className="flex items-center justify-between gap-3">
        <span className="font-black">{label}</span>
        <span className={"h-3 w-3 rounded-full " + (done ? "bg-[#55d18f]" : active ? "bg-[#e0b15a]" : "bg-bg-primary/30")} />
      </div>
      <p className="mt-1 text-xs text-bg-primary/62">{detail}</p>
    </div>
  );
}

function firebaseErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("auth/email-already-in-use")) return "Email is already registered. Use Log in.";
  if (message.includes("auth/user-not-found")) return "No account exists for this email. Use Sign up or continue as guest.";
  if (message.includes("auth/invalid-credential") || message.includes("auth/wrong-password")) return "Invalid email or password.";
  if (message.includes("auth/operation-not-allowed")) {
    return "Firebase Email/Password auth is disabled. Enable it in Firebase Console > Authentication > Sign-in method.";
  }
  if (message.includes("auth/weak-password")) return "Firebase password is too weak. Use at least 6 characters.";
  if (message.includes("auth/missing-email")) return "Enter your email address first.";
  return message;
}
