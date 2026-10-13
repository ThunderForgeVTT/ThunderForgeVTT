import type { PauseTrigger } from "@/api/playPause";

export function formatMoment(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

export function messageOf(cause: unknown, otherwise: string): string {
  return cause instanceof Error ? cause.message : otherwise;
}

/**
 * How often the page re-reads the requests and the pauses in force. A
 * takedown raises a request with nobody watching, and another operator may
 * decide one or pause a world meanwhile; the page shows that on its own.
 */
export const REFRESH_MS = 5_000;

export const TRIGGER_KIND_LABEL: Record<PauseTrigger["kind"], string> = {
  TAKEDOWN: "Takedown",
  OPERATOR: "Operator",
  ABUSE_REPORT: "Abuse report",
};

/** `world_actor` and friends, as a person would say them. */
export function entityLabel(entityType: string | null): string | null {
  switch (entityType) {
    case null:
      return null;
    case "scene":
      return "a scene";
    case "world_actor":
      return "an actor";
    case "world_item":
      return "an item";
    case "world_lore_entry":
      return "a lore entry";
    default:
      return `a ${entityType.replace(/_/g, " ")}`;
  }
}

/** How many entries of each part of the record one "Show more" reads. */
export const RECORD_PAGE = 20;
/** The most the server hands back in one page (`MAX_PAUSES_PER_PAGE`). */
export const RECORD_MAX_PAGE = 200;
