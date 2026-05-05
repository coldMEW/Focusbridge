import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import logo from "../assets/logo.png";
import { firebaseCurrentUser, firebaseEmailSignIn, firebaseEmailSignUp } from "../lib/firebaseAuth";

interface AuthStatus {
  configured: boolean;
  relayEmail?: string | null;
  lockTimeoutMinutes: number;
}

interface GoogleSignInResult {
  email: string;
  userId: string;
}

export default function AuthGate({ children }: { children: React.ReactNode }) {
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [relayEmail, setRelayEmail] = useState<string | null>(null);
  const [relayUrl, setRelayUrl] = useState("http://127.0.0.1:8443");
  const [unlocked, setUnlocked] = useState(false);
  const [password, setPassword] = useState("");
  const [relayPassword, setRelayPassword] = useState("");
  const [relayOtp, setRelayOtp] = useState("");
  const [relayOtpSent, setRelayOtpSent] = useState(false);
  const [relayBusy, setRelayBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [googleBusy, setGoogleBusy] = useState(false);
  const [email, setEmail] = useState("");
  const [firebasePassword, setFirebasePassword] = useState("");
  const [firebaseBusy, setFirebaseBusy] = useState(false);
  const [firebaseEmail, setFirebaseEmail] = useState<string | null>(null);
  const [lockTimeoutMinutes, setLockTimeoutMinutes] = useState(0);
  const [lastActivity, setLastActivity] = useState(Date.now());

  useEffect(() => {
    invoke<AuthStatus>("auth_status")
      .then((status) => {
        setConfigured(status.configured);
        setRelayEmail(status.relayEmail ?? null);
        setLockTimeoutMinutes(status.lockTimeoutMinutes ?? 0);
        window.localStorage.setItem("focusbridge.lockTimeoutMinutes", String(status.lockTimeoutMinutes ?? 0));
      })
      .catch((err) => setError(String(err)));
    firebaseCurrentUser()
      .then((user) => {
        if (user?.email) {
          setFirebaseEmail(user.email);
          setRelayEmail((current) => current ?? user.email);
        }
      })
      .catch((err) => setError(String(err)));
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

  const submit = async () => {
    setError(null);
    try {
      if (configured) {
        await invoke("auth_login", { password });
      } else {
        await invoke("auth_register", { password });
        setConfigured(true);
      }
      setUnlocked(true);
      setLastActivity(Date.now());
      setPassword("");
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

  const firebaseEmailPassword = async (mode: "signin" | "signup") => {
    setError(null);
    setFirebaseBusy(true);
    try {
      const result =
        mode === "signup"
          ? await firebaseEmailSignUp(email, firebasePassword)
          : await firebaseEmailSignIn(email, firebasePassword);
      setFirebaseEmail(result.email);
      setRelayEmail(result.email);
      setFirebasePassword("");
    } catch (err) {
      setError(firebaseErrorMessage(err));
    } finally {
      setFirebaseBusy(false);
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

  return (
    <main className="grid min-h-screen place-items-center bg-[radial-gradient(circle_at_top,#f8f2e7,#dfeee6)] px-5">
      <section className="w-full max-w-md rounded-[32px] border border-border-subtle bg-bg-primary/90 p-6 shadow-soft">
        <div className="flex items-center gap-3">
          <img src={logo} alt="" className="h-12 w-12 rounded-2xl" />
          <div>
            <p className="text-xs font-black uppercase tracking-[0.24em] text-accent-study">
              FocusBridge Secure
            </p>
            <h1 className="text-2xl font-black text-text-primary">
          {configured ? "Unlock app" : "Create app password or PIN"}
            </h1>
          </div>
        </div>
        <p className="mt-4 text-sm leading-6 text-text-secondary">
          {relayEmail || firebaseEmail
            ? `Signed in as ${relayEmail ?? firebaseEmail}. ${configured ? "Unlock your local app vault." : "Create a local password or PIN for this desktop."}`
            : "Sign in with Google through your FocusBridge relay, then unlock the local app vault."}
        </p>
        <div className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
          <label className="text-xs font-black uppercase tracking-[0.2em] text-text-muted">
            Firebase email account
          </label>
          <input
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            className="mt-2 w-full rounded-2xl border border-border-subtle bg-bg-primary/80 px-4 py-3 text-sm text-text-primary outline-none transition focus:border-border-hover"
            placeholder="Email"
          />
          <input
            value={firebasePassword}
            onChange={(event) => setFirebasePassword(event.target.value)}
            type="password"
            className="mt-3 w-full rounded-2xl border border-border-subtle bg-bg-primary/80 px-4 py-3 text-sm text-text-primary outline-none transition focus:border-border-hover"
            placeholder="Firebase account password"
          />
          <div className="mt-3 grid grid-cols-2 gap-2">
            <button
              onClick={() => void firebaseEmailPassword("signin")}
              disabled={firebaseBusy}
              className="rounded-full bg-text-primary px-4 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study disabled:cursor-not-allowed disabled:opacity-60"
            >
              Sign in
            </button>
            <button
              onClick={() => void firebaseEmailPassword("signup")}
              disabled={firebaseBusy}
              className="rounded-full border border-border-subtle bg-bg-primary px-4 py-3 text-sm font-bold text-text-primary transition hover:-translate-y-0.5 hover:border-border-hover disabled:cursor-not-allowed disabled:opacity-60"
            >
              Sign up
            </button>
          </div>
        </div>
        <details className="mt-4 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
          <summary className="cursor-pointer text-xs font-black uppercase tracking-[0.2em] text-text-muted">
            Advanced relay auth
          </summary>
          <input
            value={relayUrl}
            onChange={(event) => setRelayUrl(event.target.value)}
            className="mt-3 w-full rounded-2xl border border-border-subtle bg-bg-primary/80 px-4 py-3 text-sm text-text-primary outline-none transition focus:border-border-hover"
            placeholder="http://127.0.0.1:8443"
          />
          <input
            value={relayPassword}
            onChange={(event) => setRelayPassword(event.target.value)}
            type="password"
            className="mt-3 w-full rounded-2xl border border-border-subtle bg-bg-primary/80 px-4 py-3 text-sm text-text-primary outline-none transition focus:border-border-hover"
            placeholder="Relay password"
          />
          {relayOtpSent && (
            <input
              value={relayOtp}
              onChange={(event) => setRelayOtp(event.target.value)}
              className="mt-3 w-full rounded-2xl border border-border-subtle bg-bg-primary/80 px-4 py-3 text-sm text-text-primary outline-none transition focus:border-border-hover"
              placeholder="6-digit email code"
            />
          )}
          <button
            onClick={() => void (relayOtpSent ? verifyRelayOtp() : requestRelayOtp())}
            disabled={relayBusy}
            className="mt-3 w-full rounded-full bg-text-primary px-5 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study disabled:cursor-not-allowed disabled:opacity-60"
          >
            {relayBusy
              ? "Checking email..."
              : relayOtpSent
                ? "Verify code"
                : "Send email code"}
          </button>
          <button
            onClick={() => void signInWithGoogle()}
            disabled={googleBusy}
            className="mt-3 w-full rounded-full border border-border-subtle bg-bg-primary px-5 py-3 text-sm font-bold text-text-primary transition hover:-translate-y-0.5 hover:border-border-hover disabled:cursor-not-allowed disabled:opacity-60"
          >
            {googleBusy ? "Waiting for Google..." : relayEmail ? "Reconnect Google account" : "Continue with Google"}
          </button>
        </details>
        <input
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void submit();
          }}
          type="password"
          autoFocus
          className="mt-5 w-full rounded-2xl border border-border-subtle bg-bg-secondary/80 px-4 py-3 text-text-primary outline-none transition focus:border-border-hover"
          placeholder="Password or PIN"
        />
        {error && <p className="mt-3 text-sm font-semibold text-[#8f3324]">{error}</p>}
        <button
          onClick={() => void submit()}
          className="mt-5 w-full rounded-full bg-text-primary px-5 py-3 text-sm font-bold text-bg-primary transition hover:bg-accent-study active:scale-95"
        >
          {configured ? "Unlock FocusBridge" : "Create password"}
        </button>
      </section>
    </main>
  );
}

function firebaseErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("auth/email-already-in-use")) return "Email is already registered. Use Sign in.";
  if (message.includes("auth/invalid-credential")) return "Invalid email or password.";
  if (message.includes("auth/operation-not-allowed")) {
    return "Firebase Email/Password auth is disabled. Enable it in Firebase Console > Authentication > Sign-in method.";
  }
  if (message.includes("auth/weak-password")) return "Firebase password is too weak. Use at least 6 characters.";
  return message;
}
