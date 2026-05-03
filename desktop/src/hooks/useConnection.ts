import { useConnectionStore } from "../stores/connectionStore";

export function useConnection() {
  return useConnectionStore((s) => ({ state: s.state, deviceName: s.deviceName }));
}
