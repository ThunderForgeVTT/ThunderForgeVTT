/**
 * Spec 048: how the review names what a sheet will change. Plain functions,
 * so the rows stay thin and the wording is tested once.
 */
import type {
  ContentResolution,
  FieldCertainty,
  SheetContentChange,
  SheetFieldChange,
} from "@/api/sheetImport";

/** `ability_data.strength` reads as "strength"; the name as "Name". */
export function fieldLabel(field: SheetFieldChange): string {
  if (field.target === "actor.label") return "Name";
  const leaf = field.target.split(".").pop() ?? field.target;
  return leaf.replace(/_/g, " ");
}

/** A value as the review shows it. Absent reads as an em dash. */
export function shownValue(value: unknown): string {
  if (value === null || value === undefined || value === "") return "—";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return value.every((v) => typeof v !== "object" || v === null)
      ? value.map(String).join(", ")
      : JSON.stringify(value);
  }
  return JSON.stringify(value);
}

export const CERTAINTY_TEXT: Record<FieldCertainty, string> = {
  READ: "read",
  UNCERTAIN: "check this",
  UNREAD: "not read",
  CORRECTED: "corrected",
};

export const RESOLUTION_TEXT: Record<ContentResolution, string> = {
  WORLD: "in the world",
  STAGED_EXISTING: "already awaiting the GM",
  STAGED_NEW: "new: awaits the GM",
  DIFFERS: "differs from the world's: awaits the GM",
};

export function contentText(change: SheetContentChange): string {
  return change.removed
    ? "no longer on the sheet: removed"
    : RESOLUTION_TEXT[change.resolution];
}

/** What accepting does to a field: nothing, a new value, or a replaced one. */
export type ChangeKind = "same" | "new" | "overwrites";

const isAbsent = (value: unknown) =>
  value === null || value === undefined || value === "";

export function changeKind(field: SheetFieldChange): ChangeKind {
  if (JSON.stringify(field.old) === JSON.stringify(field.new)) return "same";
  return isAbsent(field.old) ? "new" : "overwrites";
}

export const CHANGE_TEXT: Record<ChangeKind, string> = {
  same: "",
  new: "new",
  overwrites: "will overwrite",
};

export type FieldFilter = "all" | "uncertain" | "unread";

/** The review's filters: what to check, and what the reader could not read. */
export function filterFields(
  fields: SheetFieldChange[],
  filter: FieldFilter,
): SheetFieldChange[] {
  if (filter === "uncertain") {
    return fields.filter((f) => f.certainty === "UNCERTAIN");
  }
  if (filter === "unread") {
    return fields.filter((f) => f.certainty === "UNREAD");
  }
  return fields;
}

/** A field the person may correct: one the reader was not sure of. */
export function correctable(field: SheetFieldChange): boolean {
  return field.certainty !== "READ";
}

/**
 * The typed text as a correction, shaped like the value it replaces: a
 * number for a number, a list for a list, JSON for anything structured.
 * `undefined` when the text does not fit.
 */
export function parseCorrection(
  field: SheetFieldChange,
  text: string,
): unknown {
  const trimmed = text.trim();
  if (trimmed === "") return undefined;
  const like = field.new ?? field.old;
  if (typeof like === "number" || (like == null && /^-?\d+$/.test(trimmed))) {
    const number = Number(trimmed);
    return Number.isFinite(number) ? number : undefined;
  }
  if (Array.isArray(like) && like.every((v) => typeof v !== "object")) {
    return trimmed
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
  }
  if (like !== null && typeof like === "object") {
    try {
      return JSON.parse(trimmed) as unknown;
    } catch {
      return undefined;
    }
  }
  return trimmed;
}

/** `trait_data.notes` reads as "notes". */
export function goesToText(goesTo: string): string {
  const leaf = goesTo.split(".").pop() ?? goesTo;
  return leaf.replace(/_/g, " ");
}
