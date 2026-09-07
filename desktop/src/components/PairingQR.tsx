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
  // Full size is the reliable way to scan: the side panel is narrow, and a phone
  // held at arm's length needs the modules to be several pixels across.
  const [enlarged, setEnlarged] = useState(false);
  /**
   * Whether a phone that scans the code could actually reach this PC.
   *
   * Across networks that means being present at the relay, and a PC the user has
   * disconnected is not. Showing the code anyway is what left a phone sitting at
   * "connecting" for minutes with nothing on the other end -- sometimes working,
   * sometimes not, depending on whether the code on screen happened to have been
   * asked for or merely drawn.
   */
  const [reachable, setReachable] = useState<boolean | null>(null);

  const refreshReachable = () => {
    invoke<{ configured: boolean; paused: boolean }>("relay_status")
      .then((status) => setReachable(!status.configured || !status.paused))
      .catch(() => setReachable(null));
  };

  // `deliberate` means the user pressed the button, and only that makes this PC
  // reachable over the relay. The panel also renders on load, on window focus
  // and on a timer, and treating any of those as intent would connect this PC
  // to the last phone behind the user's back whenever the pairing view happened
  // to be on screen - which is exactly what the reconnection switch forbids.
  const refreshQr = (deliberate = false) => {
    let alive = true;
    setRefreshing(true);
    setError(null);
    invoke<QrData>("generate_pairing_qr", { forPairing: deliberate && !compact })
      .then((d) => {
        if (alive) setQr(d);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setRefreshing(false);
      });
    // Asking for a code is what makes this PC reachable again.
    if (deliberate) setReachable(true);
    return () => {
      alive = false;
    };
  };

  useEffect(() => {
    refreshReachable();
    const dispose = refreshQr();
    const refreshIfStale = () => {
      setQr((current) => {
        if (!current || current.expiresAt - Date.now() < 60_000) {
          refreshQr();
        }
        return current;
      });
    };
    const refreshOnFocus = () => refreshQr();
    window.addEventListener("focus", refreshOnFocus);
    const timer = window.setInterval(refreshIfStale, 15_000);
    return () => {
      dispose();
      window.removeEventListener("focus", refreshOnFocus);
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
      {reachable === false ? (
        <div className="mt-5 grid gap-4">
          <div className="rounded-[28px] border border-border-subtle bg-bg-secondary/60 p-6 text-sm leading-6 text-text-secondary">
            This PC is disconnected, so a code shown now could not be scanned from
            another network: your phone would wait and never arrive. Ask for a code
            and this PC becomes reachable again.
          </div>
          <button
            onClick={() => refreshQr(true)}
            disabled={refreshing}
            className="rounded-full bg-text-primary px-4 py-2 text-sm font-semibold text-bg-primary transition hover:bg-accent-study disabled:cursor-wait disabled:opacity-60"
          >
            {refreshing ? "Preparing..." : "Show a pairing code"}
          </button>
        </div>
      ) : qr ? (
        <div className="mt-5 grid min-w-0 gap-4">
          <div className="mx-auto w-fit max-w-full rounded-[28px] border border-border-subtle bg-white p-3 shadow-soft">
            {/* Square at every width: fixing both dimensions and then capping the
                width stretched the code into a rectangle in the narrow panel, and
                a distorted QR does not scan. Nearest-neighbour scaling keeps the
                module edges hard, which is what a camera needs to resolve them.

                Size is not cosmetic here. A camera needs several pixels per
                module, and in the side panel this was capped at 224px, which left
                well under three of them -- unreadable however long you held the
                phone there. It now fills the panel, and either code opens full
                screen, which is the size to scan from. */}
            <button
              type="button"
              onClick={() => setEnlarged(true)}
              title="Show the code full size"
              className="block w-full rounded-2xl focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-study"
            >
              <img
                src={`data:image/png;base64,${qr.pngBase64}`}
                alt="Pairing QR"
                style={{ imageRendering: "pixelated" }}
                className={`aspect-square h-auto w-full ${compact ? "max-w-[288px]" : "max-w-[360px]"}`}
              />
            </button>
            <p className="mt-2 text-center text-xs text-text-muted">
              Hard to scan? Click the code to fill the screen.
            </p>
          </div>
          <button
            onClick={() => refreshQr(true)}
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
        <div
          className={`mx-auto mt-5 aspect-square w-full animate-pulse rounded-[28px] border border-border-subtle bg-bg-secondary ${
            compact ? "max-w-[224px]" : "max-w-[360px]"
          }`}
        />
      )}
      {enlarged && qr && (
        <div
          role="dialog"
          aria-label="Pairing code, full size"
          onClick={() => setEnlarged(false)}
          className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-4 bg-black/70 p-6"
        >
          <div className="rounded-[32px] bg-white p-6 shadow-soft">
            <img
              src={`data:image/png;base64,${qr.pngBase64}`}
              alt="Pairing QR, full size"
              style={{ imageRendering: "pixelated" }}
              className="aspect-square h-auto w-[min(70vh,70vw,560px)]"
            />
          </div>
          <p className="text-sm text-white/90">Scan this, then tap anywhere to close.</p>
        </div>
      )}
      <p className="mt-4 text-xs text-text-muted">
        {minutes === null ? "Generating secure local payload..." : `Expires in ${minutes} min`}
      </p>
    </section>
  );
}
