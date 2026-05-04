import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import logo from "../assets/logo.png";

interface AuthStatus {
  configured: boolean;
}

export default function AuthGate({ children }: { children: React.ReactNode }) {
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [unlocked, setUnlocked] = useState(false);
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AuthStatus>("auth_status")
      .then((status) => setConfigured(status.configured))
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
          {configured
            ? "Enter your local app password to open notifications and settings."
            : "Create a local password before using FocusBridge on this desktop."}
        </p>
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
