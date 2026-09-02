import { postGraphQL, postGraphQLMultipart } from "@/api/graphqlClient";
import type {
  ActorPermissionLevel,
  ActorPermissionRecord,
  WorldActorRecord,
} from "@/types/actor";

const WORLD_ACTOR_FIELDS = `
  id
  worldId
  sceneId
  actorType
  gameSystemId
  label
  description
  isPublic
  isNpc
  createdBy
  ownedBy
  myPermissionLevel
  createdAt
  updatedAt
  loreLinkedFrom {
    id
    title
    slug
  }
  availableForClaim
  claimedBy {
    id
    worldId
    userId
    username
  }
`;

type WorldActorsQuery = {
  worldActors: WorldActorRecord[];
};

/**
 * Fetch every actor (NPCs and player characters, distinguished by `isNpc`)
 * in a world, across every scene — used by the GM staging page and the
 * full-screen sidebar's NPC/combat roster (spec 009), and by spec 010's
 * actor detail/edit routes.
 */
export function getWorldActors(worldId: string): Promise<WorldActorRecord[]> {
  return postGraphQL<WorldActorsQuery>(
    `
      query WorldActors($worldId: UUID!) {
        worldActors(worldId: $worldId) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldActors);
}

/**
 * Fetch a single actor by id — implemented by filtering a `worldActors`
 * fetch client-side (no dedicated single-actor query exists yet), used by
 * `ActorDetailPage.tsx`'s view/edit routes.
 */
export async function getActor(
  worldId: string,
  actorId: string,
): Promise<WorldActorRecord | null> {
  const actors = await getWorldActors(worldId);
  return actors.find((actor) => actor.id === actorId) ?? null;
}

type SearchActorsQuery = {
  searchActors: WorldActorRecord[];
};

/**
 * Server-side ILIKE search over a world's actor label/description
 * (`queries/actor.rs`'s `searchActors`) — pairs with the client-side
 * FlexSearch index (`@/search/actorSearch`) for callers that haven't
 * (or can't) mirror the full roster locally, or want a fresh
 * server-authoritative match against a roster too large to keep in
 * sync client-side.
 */
export function searchActors(
  worldId: string,
  query: string,
): Promise<WorldActorRecord[]> {
  return postGraphQL<SearchActorsQuery>(
    `
      query SearchActors($worldId: UUID!, $query: String!) {
        searchActors(worldId: $worldId, query: $query) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { worldId, query },
  ).then((data) => data.searchActors);
}

type CreateActorInput = {
  worldId: string;
  label: string;
  isNpc: boolean;
  actorType?: string;
  gameSystemId?: string;
  description?: string;
};

type CreateActorMutation = {
  createActor: WorldActorRecord;
};

/** DM-only (FR-019). Creates a new actor in the world's default scene. */
export function createActor(
  input: CreateActorInput,
): Promise<WorldActorRecord> {
  return postGraphQL<CreateActorMutation>(
    `
      mutation CreateActor($input: CreateActorInput!) {
        createActor(input: $input) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createActor);
}

type UpdateActorInput = {
  actorId: string;
  label?: string;
  isNpc?: boolean;
  actorType?: string;
  description?: string;
};

type UpdateActorMutation = {
  updateActor: WorldActorRecord;
};

/** Requires Editor or Owner effective permission (FR-010, FR-011). */
export function updateActor(
  input: UpdateActorInput,
): Promise<WorldActorRecord> {
  return postGraphQL<UpdateActorMutation>(
    `
      mutation UpdateActor($input: UpdateActorInput!) {
        updateActor(input: $input) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.updateActor);
}

type ActorPermissionsQuery = {
  actorPermissions: ActorPermissionRecord[];
};

/** DM-only (FR-014). Returns only explicit ownership-block rows. */
export function getActorPermissions(
  actorId: string,
): Promise<ActorPermissionRecord[]> {
  return postGraphQL<ActorPermissionsQuery>(
    `
      query ActorPermissions($actorId: UUID!) {
        actorPermissions(actorId: $actorId) {
          actorId
          userId
          level
          updatedAt
        }
      }
    `,
    { actorId },
  ).then((data) => data.actorPermissions);
}

type SetActorPermissionMutation = {
  setActorPermission: ActorPermissionRecord;
};

/** DM-only (FR-014). UPSERT on (actorId, userId). */
export function setActorPermission(
  actorId: string,
  userId: string,
  level: ActorPermissionLevel,
): Promise<ActorPermissionRecord> {
  return postGraphQL<SetActorPermissionMutation>(
    `
      mutation SetActorPermission($input: SetActorPermissionInput!) {
        setActorPermission(input: $input) {
          actorId
          userId
          level
          updatedAt
        }
      }
    `,
    { input: { actorId, userId, level } },
  ).then((data) => data.setActorPermission);
}

type RemoveActorPermissionMutation = {
  removeActorPermission: boolean;
};

/** DM-only (FR-014). Idempotent — reverts the member to default Viewer. */
export function removeActorPermission(
  actorId: string,
  userId: string,
): Promise<boolean> {
  return postGraphQL<RemoveActorPermissionMutation>(
    `
      mutation RemoveActorPermission($actorId: UUID!, $userId: UUID!) {
        removeActorPermission(actorId: $actorId, userId: $userId)
      }
    `,
    { actorId, userId },
  ).then((data) => data.removeActorPermission);
}

type SetActorAvailabilityMutation = {
  setActorAvailability: WorldActorRecord;
};

/**
 * Spec 017 (FR-004): GM-only (Owner-level Actor permission). Marks/unmarks
 * a PC-classified actor as offered on the Actor Selection screen.
 */
export function setActorAvailability(
  actorId: string,
  available: boolean,
): Promise<WorldActorRecord> {
  return postGraphQL<SetActorAvailabilityMutation>(
    `
      mutation SetActorAvailability($actorId: UUID!, $available: Boolean!) {
        setActorAvailability(actorId: $actorId, available: $available) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { actorId, available },
  ).then((data) => data.setActorAvailability);
}

type UnclaimActorMutation = {
  unclaimActor: WorldActorRecord;
};

/**
 * Spec 017 (FR-013): GM-only. Frees a claimed character (e.g. a player
 * left, or a mistake was made) without removing the prior claimant from
 * the world.
 *
 * Spec 031 (FR-034): `expectedWorldMemberId` names the claim the caller was
 * looking at when it offered the release. The players section writes the
 * same relation, so a page left open can be describing a binding that has
 * already moved; the server refuses such a release as `CLAIM_CHANGED`
 * rather than erasing a binding this screen never showed. Callers that
 * genuinely mean "whoever holds it" omit it.
 */
export function unclaimActor(
  actorId: string,
  expectedWorldMemberId?: string,
): Promise<WorldActorRecord> {
  return postGraphQL<UnclaimActorMutation>(
    `
      mutation UnclaimActor($actorId: UUID!, $expectedWorldMemberId: UUID) {
        unclaimActor(actorId: $actorId, expectedWorldMemberId: $expectedWorldMemberId) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { actorId, expectedWorldMemberId: expectedWorldMemberId ?? null },
  ).then((data) => data.unclaimActor);
}

/**
 * Spec 031 (FR-036): one of an actor's images, named by what it is for.
 *
 * `role` is the stored string rather than a union: ADR-057 keeps the set open
 * so the deferred presentation images are additive, and a role this client
 * does not recognise is meant to be skipped, not rendered in the wrong place.
 */
export interface ActorImageRecord {
  id: string;
  actorId: string;
  role: string;
  assetId: string;
  url: string;
  thumbnailUrl: string;
}

/** The two roles this application renders today (ADR-057). */
export const ACTOR_IMAGE_PORTRAIT = "portrait";
export const ACTOR_IMAGE_TOKEN = "token";

/**
 * Every actor's imagery in one world, keyed by actor id.
 *
 * A separate document from `WORLD_ACTOR_FIELDS` on purpose: `images` costs a
 * query per actor server-side, and the roster is fetched on screens — the
 * staging page, the play sidebar — that never show a picture. The surfaces
 * that do show one ask for it.
 */
export async function getWorldActorImages(
  worldId: string,
): Promise<Record<string, ActorImageRecord[]>> {
  const data = await postGraphQL<{
    worldActors: { id: string; images: ActorImageRecord[] }[];
  }>(
    `
      query WorldActorImages($worldId: UUID!) {
        worldActors(worldId: $worldId) {
          id
          images {
            id
            actorId
            role
            assetId
            url
            thumbnailUrl
          }
        }
      }
    `,
    { worldId },
  );
  return Object.fromEntries(
    data.worldActors.map((actor) => [actor.id, actor.images]),
  );
}

/**
 * FR-036: uploads one image for one role, replacing whatever that role held.
 *
 * Sent as an `Upload!` scalar over the GraphQL multipart request spec, the
 * same way `uploadLoreImage`/`uploadCanvasImage` send theirs — a JSON body
 * cannot carry the bytes. The server transcodes to WebP and refuses an
 * oversized or undecodable file before writing anything, so a rejection here
 * means the actor's imagery is exactly as it was.
 */
export async function uploadActorImage(
  actorId: string,
  role: string,
  file: Blob,
): Promise<ActorImageRecord> {
  const data = await postGraphQLMultipart<{
    uploadActorImage: ActorImageRecord;
  }>(
    `
      mutation UploadActorImage($actorId: UUID!, $role: String!, $file: Upload!) {
        uploadActorImage(actorId: $actorId, role: $role, file: $file) {
          id
          actorId
          role
          assetId
          url
          thumbnailUrl
        }
      }
    `,
    { actorId, role },
    file,
    "file",
  );
  return data.uploadActorImage;
}

/** Removes one role's image, leaving the actor's other roles untouched. */
export function removeActorImage(
  actorId: string,
  role: string,
): Promise<boolean> {
  return postGraphQL<{ removeActorImage: boolean }>(
    `
      mutation RemoveActorImage($actorId: UUID!, $role: String!) {
        removeActorImage(actorId: $actorId, role: $role)
      }
    `,
    { actorId, role },
  ).then((data) => data.removeActorImage);
}
