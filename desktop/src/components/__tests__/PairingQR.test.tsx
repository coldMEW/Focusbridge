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

  it("is shown whenever the pairing screen is open", async () => {
    respond({ configured: true, paused: false });

    render(<PairingQR />);

    await waitFor(() => {
      expect(screen.getByAltText("Pairing QR")).toBeTruthy();
    });
  });

  it("is still shown after a disconnect, because a shown code always works", async () => {
    // The code is not hidden and no extra step is asked for. A disconnected PC
    // still waits where a phone that scans this code can find it; what it will
    // not do is accept a phone that has not scanned it.
    respond({ configured: true, paused: true });

    render(<PairingQR />);

    await waitFor(() => {
      expect(screen.getByAltText("Pairing QR")).toBeTruthy();
    });
    expect(screen.queryByRole("button", { name: /show a pairing code/i })).toBeNull();
  });

  it("is shown with no relay configured", async () => {
    respond({ configured: false, paused: false });

    render(<PairingQR />);

    await waitFor(() => {
      expect(screen.getByAltText("Pairing QR")).toBeTruthy();
    });
  });
});
