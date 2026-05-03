import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface QrData {
  payload: string;
  pngBase64: string;
  expiresAt: number;
}

export default function PairingQR() {
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

  return (
    <div className="flex h-full w-full flex-col items-center justify-center">
      <h2 className="mb-2 text-lg">Pair your phone</h2>
      <p className="mb-6 max-w-xs text-center text-sm text-text-secondary">
        Open FocusBridge on your phone and scan this code.
      </p>
      {error && <p className="text-sm text-text-secondary">Pairing error: {error}</p>}
      {qr ? (
        <img
          src={`data:image/png;base64,${qr.pngBase64}`}
          alt="Pairing QR"
          className="h-56 w-56 border border-border-subtle"
        />
      ) : (
        <div className="h-56 w-56 animate-pulse border border-border-subtle" />
      )}
      <p className="mt-4 text-xs text-text-muted">Waiting for connection…</p>
    </div>
  );
}
