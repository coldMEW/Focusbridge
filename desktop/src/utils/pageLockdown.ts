// The page half of taking the browser out of the webview. The native half
// (window.rs) switches WebView2's own menu and shortcuts off; this covers the
// same ground from inside the page, so a platform or runtime where the native
// switch is missing still cannot export the inbox with one click.

interface KeyLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

// Save page, print, view source, and the developer tools: each of them reads or
// writes the whole page, messages included.
const BLOCKED_WITH_MODIFIER = new Set(["s", "p", "u"]);
const BLOCKED_WITH_MODIFIER_AND_SHIFT = new Set(["i", "j", "c"]);

export function isBlockedShortcut(event: KeyLike): boolean {
  const key = event.key.toLowerCase();
  if (key === "f12") return true;
  const modifier = event.ctrlKey || event.metaKey;
  if (!modifier || event.altKey) return false;
  if (event.shiftKey) return BLOCKED_WITH_MODIFIER_AND_SHIFT.has(key);
  return BLOCKED_WITH_MODIFIER.has(key);
}

// Text fields keep their menu for cut, copy and paste. The page itself does not.
export function allowsContextMenu(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLInputElement) {
    return !["button", "checkbox", "radio", "submit", "reset", "range", "color", "file"].includes(
      target.type,
    );
  }
  return target.isContentEditable === true;
}

export function installPageLockdown(doc: Document = document): () => void {
  // A popup window is a fresh webview that the native lockdown never reached,
  // with its own menu, Save as and Print. Nothing in the app opens one, so
  // nothing may. Printing is refused for the same reason the shortcut is.
  const view = doc.defaultView;
  const originalOpen = view?.open;
  const originalPrint = view?.print;
  if (view) {
    view.open = () => null;
    view.print = () => undefined;
  }
  const onContextMenu = (event: MouseEvent) => {
    if (!allowsContextMenu(event.target)) event.preventDefault();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (isBlockedShortcut(event)) {
      event.preventDefault();
      event.stopPropagation();
    }
  };
  // Dragging selected text or an image out of the window is another way to
  // lift content into a file.
  const onDragStart = (event: DragEvent) => {
    if (!allowsContextMenu(event.target)) event.preventDefault();
  };
  doc.addEventListener("contextmenu", onContextMenu, true);
  doc.addEventListener("keydown", onKeyDown, true);
  doc.addEventListener("dragstart", onDragStart, true);
  return () => {
    doc.removeEventListener("contextmenu", onContextMenu, true);
    doc.removeEventListener("keydown", onKeyDown, true);
    doc.removeEventListener("dragstart", onDragStart, true);
    if (view && originalOpen && originalPrint) {
      view.open = originalOpen;
      view.print = originalPrint;
    }
  };
}
