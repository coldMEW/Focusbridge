import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import NotificationCard from "../NotificationCard";
import type { Notification } from "../../types";

function make(overrides: Partial<Notification> = {}): Notification {
  return {
    id: "n1",
    appName: "WhatsApp",
    packageName: "com.whatsapp",
    sender: "John",
    message: "Hey",
    timestamp: Date.now() - 60_000,
    receivedAt: Date.now(),
    status: "NEW",
    priority: 45,
    contentHidden: false,
    ...overrides,
  };
}

describe("NotificationCard", () => {
  it("renders sender and app", () => {
    render(
      <NotificationCard
        notification={make()}
        onIgnore={() => {}}
        onImportant={() => {}}
        onDelete={() => {}}
      />,
    );
    expect(screen.getByText("WhatsApp")).toBeInTheDocument();
    expect(screen.getByText("John")).toBeInTheDocument();
  });

  it("fires ignore callback", () => {
    const ignore = vi.fn();
    render(
      <NotificationCard
        notification={make()}
        onIgnore={ignore}
        onImportant={() => {}}
        onDelete={() => {}}
      />,
    );
    fireEvent.click(screen.getByText("Ignore"));
    expect(ignore).toHaveBeenCalledWith("n1");
  });

  it("masks hidden content until the user peeks", () => {
    render(
      <NotificationCard
        notification={make({ contentHidden: true, message: "hidden" })}
        onIgnore={() => {}}
        onImportant={() => {}}
        onDelete={() => {}}
      />,
    );
    expect(screen.getByText("Masked message - hover or tap to peek")).toBeInTheDocument();
    expect(screen.queryByText("hidden")).not.toBeInTheDocument();
    fireEvent.mouseEnter(screen.getByLabelText("Masked message. Hover, focus, or click to peek."));
    expect(screen.getByText("hidden")).toBeInTheDocument();
  });

  it("shows 2FA badge for high priority", () => {
    render(
      <NotificationCard
        notification={make({ priority: 100 })}
        onIgnore={() => {}}
        onImportant={() => {}}
        onDelete={() => {}}
      />,
    );
    expect(screen.getByText("security")).toBeInTheDocument();
  });
});
