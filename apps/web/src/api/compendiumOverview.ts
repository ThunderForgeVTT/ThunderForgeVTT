/**
 * The Compendium's header description is authored by the GM as an
 * ordinary lore entry (spec 021) rather than a hardcoded sentence, so it
 * reuses the full lore pipeline (Markdown, `[[links]]`, revisions,
 * sanitized server-rendered HTML) instead of a bespoke settings field.
 * A fixed, reserved title/slug pair identifies this one entry per world —
 * there is nothing else structurally special about it.
 */
export const COMPENDIUM_OVERVIEW_TITLE = "Compendium Overview";
export const COMPENDIUM_OVERVIEW_SLUG = "compendium-overview";

export const COMPENDIUM_OVERVIEW_DEFAULT_CONTENT =
  "Browse and curate this world's NPCs, lore, items, and abilities without entering play.";
