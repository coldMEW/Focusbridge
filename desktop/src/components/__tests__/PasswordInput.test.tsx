import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import AuthGate from "../AuthGate";
import PasswordInput from "../PasswordInput";
import { writeAccountSession } from "../../lib/accountSession";
import { firebaseEmailSignIn, firebaseEmailSignUp } from "../../lib/firebaseAuth";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../lib/firebaseAuth", () => ({
  firebaseCurrentUser: vi.fn().mockResolvedValue(null),
  firebaseEmailSignIn: vi.fn(),
  firebaseEmailSignUp: vi.fn(),
  firebaseSendPasswordReset: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.clear();
  vi.mocked(invoke).mockResolvedValue({
    configured: true,
    lockTimeoutMinutes: 0,
    recoveryConfigured: true,
    recoveryQuestion: "Test recovery question",
  });
  const account = { email: "test@example.com", uid: "test-user", idToken: "test-token" };
  vi.mocked(firebaseEmailSignIn).mockResolvedValue(account);
  vi.mocked(firebaseEmailSignUp).mockResolvedValue(account);
});

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

function expectPasswordToggle(label: string, placeholder: string, autoComplete: string) {
  const input = screen.getByPlaceholderText(placeholder);
  const button = screen.getByRole("button", { name: `Show ${label}` });
  expect(input).toHaveAccessibleName(label);
  expect(input).toHaveAttribute("type", "password");
  expect(input).toHaveAttribute("autocomplete", autoComplete);
  expect(input).toHaveClass("auth-input");
  expect(button).toHaveAttribute("type", "button");
  expect(button).toHaveAttribute("aria-controls", input.id);
  expect(button.className).toContain("focus-visible:");

  fireEvent.change(input, { target: { value: "test-secret-123" } });
  button.focus();
  expect(button).toHaveFocus();
  fireEvent.click(button);
  expect(input).toHaveAttribute("type", "text");
  expect(input).toHaveValue("test-secret-123");
  expect(screen.getByRole("button", { name: `Hide ${label}` })).toBe(button);
  fireEvent.click(button);
  expect(input).toHaveAttribute("type", "password");
  expect(input).toHaveValue("test-secret-123");
  expect(screen.getByRole("button", { name: `Show ${label}` })).toBe(button);
  return input;
}

describe("PasswordInput", () => {
  it.each(["", "   "])("provides a nonblank accessible name when label=%j and the placeholder is blank", (label) => {
    render(<PasswordInput label={label} placeholder="" />);
    expect(screen.getByLabelText("Password")).toHaveAttribute("type", "password");
    expect(screen.getByRole("button", { name: "Show Password" })).toBeEnabled();
  });

  it("disables the reveal control when the input is disabled", () => {
    render(<PasswordInput label="Password" disabled defaultValue="test-secret-123" />);
    const button = screen.getByRole("button", { name: "Show Password" });
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(screen.getByLabelText("Password")).toBeDisabled();
    expect(screen.getByLabelText("Password")).toHaveAttribute("type", "password");
  });

  it("preserves input props and handlers while reserving space for the eye button", () => {
    const change = vi.fn();
    const keyDown = vi.fn();
    render(
      <PasswordInput
        label="Password"
        id="test-password"
        name="password"
        defaultValue="1234"
        autoComplete="new-password"
        inputMode="numeric"
        maxLength={8}
        required
        autoFocus
        className="auth-input custom-input"
        wrapperClassName="mt-3"
        style={{ color: "red" }}
        aria-describedby="password-help"
        onChange={change}
        onKeyDown={keyDown}
      />,
    );
    const input = screen.getByLabelText("Password");
    expect(input).toHaveFocus();
    expect(input).toHaveAttribute("id", "test-password");
    expect(input).toHaveAttribute("name", "password");
    expect(input).toHaveAttribute("autocomplete", "new-password");
    expect(input).toHaveAttribute("inputmode", "numeric");
    expect(input).toHaveAttribute("maxlength", "8");
    expect(input).toHaveAttribute("aria-describedby", "password-help");
    expect(input).toBeRequired();
    expect(input).toHaveClass("auth-input", "custom-input", "!pr-12");
    expect(input).toHaveStyle({ color: "rgb(255, 0, 0)" });
    expect(input.parentElement).toHaveClass("relative", "mt-3");
    fireEvent.click(screen.getByRole("button", { name: "Show Password" }));
    expect(input).toHaveValue("1234");
    expect(change).not.toHaveBeenCalled();
    expect(keyDown).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "5678" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(change).toHaveBeenCalledTimes(1);
    expect(keyDown).toHaveBeenCalledTimes(1);
  });

  it("gives each input a unique button association and independent visibility", () => {
    render(<><PasswordInput label="Password" /><PasswordInput label="Confirm password" readOnly defaultValue="test-secret-123" /></>);
    const password = screen.getByLabelText("Password");
    const confirmation = screen.getByLabelText("Confirm password");
    expect(password.id).not.toBe(confirmation.id);
    expect(screen.getByRole("button", { name: "Show Password" })).toHaveAttribute("aria-controls", password.id);
    const confirmButton = screen.getByRole("button", { name: "Show Confirm password" });
    expect(confirmButton).toHaveAttribute("aria-controls", confirmation.id);
    fireEvent.click(confirmButton);
    expect(password).toHaveAttribute("type", "password");
    expect(confirmation).toHaveAttribute("type", "text");
    expect(confirmation).toHaveAttribute("readonly");
    expect(confirmation).toHaveValue("test-secret-123");
  });
});

describe("PasswordInput in AuthGate", () => {
  it("does not submit an enclosing form when toggling visibility", async () => {
    const submit = vi.fn();
    render(
      <form onSubmit={(event) => { event.preventDefault(); submit(); }}>
        <AuthGate>Dashboard</AuthGate>
      </form>,
    );
    await screen.findByRole("heading", { name: "Account login" });
    expectPasswordToggle("Account password", "Account password", "current-password");
    expect(submit).not.toHaveBeenCalled();
  });

  it.each(["login", "signup"] as const)("reveals the %s password without authenticating and preserves Enter", async (mode) => {
    render(<AuthGate>Dashboard</AuthGate>);
    await screen.findByRole("heading", { name: "Account login" });
    if (mode === "signup") fireEvent.click(screen.getByRole("button", { name: "Sign up" }));
    fireEvent.change(screen.getByPlaceholderText("Email address"), { target: { value: "test@example.com" } });

    const input = expectPasswordToggle("Account password", "Account password", mode === "signup" ? "new-password" : "current-password");
    expect(firebaseEmailSignIn).not.toHaveBeenCalled();
    expect(firebaseEmailSignUp).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(mode === "signup" ? firebaseEmailSignUp : firebaseEmailSignIn)
      .toHaveBeenCalledWith("test@example.com", "test-secret-123"));
    await screen.findByRole("heading", { name: "Unlock local vault" });
  });

  it("hides the account password again when switching account modes", async () => {
    render(<AuthGate>Dashboard</AuthGate>);
    await screen.findByRole("heading", { name: "Account login" });
    fireEvent.change(screen.getByPlaceholderText("Account password"), { target: { value: "test-secret-123" } });
    fireEvent.click(screen.getByRole("button", { name: "Show Account password" }));
    fireEvent.click(screen.getByRole("button", { name: "Sign up" }));
    expect(screen.getByLabelText("Account password")).toHaveAttribute("type", "password");
    expect(screen.getByLabelText("Account password")).toHaveValue("test-secret-123");
  });

  it.each([true, false])("reveals the local secret with configured=%s without unlocking and preserves autofocus and Enter", async (configured) => {
    vi.mocked(invoke).mockResolvedValue({ configured, lockTimeoutMinutes: 0, recoveryConfigured: true });
    writeAccountSession(window.localStorage, { mode: "guest", lastLoginAt: Date.now() });
    render(<AuthGate>Dashboard</AuthGate>);
    const placeholder = configured ? "PIN or password" : "Create PIN or password";
    expect(await screen.findByPlaceholderText(placeholder)).toHaveFocus();
    const input = expectPasswordToggle(placeholder, placeholder, configured ? "current-password" : "new-password");
    expect(invoke).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith(
      configured ? "auth_login" : "auth_register_with_recovery",
      configured ? { password: "test-secret-123" } : {
        password: "test-secret-123", securityQuestion: "", securityAnswer: "",
      },
    ));
    await screen.findByText("Dashboard");
  });

  it("reveals the reset secret without submitting recovery and returns to a hidden unlock field", async () => {
    writeAccountSession(window.localStorage, { mode: "guest", lastLoginAt: Date.now() });
    render(<AuthGate>Dashboard</AuthGate>);
    fireEvent.click(await screen.findByRole("button", { name: "Forgot local PIN/password?" }));
    expectPasswordToggle("New PIN or password", "New PIN or password", "new-password");
    expect(invoke).toHaveBeenCalledTimes(1);
    fireEvent.change(screen.getByPlaceholderText("Security answer"), { target: { value: "test-answer" } });
    fireEvent.click(screen.getByRole("button", { name: "Reset local lock" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("auth_reset_password_with_recovery", {
      securityAnswer: "test-answer", newPassword: "test-secret-123",
    }));
    expect(await screen.findByLabelText("PIN or password")).toHaveAttribute("type", "password");
  });

  it("keeps Advanced relay auth and reveals only its password without requesting a code", async () => {
    render(<AuthGate>Dashboard</AuthGate>);
    await screen.findByRole("heading", { name: "Account login" });
    const summary = screen.getByText("Advanced relay auth");
    fireEvent.click(summary);
    expect(summary.closest("details")).toHaveAttribute("open");
    expectPasswordToggle("Relay password", "Relay password", "current-password");
    expect(screen.getByLabelText("Account password")).toHaveAttribute("type", "password");
    expect(invoke).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Send email code" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("auth_relay_otp_start", {
      relayUrl: "http://127.0.0.1:8443", email: "", password: "test-secret-123",
    }));
    await screen.findByRole("button", { name: "Verify code" });
  });
});
