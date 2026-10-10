import { useEffect } from "react";

/**
 * Spec 088 US7 (FR-054): while a form holds unsaved changes, ask before they
 * are lost. A reload or a closed tab gets the browser's own `beforeunload`
 * prompt. An in-app link, which the router follows without unloading, gets a
 * confirm dialog first; the settings page's sections are links, so switching
 * section is asked about the same way.
 *
 * The app mounts a `BrowserRouter`, not a data router, so there is no
 * `useBlocker`. The guard listens for clicks on the document in the capture
 * phase, ahead of the router's own handler on the React root, and stops the
 * click there when the answer is to stay. Programmatic `navigate()` calls are
 * not intercepted: a page that navigates on its own asks with
 * `window.confirm` itself.
 */

export const UNSAVED_MESSAGE =
  "You have unsaved changes. Leave this page and lose them?";

interface ListenerTarget {
  addEventListener(
    type: string,
    listener: (event: Event) => void,
    capture?: boolean,
  ): void;
  removeEventListener(
    type: string,
    listener: (event: Event) => void,
    capture?: boolean,
  ): void;
}

interface GuardedClick {
  button: number;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  defaultPrevented: boolean;
  target: unknown;
  preventDefault(): void;
  stopPropagation(): void;
}

/**
 * Whether following `href` from `here` moves to another page of this app.
 * An anchor on this page stays; another site unloads the page, which
 * `beforeunload` already asks about.
 */
export function leavesThePage(href: string, here: string): boolean {
  const to = new URL(href, here);
  const from = new URL(here);
  if (to.origin !== from.origin) {
    return false;
  }
  return to.pathname !== from.pathname || to.search !== from.search;
}

function anchorOf(target: unknown): { href: string; target: string } | null {
  const element = target as {
    closest?: (selector: string) => { href: string; target: string } | null;
  } | null;
  return element?.closest?.("a[href]") ?? null;
}

/** Installs the guard; returns the function that removes it. */
export function installUnsavedGuard(
  win: ListenerTarget,
  doc: ListenerTarget,
  message: string,
  confirm: (message: string) => boolean,
  here: () => string,
): () => void {
  const onBeforeUnload = (event: {
    preventDefault(): void;
    returnValue: unknown;
  }) => {
    event.preventDefault();
    // Older browsers show the prompt only when this is set.
    event.returnValue = "";
  };
  const onClick = (event: GuardedClick) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return;
    }
    const anchor = anchorOf(event.target);
    if (!anchor || (anchor.target && anchor.target !== "_self")) {
      return;
    }
    if (!leavesThePage(anchor.href, here())) {
      return;
    }
    if (!confirm(message)) {
      event.preventDefault();
      event.stopPropagation();
    }
  };
  // The handlers read only the fields they name, so a test can pass a plain
  // object for the event; the DOM hands them the real one.
  const unload = onBeforeUnload as unknown as (event: Event) => void;
  const click = onClick as unknown as (event: Event) => void;
  win.addEventListener("beforeunload", unload);
  doc.addEventListener("click", click, true);
  return () => {
    win.removeEventListener("beforeunload", unload);
    doc.removeEventListener("click", click, true);
  };
}

export function useUnsavedChanges(
  dirty: boolean,
  message: string = UNSAVED_MESSAGE,
): void {
  useEffect(() => {
    if (!dirty) {
      return;
    }
    return installUnsavedGuard(
      window,
      document,
      message,
      (text) => window.confirm(text),
      () => window.location.href,
    );
  }, [dirty, message]);
}
