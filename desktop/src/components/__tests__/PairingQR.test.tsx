import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("../ConnectionStatus", () => ({ default: () => null }));

import PairingQR from "../PairingQR";

const qr = {
  payload: '{"v":2}',
  pngBase64: "iVBORw0KGgo=",
  expiresAt: Date.now() + 300_000,
};

function respond(status: { configured: boolean; paused: boolean }) {
  invoke.mockImplementation((command: string) => {
    if (command === "relay_status") return Promise.resolve(status);
    if (command === "generate_pairing_qr") return Promise.resolve(qr);
    return Promise.resolve(null);
  });
}

describe("the pairing code", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("is not shown at all when this PC would not answer a scan of it", async () => {
    // A disconnected PC is not present at the relay, so a phone on another
    // network scans a valid code and then waits for a machine that is not
    // listening. That is what made pairing work sometimes and not others.
    respond({ configured: true, paused: true });

    render(<PairingQR />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /show a pairing code/i })).toBeTruthy();
    });
    expect(screen.queryByAltText("Pairing QR")).toBeNull();
  });

  it("is shown once this PC is reachable", async () => {
    respond({ configured: true, paused: false });

    render(<PairingQR />);

    await waitFor(() => {
      expect(screen.getByAltText("Pairing QR")).toBeTruthy();
    });
  });

  it("is shown when there is no relay to be absent from", async () => {
    // A LAN-only pairing needs no relay presence: the listener is always up, so
    // a code always works and hiding it would be wrong.
    respond({ configured: false, paused: true });

    render(<PairingQR />);

    await waitFor(() => {
      expect(screen.getByAltText("Pairing QR")).toBeTruthy();
    });
  });
});
