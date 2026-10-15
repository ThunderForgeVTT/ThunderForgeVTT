import { holderId } from "../engine/host.ts";

// Whether the page itself holds the table (`PageTable.tsx`). The frames ask
// so they can send their scene to the page rather than take the canvas away
// from it: a roll in the tray tumbles over the page the visitor is reading.

export const PAGE_TABLE = "page-table";
const CHANGED = "thunderforge:page-table";

export const onPage = () => holderId() === PAGE_TABLE;

/** `PageTable` says when it took or lost the table. */
export function announcePage() {
  window.dispatchEvent(new Event(CHANGED));
}

export function watchPage(fn: (on: boolean) => void): () => void {
  const tell = () => fn(onPage());
  window.addEventListener(CHANGED, tell);
  tell();
  return () => window.removeEventListener(CHANGED, tell);
}
