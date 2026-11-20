import { useSyncExternalStore } from "react";

/**
 * Spec 102 R1: a phone is a viewport whose shorter side is under 600px, so a
 * phone turned on its side stays a phone and a tablet in either orientation
 * keeps the desktop board.
 */
export const PHONE_QUERY = "(max-width: 599px), (max-height: 599px)";

function query(): MediaQueryList | null {
  return typeof globalThis.matchMedia === "function"
    ? globalThis.matchMedia(PHONE_QUERY)
    : null;
}

export function subscribePhone(onChange: () => void): () => void {
  const list = query();
  if (!list) return () => {};
  list.addEventListener("change", onChange);
  return () => list.removeEventListener("change", onChange);
}

export function getPhoneSnapshot(): boolean {
  return query()?.matches ?? false;
}

/**
 * Whether the play board uses its phone arrangement. Rotating or resizing
 * re-renders the chrome; the page's state is the same either way (FR-003).
 */
export function usePhoneLayout(): boolean {
  return useSyncExternalStore(subscribePhone, getPhoneSnapshot, () => false);
}
