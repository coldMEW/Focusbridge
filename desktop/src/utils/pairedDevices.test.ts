import { describe, expect, it } from "vitest";
import { summarizePairedDevice } from "./pairedDevices";

describe("paired device summary", () => {
  it("shows an empty endpoint fallback and offline state", () => {
    const summary = summarizePairedDevice(
      {
        deviceName: "",
        endpoint: null,
        isActive: 0,
        createdAt: Date.now() - 86_400_000,
        lastConnectedAt: null,
      },
      "CONNECTED",
    );

    expect(summary.name).toBe("Android phone");
    expect(summary.endpoint).toBe("LAN pairing");
    expect(summary.connected).toBe(false);
  });

  it("marks an active saved device connected only when desktop is connected", () => {
    const device = {
      deviceName: "Pixel 8",
      endpoint: "ws://192.168.1.20:9173",
      isActive: 1,
      createdAt: Date.now(),
      lastConnectedAt: Date.now(),
    };

    expect(summarizePairedDevice(device, "CONNECTED").connected).toBe(true);
    expect(summarizePairedDevice(device, "DISCONNECTED").connected).toBe(false);
  });
});
