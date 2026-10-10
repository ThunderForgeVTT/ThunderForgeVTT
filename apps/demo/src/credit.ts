import credit from "../../../examples/maps/credit.json";

/**
 * Spec 074 FR-018: whose the maps are. The one place it is written; the
 * notice on every page, each scene's description and `maps/NOTICE.txt` all
 * read `examples/maps/credit.json` (spec 088, FR-028).
 */
export const MAP_CREDIT = credit;

export const MAP_CREDIT_LINE = `Map by ${credit.author}, ${credit.licence}. ${credit.source} — more at ${credit.catalog}`;
