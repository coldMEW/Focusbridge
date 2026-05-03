import { useConnection } from "../hooks/useConnection";

export default function ConnectionStatus() {
  const { state, deviceName } = useConnection();
  const color = {
    CONNECTED: "bg-[#7a9870]",
    CONNECTING: "bg-[#9a8860]",
    DISCONNECTED: "bg-[#8a6060]",
    PAUSED: "bg-[#707070]",
  }[state];

  const label = {
    CONNECTED: deviceName ? `Connected · ${deviceName}` : "Connected",
    CONNECTING: "Connecting…",
    DISCONNECTED: "Disconnected",
    PAUSED: "Paused",
  }[state];

  return (
    <div className="flex items-center gap-2 text-xs text-text-secondary">
      <span className={"h-2 w-2 rounded-full " + color} />
      <span>{label}</span>
    </div>
  );
}
