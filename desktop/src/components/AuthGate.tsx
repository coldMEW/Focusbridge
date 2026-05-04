import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import logo from "../assets/logo.png";

interface AuthStatus {
  configured: boolean;
  relayEmail?: string | null;
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
  const [error, setError] = useState<string | null>(null);
  const [googleBusy, setGoogleBusy] = useState(false);

  useEffect(() => {
    invoke<AuthStatus>("auth_status")
      .then((status) => {
        setConfigured(status.configured);
        setRelayEmail(status.relayEmail ?? null);
      })
      .catch((err) => setError(String(err)));
  }, []);

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
              {configured ? "Unlock app" : "Create app password"}
            </h1>
          </div>
        </div>
        <p className="mt-4 text-sm leading-6 text-text-secondary">
          {relayEmail
            ? `Signed in as ${relayEmail}. ${configured ? "Unlock your local app vault." : "Create a local app password for this desktop."}`
            : "Sign in with Google through your FocusBridge relay, then unlock the local app vault."}
        </p>
        <div className="mt-5 rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
          <label className="text-xs font-black uppercase tracking-[0.2em] text-text-muted">
            Relay URL
          </label>
          <input
            value={relayUrl}
            onChange={(event) => setRelayUrl(event.target.value)}
            className="mt-2 w-full rounded-2xl border border-border-subtle bg-bg-primary/80 px-4 py-3 text-sm text-text-primary outline-none transition focus:border-border-hover"
            placeholder="http://127.0.0.1:8443"
          />
          <button
            onClick={() => void signInWithGoogle()}
            disabled={googleBusy}
            className="mt-3 w-full rounded-full border border-border-subtle bg-bg-primary px-5 py-3 text-sm font-bold text-text-primary transition hover:-translate-y-0.5 hover:border-border-hover disabled:cursor-not-allowed disabled:opacity-60"
          >
            {googleBusy ? "Waiting for Google..." : relayEmail ? "Reconnect Google account" : "Continue with Google"}
          </button>
        </div>
        <input
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void submit();
          }}
          type="password"
          autoFocus
          className="mt-5 w-full rounded-2xl border border-border-subtle bg-bg-secondary/80 px-4 py-3 text-text-primary outline-none transition focus:border-border-hover"
          placeholder="Password"
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
