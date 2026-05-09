import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ConnectionStatus from "./ConnectionStatus";

interface QrData {
  payload: string;
  pngBase64: string;
  expiresAt: number;
}

export default function PairingQR({ compact = false }: { compact?: boolean }) {
  const [qr, setQr] = useState<QrData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const refreshQr = () => {
    let alive = true;
    setRefreshing(true);
    setError(null);
    invoke<QrData>("generate_pairing_qr")
      .then((d) => {
        if (alive) setQr(d);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setRefreshing(false);
      });
    return () => {
      alive = false;
    };
  };

  useEffect(() => {
    const dispose = refreshQr();
    const refreshIfStale = () => {
      setQr((current) => {
        if (!current || current.expiresAt - Date.now() < 60_000) {
          refreshQr();
        }
        return current;
      });
    };
    window.addEventListener("focus", refreshQr);
    const timer = window.setInterval(refreshIfStale, 15_000);
    return () => {
      dispose();
      window.removeEventListener("focus", refreshQr);
      window.clearInterval(timer);
    };
  }, []);

  const minutes = qr ? Math.max(0, Math.round((qr.expiresAt - Date.now()) / 60000)) : null;

  return (
    <section className="glass-panel min-w-0 rounded-[32px] p-5">
      <div className="grid gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-xs font-semibold uppercase tracking-[0.24em] text-accent-study">
            Pairing
          </p>
          <ConnectionStatus />
        </div>
        <div>
          <h2 className="mt-2 text-2xl font-semibold tracking-[-0.035em]">
            {compact ? "Add another device" : "Pair your device"}
          </h2>
          <p className="mt-2 max-w-[32rem] text-sm leading-6 text-text-secondary">
            Scan this local QR code from the Android app. If Wi-Fi or hotspot changes, tap refresh
            before scanning.
          </p>
        </div>
      </div>

      {error && <p className="mt-4 text-sm text-[#9b4b3d]">Pairing error: {error}</p>}
      {qr ? (
        <div className="mt-5 grid min-w-0 gap-4">
          <div className="mx-auto max-w-full rounded-[28px] border border-border-subtle bg-white p-3 shadow-soft">
            <img
              src={`data:image/png;base64,${qr.pngBase64}`}
              alt="Pairing QR"
              className={compact ? "h-40 w-40 max-w-full" : "h-56 w-56 max-w-full"}
            />
          </div>
          <button
            onClick={refreshQr}
            disabled={refreshing}
            className="rounded-full bg-text-primary px-4 py-2 text-sm font-semibold text-bg-primary transition hover:bg-accent-study disabled:cursor-wait disabled:opacity-60"
          >
            {refreshing ? "Refreshing QR..." : "Refresh QR / network"}
          </button>
          <div className="w-full min-w-0 rounded-3xl bg-bg-secondary/80 p-3">
            <div className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
              Manual payload
            </div>
            <code className="mt-2 block max-h-36 w-full min-w-0 max-w-full overflow-auto whitespace-pre-wrap break-all rounded-2xl bg-bg-primary/70 p-3 text-[11px] leading-5 text-text-secondary">
              {qr.payload}
            </code>
          </div>
        </div>
      ) : (
        <div className="mx-auto mt-5 h-56 w-56 animate-pulse rounded-[28px] border border-border-subtle bg-bg-secondary" />
      )}
      <p className="mt-4 text-xs text-text-muted">
        {minutes === null ? "Generating secure local payload..." : `Expires in ${minutes} min`}
      </p>
    </section>
  );
}
