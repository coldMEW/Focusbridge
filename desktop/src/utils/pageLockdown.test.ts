import { afterEach, describe, expect, it } from "vitest";
import { allowsContextMenu, installPageLockdown, isBlockedShortcut } from "./pageLockdown";

const key = (k: string, mods: Partial<{ ctrl: boolean; shift: boolean; meta: boolean; alt: boolean }> = {}) => ({
  key: k,
  ctrlKey: Boolean(mods.ctrl),
  shiftKey: Boolean(mods.shift),
  metaKey: Boolean(mods.meta),
  altKey: Boolean(mods.alt),
});

describe("page lockdown", () => {
  let dispose: (() => void) | null = null;
  afterEach(() => {
    dispose?.();
    dispose = null;
    document.body.innerHTML = "";
  });

  it("blocks save, print, view source and developer tools", () => {
    expect(isBlockedShortcut(key("s", { ctrl: true }))).toBe(true);
    expect(isBlockedShortcut(key("S", { ctrl: true }))).toBe(true);
    expect(isBlockedShortcut(key("p", { ctrl: true }))).toBe(true);
    expect(isBlockedShortcut(key("u", { ctrl: true }))).toBe(true);
    expect(isBlockedShortcut(key("s", { meta: true }))).toBe(true);
    expect(isBlockedShortcut(key("i", { ctrl: true, shift: true }))).toBe(true);
    expect(isBlockedShortcut(key("F12"))).toBe(true);
  });

  it("leaves editing keys alone", () => {
    for (const k of ["c", "v", "x", "a", "z", "y"]) {
      expect(isBlockedShortcut(key(k, { ctrl: true }))).toBe(false);
    }
    expect(isBlockedShortcut(key("s"))).toBe(false);
    expect(isBlockedShortcut(key("Enter"))).toBe(false);
  });

  it("offers no context menu on messages, but keeps it in text fields", () => {
    const message = document.createElement("p");
    const field = document.createElement("input");
    field.type = "text";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    const area = document.createElement("textarea");
    expect(allowsContextMenu(message)).toBe(false);
    expect(allowsContextMenu(checkbox)).toBe(false);
    expect(allowsContextMenu(field)).toBe(true);
    expect(allowsContextMenu(area)).toBe(true);
    expect(allowsContextMenu(null)).toBe(false);
  });

  it("actually cancels the right click and the save shortcut once installed", () => {
    dispose = installPageLockdown(document);
    const message = document.createElement("p");
    document.body.appendChild(message);

    const rightClick = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    message.dispatchEvent(rightClick);
    expect(rightClick.defaultPrevented).toBe(true);

    const save = new KeyboardEvent("keydown", { key: "s", ctrlKey: true, bubbles: true, cancelable: true });
    message.dispatchEvent(save);
    expect(save.defaultPrevented).toBe(true);

    const copy = new KeyboardEvent("keydown", { key: "c", ctrlKey: true, bubbles: true, cancelable: true });
    message.dispatchEvent(copy);
    expect(copy.defaultPrevented).toBe(false);
  });

  it("opens no popup window and prints nothing", () => {
    dispose = installPageLockdown(document);
    expect(window.open("https://example.com", "_blank")).toBeNull();
    expect(() => window.print()).not.toThrow();
  });
});
