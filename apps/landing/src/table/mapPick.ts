// Spec 098 FR-007: the atlas tile the visitor picked, for the page's table.
//
// `MapLegend` says when the pick changes; `PageTable` reads the pressed
// tile's box itself, so a resize needs no word from the atlas.

const CHANGED = "thunderforge:map-pick";

export function announcePick() {
  window.dispatchEvent(new Event(CHANGED));
}

/** The picked tile's button, while the atlas is on the page. */
export const pickedTile = () =>
  document.querySelector<HTMLElement>('.atlas-tile[aria-pressed="true"]');

export function watchPick(fn: () => void): () => void {
  window.addEventListener(CHANGED, fn);
  return () => window.removeEventListener(CHANGED, fn);
}
