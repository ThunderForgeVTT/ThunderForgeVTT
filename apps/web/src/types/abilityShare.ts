import type {
  AbilityClassification,
  AbilityEffectRecord,
} from "@/types/ability";

export type { DmWorldSummary } from "@/types/actorShare";

export type AbilityShareLinkRecord = {
  id: string;
  abilityId: string;
  shareCode: string;
  revoked: boolean;
  createdAt: string;
};

/**
 * Spec 025 (FR-033): what a share-link viewer sees.
 *
 * Deliberately carries no `id`, `worldId`, or `createdBy` — a viewer must not
 * be able to identify the source world or its members. Adding any of those
 * here would be a real leak, not a convenience.
 */
export type SharedAbilityPreview = {
  name: string;
  description: string | null;
  classification: AbilityClassification;
  /**
   * The word the owning world's system uses for this ability's type.
   *
   * Resolved server-side because this viewer is deliberately not a member of
   * that world and so cannot read its vocabulary (spec 033 FR-006). It names
   * a *type*, which is not world-identifying — the note above still holds.
   */
  classificationLabel: string;
  effects: AbilityEffectRecord[];
};
