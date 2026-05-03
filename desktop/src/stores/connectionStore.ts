import { create } from "zustand";
import type { ConnectionState } from "../types";

interface ConnectionStoreState {
  state: ConnectionState;
  deviceName?: string | null;
  setState: (s: ConnectionState) => void;
  setDeviceName: (n: string | null) => void;
}

export const useConnectionStore = create<ConnectionStoreState>((set) => ({
  state: "DISCONNECTED",
  deviceName: null,
  setState: (s) => set({ state: s }),
  setDeviceName: (n) => set({ deviceName: n }),
}));
