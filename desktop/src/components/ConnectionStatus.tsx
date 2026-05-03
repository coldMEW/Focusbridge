import { useConnection } from "../hooks/useConnection";

export default function ConnectionStatus() {
  const { state, deviceName } = useConnection();
  const tone = {
    CONNECTED: "connection-led connected",
    CONNECTING: "connection-led connecting",
    DISCONNECTED: "connection-led disconnected",
    PAUSED: "connection-led paused",
  }[state];

  const label = {
    CONNECTED: deviceName ? `Connected - ${deviceName}` : "Connected",
    CONNECTING: "Connecting...",
    DISCONNECTED: "Disconnected",
    PAUSED: "Paused",
  }[state];

  return (
    <div className="connection-pill" aria-live="polite">
      <span className={tone} aria-hidden="true" />
      <span>{label}</span>
    </div>
  );
}
