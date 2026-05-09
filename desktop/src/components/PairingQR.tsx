import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ConnectionStatus from "./ConnectionStatus";
import { useConnection } from "../hooks/useConnection";

interface QrData {
  payload: string;
  pngBase64: string;
  expiresAt: number;
}

interface PairedDevice {
  id: number;
  deviceName: string;
  deviceId: string;
  endpoint?: string | null;
  isActive: number;
  createdAt: number;
  lastConnectedAt?: number | null;
}

export default function PairingQR({ compact = false }: { compact?: boolean }) {
  const [qr, setQr] = useState<QrData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [devices, setDevices] = useState<PairedDevice[]>([]);
  const { state } = useConnection();

  const refreshDevices = () => {
    invoke<PairedDevice[]>("list_paired_devices")
      .then(setDevices)
      .catch(() => setDevices([]));
  };

  const refreshQr = () => {
    let alive = true;
    setRefreshing(true);
    setError(null);
    invoke<QrData>("generate_pairing_qr")
      .then((d) => {
        if (alive) setQr(d);
        refreshDevices();
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
    <section className={`glass-panel min-w-0 overflow-hidden rounded-[32px] p-5 ${compact ? "" : "min-h-[420px]"}`}>
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.24em] text-accent-study">
            Pairing
          </p>
          <h2 className="mt-2 text-2xl font-semibold tracking-[-0.035em]">
            {compact ? "Add another phone" : "Pair your Android"}
          </h2>
          <p className="mt-2 text-sm leading-5 text-text-secondary">
            Scan this local QR code from the Android app. If Wi-Fi or hotspot changes, tap refresh
            before scanning.
          </p>
        </div>
        <ConnectionStatus />
      </div>

      {error && <p className="mt-4 text-sm text-[#9b4b3d]">Pairing error: {error}</p>}
      {qr ? (
        <div className="mt-5 grid min-w-0 gap-4">
          <div className="mx-auto max-w-full rounded-[28px] border border-border-subtle bg-white p-3 shadow-soft">
            <img
              src={`data:image/png;base64,${qr.pngBase64}`}
              alt="Pairing QR"
              className={compact ? "h-36 w-36 max-w-full" : "h-56 w-56 max-w-full"}
            />
          </div>
          <button
            onClick={refreshQr}
            disabled={refreshing}
            className="rounded-full bg-text-primary px-4 py-2 text-sm font-semibold text-bg-primary transition hover:bg-accent-study disabled:cursor-wait disabled:opacity-60"
          >
            {refreshing ? "Refreshing QR..." : "Refresh QR / network"}
          </button>
          <div className="w-full min-w-0 overflow-hidden rounded-3xl bg-bg-secondary/80 p-3">
            <div className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
              Manual payload
            </div>
            <code className="mt-2 block max-h-28 w-full min-w-0 max-w-full overflow-auto whitespace-pre-wrap break-all rounded-2xl bg-bg-primary/70 p-2 text-[11px] leading-5 text-text-secondary">
              {qr.payload}
            </code>
          </div>
          {!compact && devices.length > 0 && (
            <div className="rounded-3xl border border-border-subtle bg-bg-secondary/70 p-4">
              <div className="text-[11px] uppercase tracking-[0.2em] text-text-muted">
                Previous connections
              </div>
              <div className="mt-3 grid gap-2">
                {devices.slice(0, 3).map((device) => {
                  const connected = state === "CONNECTED" && device.isActive !== 0;
                  const lastSeen = device.lastConnectedAt ?? device.createdAt;
                  return (
                    <div
                      key={device.deviceId}
                      className="rounded-2xl border border-border-subtle bg-bg-primary/70 p-3"
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0">
                          <div className="truncate text-sm font-semibold text-text-primary">
                            {device.deviceName || "Android phone"}
                          </div>
                          <div className="mt-1 truncate text-[11px] text-text-muted">
                            {device.endpoint || "LAN pairing"} - {relativeTime(lastSeen)}
                          </div>
                        </div>
                        <span
                          className={
                            "h-2.5 w-2.5 shrink-0 rounded-full " +
                            (connected ? "bg-[#2f8f61]" : "bg-[#d85b46]")
                          }
                        />
                      </div>
                      <button
                        onClick={refreshQr}
                        className="mt-3 w-full rounded-full border border-border-subtle px-3 py-2 text-xs font-semibold text-text-secondary transition hover:border-border-hover hover:text-text-primary"
                      >
                        {connected ? "Refresh pairing details" : "Show reconnect QR"}
                      </button>
                    </div>
                  );
                })}
              </div>
              <p className="mt-3 text-xs leading-5 text-text-muted">
                LAN mode cannot wake an offline phone. Open FocusBridge on Android, then scan this QR
                or paste the manual payload to reconnect.
              </p>
            </div>
          )}
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

function relativeTime(timestamp?: number | null): string {
  if (!timestamp) return "Never connected";
  const diff = Date.now() - timestamp;
  if (diff < 60_000) return "Connected just now";
  if (diff < 3_600_000) return `${Math.max(1, Math.round(diff / 60_000))} min ago`;
  if (diff < 86_400_000) return `${Math.max(1, Math.round(diff / 3_600_000))} hr ago`;
  return `${Math.max(1, Math.round(diff / 86_400_000))} day ago`;
}
