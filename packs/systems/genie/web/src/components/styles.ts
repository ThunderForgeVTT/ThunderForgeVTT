/**
 * Shared class strings for this pack's session-loop components
 * (`SessionWishPool`/`SessionClocks`/`SessionResourceTrade`), so the three
 * cards that stack together in the host app's Genie session panel read as
 * one section instead of three differently-styled boxes.
 *
 * Everything here is expressed in the host app's design tokens
 * (`--card`/`--border`/`--muted-foreground`/… — see
 * `apps/web/src/styles/globals.css`) rather than fixed palette classes.
 * The previous `bg-white`/`text-gray-600`/`border` styling ignored the
 * token layer entirely, so these cards stayed white-on-grey while the rest
 * of the app was in dark mode. Tokens flip with the theme; raw palette
 * classes don't.
 *
 * Full literal strings, never composed at runtime — Tailwind only sees
 * class names it can find verbatim in source.
 */

/** One card in the session panel's stack. */
export const cardClass =
  'rounded-xl border border-border bg-card p-4 text-card-foreground shadow-sm';

/** A card's title. */
export const cardTitleClass = 'text-sm font-semibold tracking-tight';

/** A subsection heading inside a card. */
export const sectionHeadingClass =
  'text-xs font-semibold tracking-widest text-muted-foreground uppercase';

/** Supporting/secondary copy. */
export const hintClass = 'text-xs text-muted-foreground';

/** `<input>`/`<select>`/`<textarea>` — matches the host app's own controls
 * in `GenieSessionPanel.tsx`. */
export const fieldClass =
  'h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50';

/** Multi-line variant of `fieldClass` (no fixed height). */
export const textareaClass =
  'rounded-lg border border-input bg-transparent px-2.5 py-2 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50';

const buttonBase =
  'inline-flex items-center justify-center gap-1.5 rounded-lg text-sm font-medium transition-colors outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50';

export const primaryButtonClass = `${buttonBase} h-9 px-4 bg-primary text-primary-foreground hover:bg-primary/90`;
export const secondaryButtonClass = `${buttonBase} h-9 px-4 bg-secondary text-secondary-foreground hover:bg-secondary/80`;
export const dangerButtonClass = `${buttonBase} h-9 px-4 bg-destructive text-white hover:bg-destructive/90`;
export const smallButtonClass = `${buttonBase} h-8 px-3 text-xs bg-secondary text-secondary-foreground hover:bg-secondary/80`;
export const smallPrimaryButtonClass = `${buttonBase} h-8 px-3 text-xs bg-primary text-primary-foreground hover:bg-primary/90`;
