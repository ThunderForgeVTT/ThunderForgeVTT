/**
 * Class strings for this pack's sheet.
 *
 * Expressed in the host app's design tokens (`--card`, `--border`,
 * `--muted-foreground`, …) rather than fixed palette classes, so the sheet
 * follows the theme instead of staying light while the app is dark.
 *
 * Full literal strings, never composed at runtime — Tailwind only sees class
 * names it can find verbatim in source.
 */

/** One section of the sheet. */
export const cardClass =
  "rounded-xl border border-border bg-card p-4 text-card-foreground shadow-sm";

/** A section's title. */
export const cardTitleClass = "text-sm font-semibold tracking-tight";

/** Supporting copy: what a control is for, or what the table decides. */
export const hintClass = "text-xs text-muted-foreground";

/** A single-line control. */
export const fieldClass =
  "h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50";

/** The description box. */
export const textareaClass =
  "w-full rounded-lg border border-input bg-transparent px-2.5 py-2 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50";

/** One die, drawn as a face. */
export const dieClass =
  "inline-flex h-9 w-9 items-center justify-center rounded-lg border border-border bg-muted text-sm font-semibold tabular-nums";

/** A die that counts as a six because experience was spent on it. */
export const boughtDieClass =
  "inline-flex h-9 w-9 items-center justify-center rounded-lg border border-primary bg-primary/10 text-sm font-semibold tabular-nums text-primary";

/**
 * A die the Game Master rolled for the opposition.
 *
 * Drawn differently from a character's die on purpose: these two sets of dice
 * are never added together, never counted together for an advancement, and a
 * player who confuses them has misread the roll.
 */
export const gmDieClass =
  "inline-flex h-9 w-9 items-center justify-center rounded-lg border border-dashed border-border bg-background text-sm font-semibold tabular-nums text-muted-foreground";
