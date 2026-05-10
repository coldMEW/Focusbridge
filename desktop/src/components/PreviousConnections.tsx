import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useConnection } from "../hooks/useConnection";
import { summarizePairedDevice } from "../utils/pairedDevices";

interface PairedDevice {
  id: number;
  deviceName: string;
  deviceId: string;
  endpoint?: string | null;
  isActive: number;
  createdAt: number;
  lastConnectedAt?: number | null;
}

export default function PreviousConnections() {
  const [devices, setDevices] = useState<PairedDevice[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const { state } = useConnection();

  const refreshDevices = () => {
    setLoading(true);
    invoke<PairedDevice[]>("list_paired_devices")
      .then((rows) => {
        setDevices(rows);
        setMessage(null);
      })
      .catch((error) => {
        setDevices([]);
        setMessage(`Could not load previous connections: ${String(error)}`);
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    refreshDevices();
    const timer = window.setInterval(refreshDevices, 15_000);
    window.addEventListener("focus", refreshDevices);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refreshDevices);
    };
  }, []);

  const requestReconnect = async (device: PairedDevice) => {
    const summary = summarizePairedDevice(device, state);
    setMessage(`Checking ${summary.name}...`);
    try {
      await invoke("request_device_reconnect", { deviceId: device.deviceId });
      setMessage("Reconnect request sent. Accept it on your phone to resume sync.");
      refreshDevices();
    } catch (error) {
      setMessage(String(error));
    }
  };

  const deleteDevice = async (device: PairedDevice) => {
    const summary = summarizePairedDevice(device, state);
    const confirmed = window.confirm(
      `Remove ${summary.name} from previous connections? You can pair it again later.`,
    );
    if (!confirmed) return;
    setMessage(`Removing ${summary.name}...`);
    try {
      await invoke<number>("delete_paired_device", { deviceId: device.deviceId });
      setDevices((current) => current.filter((row) => row.deviceId !== device.deviceId));
      setMessage(`${summary.name} removed from previous connections.`);
      refreshDevices();
    } catch (error) {
      setMessage(`Remove failed: ${String(error)}`);
    }
  };

  return (
    <section className="glass-panel shrink-0 rounded-[32px] p-5">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.24em] text-accent-study">
            Previous connections
          </p>
          <h3 className="mt-2 text-xl font-semibold tracking-[-0.03em]">
            Reconnect a known phone.
          </h3>
        </div>
        <button
          onClick={refreshDevices}
          className="rounded-full border border-border-subtle px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-text-secondary transition hover:border-border-hover hover:text-text-primary"
        >
          Refresh
        </button>
      </div>

      <div className="mt-4 grid max-h-64 gap-2 overflow-y-auto pr-1">
        {loading && devices.length === 0 && (
          <div className="rounded-2xl bg-bg-secondary/70 px-4 py-3 text-sm text-text-muted">
            Loading saved phones...
          </div>
        )}
        {!loading && devices.length === 0 && (
          <div className="rounded-2xl border border-dashed border-border-subtle bg-bg-secondary/60 px-4 py-4 text-sm leading-6 text-text-secondary">
            No previous connections yet. Pair your Android once and it will appear here.
          </div>
        )}
        {devices.map((device) => {
          const summary = summarizePairedDevice(device, state);
          return (
            <div
              key={device.deviceId}
              className="rounded-2xl border border-border-subtle bg-bg-secondary/70 p-3"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold text-text-primary">
                    {summary.name}
                  </div>
                  <div className="mt-1 truncate text-[11px] text-text-muted">
                    {summary.endpoint} - {summary.lastSeenLabel}
                  </div>
                </div>
                <span
                  title={summary.connected ? "Connected" : "Disconnected"}
                  className={
                    "h-2.5 w-2.5 shrink-0 rounded-full " +
                    (summary.connected ? "bg-[#2f8f61]" : "bg-[#d85b46]")
                  }
                />
              </div>
              <div className="mt-3 grid grid-cols-[minmax(0,1fr)_auto] gap-2">
                <button
                  onClick={() => void requestReconnect(device)}
                  className="rounded-full border border-border-subtle bg-bg-primary/60 px-3 py-2 text-xs font-semibold text-text-secondary transition hover:border-border-hover hover:text-text-primary active:scale-95"
                >
                  {summary.connected ? "Ask phone to confirm sync" : "Reconnect this phone"}
                </button>
                <button
                  onClick={() => void deleteDevice(device)}
                  className="rounded-full border border-[#f0b8aa] bg-[#fff0eb] px-3 py-2 text-xs font-semibold text-[#8f3324] transition hover:bg-[#ffe4dc] active:scale-95"
                  title={`Remove ${summary.name}`}
                >
                  Delete
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {message && <p className="mt-3 text-xs leading-5 text-text-muted">{message}</p>}
      <p className="mt-3 text-xs leading-5 text-text-muted">
        LAN mode cannot wake an offline phone. Open FocusBridge on Android first, then reconnect.
      </p>
    </section>
  );
}
