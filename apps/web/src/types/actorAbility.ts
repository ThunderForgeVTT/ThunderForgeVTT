import type { AbilityClassification } from "@/types/ability";

/**
 * Spec 025 (T053): one ability an actor knows.
 *
 * `abilityId`/`classification` are null when the source ability was deleted —
 * the entry survives as a tombstone so it can render "Fireball (deleted
 * ability)" rather than vanishing (FR-023). `abilityName` is a server-side
 * snapshot and is always present.
 *
 * A non-DM never receives an entry for a GM-only ability: those are filtered
 * server-side (FR-024b), silently, so the client has no filtering to do.
 */
export type ActorAbilityEntryRecord = {
  id: string;
  actorId: string;
  abilityId: string | null;
  abilityName: string;
  classification: AbilityClassification | null;
  gmOnly: boolean;
};
