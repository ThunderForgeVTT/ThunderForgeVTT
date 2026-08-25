import { postGraphQL } from "@/api/graphqlClient";
import type { ActorShareLinkRecord, DmWorldSummary, SharedActorPreview } from "@/types/actorShare";

type CreateActorShareLinkMutation = {
  createActorShareLink: ActorShareLinkRecord;
};

/** Requires effective Owner on the actor (FR-023). */
export function createActorShareLink(actorId: string): Promise<ActorShareLinkRecord> {
  return postGraphQL<CreateActorShareLinkMutation>(
    `
      mutation CreateActorShareLink($actorId: UUID!) {
        createActorShareLink(actorId: $actorId) {
          id
          actorId
          shareCode
          revoked
          createdAt
        }
      }
    `,
    { actorId },
  ).then((data) => data.createActorShareLink);
}

type RevokeActorShareLinkMutation = {
  revokeActorShareLink: boolean;
};

/** The link's creator or the world's DM (FR-029). */
export function revokeActorShareLink(shareId: string): Promise<boolean> {
  return postGraphQL<RevokeActorShareLinkMutation>(
    `
      mutation RevokeActorShareLink($shareId: UUID!) {
        revokeActorShareLink(shareId: $shareId)
      }
    `,
    { shareId },
  ).then((data) => data.revokeActorShareLink);
}

type SharedActorQuery = {
  sharedActor: SharedActorPreview;
};

/** Authenticated-only, world-identity-scrubbed (research.md §9). */
export function getSharedActor(shareCode: string): Promise<SharedActorPreview> {
  return postGraphQL<SharedActorQuery>(
    `
      query SharedActor($shareCode: String!) {
        sharedActor(shareCode: $shareCode) {
          label
          actorType
          isNpc
          gameSystemId
          systemData {
            abilityData
            resourceData
            proficiencyData
            traitData
            spellData
          }
        }
      }
    `,
    { shareCode },
  ).then((data) => data.sharedActor);
}

type MyDmWorldsQuery = {
  myDmWorlds: DmWorldSummary[];
};

/** Worlds where the caller holds DM-level access (research.md §8) — used
 * to populate the "Copy to World" destination picker. */
export function getMyDmWorlds(): Promise<DmWorldSummary[]> {
  return postGraphQL<MyDmWorldsQuery>(
    `
      query MyDmWorlds {
        myDmWorlds {
          id
          name
        }
      }
    `,
  ).then((data) => data.myDmWorlds);
}

type CopySharedActorMutation = {
  copySharedActorToWorld: { id: string; label: string; worldId: string };
};

/** Re-verified server-side regardless of what myDmWorlds returned earlier
 * (FR-025/026/027/030). */
export function copySharedActorToWorld(
  shareCode: string,
  destinationWorldId: string,
): Promise<{ id: string; label: string; worldId: string }> {
  return postGraphQL<CopySharedActorMutation>(
    `
      mutation CopySharedActorToWorld($input: CopySharedActorInput!) {
        copySharedActorToWorld(input: $input) {
          id
          label
          worldId
        }
      }
    `,
    { input: { shareCode, destinationWorldId } },
  ).then((data) => data.copySharedActorToWorld);
}
