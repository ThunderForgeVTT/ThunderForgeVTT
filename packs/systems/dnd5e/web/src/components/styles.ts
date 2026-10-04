/**
 * Shared class strings for this pack's host-mounted sheet, in the host app's
 * design tokens (`--card`/`--border`/`--muted-foreground`/…, see
 * `apps/web/src/styles/globals.css`) rather than fixed palette classes, so
 * the sheet follows the theme like every other card on the page.
 *
 * The same tokens as `packs/systems/genie/web/src/components/styles.ts`, kept
 * per pack on purpose: a pack is a unit that can be copied out whole.
 *
 * Full literal strings, never composed at runtime — Tailwind only sees class
 * names it can find verbatim in source.
 */

/** One card on the sheet. */
export const cardClass =
  "rounded-xl border border-border bg-card p-4 text-card-foreground shadow-sm";

/** A subsection heading inside a card. */
export const sectionHeadingClass =
  "text-xs font-semibold tracking-widest text-muted-foreground uppercase";

/** Supporting/secondary copy. */
export const hintClass = "text-xs text-muted-foreground";

/** `<input>`/`<select>` — matches the host app's own controls. */
export const fieldClass =
  "h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50";

/** Multi-line variant of `fieldClass` (no fixed height). */
export const textareaClass =
  "rounded-lg border border-input bg-transparent px-2.5 py-2 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50";

/** A single stat tile: a score with its modifier, or a derived number. */
export const tileClass =
  "grid justify-items-center gap-1 rounded-lg border border-border bg-muted/30 p-3 text-center";
