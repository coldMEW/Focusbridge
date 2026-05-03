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

  useEffect(() => {
    let alive = true;
    invoke<QrData>("generate_pairing_qr")
      .then((d) => {
        if (alive) setQr(d);
      })
      .catch((e) => setError(String(e)));
    return () => {
      alive = false;
    };
  }, []);

  const minutes = qr ? Math.max(0, Math.round((qr.expiresAt - Date.now()) / 60000)) : null;

  return (
    <section className={`glass-panel rounded-[32px] p-5 ${compact ? "" : "min-h-[420px]"}`}>
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.24em] text-accent-study">
            Pairing
          </p>
          <h2 className="mt-2 text-2xl font-semibold tracking-[-0.035em]">
            {compact ? "Add another phone" : "Pair your Android"}
          </h2>
          <p className="mt-2 text-sm leading-5 text-text-secondary">
            Scan this local QR code from the Android app while both devices share Wi-Fi.
          </p>
        </div>
        <ConnectionStatus />
      </div>

      {error && <p className="mt-4 text-sm text-[#9b4b3d]">Pairing error: {error}</p>}
      {qr ? (
        <div className="mt-5 grid gap-4">
          <div className="mx-auto rounded-[28px] border border-border-subtle bg-white p-3 shadow-soft">
            <img
              src={`data:image/png;base64,${qr.pngBase64}`}
              alt="Pairing QR"
              className={compact ? "h-36 w-36" : "h-56 w-56"}
            />
          </div>
          <div className="rounded-3xl bg-bg-secondary/80 p-3">
            <div className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
              Manual payload
            </div>
            <code className="mt-2 block max-h-24 overflow-auto break-all text-[11px] leading-5 text-text-secondary">
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
