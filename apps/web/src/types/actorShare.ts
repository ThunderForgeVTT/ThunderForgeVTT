import type { ActorPermissionLevel } from "@/types/actor";

export type { ActorPermissionLevel };

/** Narrow projection returned by `myDmWorlds` — just enough for a
 * destination-world picker, not the full `WorldRecord` shape. */
export type DmWorldSummary = {
  id: string;
  name: string;
};

export type ActorShareLinkRecord = {
  id: string;
  actorId: string;
  shareCode: string;
  revoked: boolean;
  createdAt: string;
};

export type SharedActorPreview = {
  label: string;
  actorType: string;
  isNpc: boolean;
  gameSystemId: string | null;
  systemData: {
    id: string;
    actorId: string;
    gameSystemId: string;
    abilityData: unknown;
    resourceData: unknown;
    proficiencyData: unknown;
    traitData: unknown;
    spellData: unknown;
    createdAt: string;
    updatedAt: string;
  } | null;
};
