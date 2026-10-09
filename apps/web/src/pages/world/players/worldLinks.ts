/**
 * Spec 088 (US1): the pure part of the world links panel. What a GM's
 * choices send to `generateInviteCode`, and how the list is ordered and
 * described.
 */
import type { WorldLinkOptions } from "@/api/world";
import type { WorldInviteDoc } from "@/db/collections/worldInvitesCollection";

export type LinkExpiry = "1d" | "7d" | "30d" | "never";

export interface LinkChoice {
  /** `null` is no limit; otherwise 1 to 50 uses. */
  limit: number | null;
  expiry: LinkExpiry;
}

/** FR-002: no limit, and 7 days. */
export const DEFAULT_LINK_CHOICE: LinkChoice = { limit: null, expiry: "7d" };

export const MAX_LINK_USES = 50;

export const EXPIRY_LABELS: Record<LinkExpiry, string> = {
  "1d": "1 day",
  "7d": "7 days",
  "30d": "30 days",
  never: "Never",
};

const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * The server's defaults are the panel's defaults, so a default choice sends
 * neither key: the server stays the one place that says what "7 days" is.
 */
export function linkOptionsFrom(
  choice: LinkChoice,
  now: Date,
): WorldLinkOptions {
  const options: WorldLinkOptions = {};
  if (choice.limit !== null) options.maxUses = choice.limit;
  if (choice.expiry === "never") options.expiresAt = null;
  if (choice.expiry === "1d" || choice.expiry === "30d") {
    const days = choice.expiry === "1d" ? 1 : 30;
    options.expiresAt = new Date(now.getTime() + days * DAY_MS).toISOString();
  }
  return options;
}

/** Working links first, as they came; the rest under **Past links**. */
export function splitLinks(links: WorldInviteDoc[]): {
  active: WorldInviteDoc[];
  past: WorldInviteDoc[];
} {
  return {
    active: links.filter((link) => link.state === "ACTIVE"),
    past: links.filter((link) => link.state !== "ACTIVE"),
  };
}

/** Its joins, and its uses left when it has a limit. */
export function describeUses(link: WorldInviteDoc): string {
  const joins =
    link.used_count === 0
      ? "No joins yet"
      : `${link.used_count} ${link.used_count === 1 ? "join" : "joins"}`;
  if (link.max_uses === null || link.max_uses === undefined) return joins;
  const left =
    link.remaining_uses ?? Math.max(0, link.max_uses - link.used_count);
  return `${joins} · ${left} of ${link.max_uses} uses left`;
}
