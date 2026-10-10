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
