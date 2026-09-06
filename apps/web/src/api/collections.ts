import { postGraphQL } from "@/api/graphqlClient";
import type {
  CollectionMemberRecord,
  CollectionRecord,
  CollectionShareLinkRecord,
  CopyReceipt,
  DmWorldSummary,
  SharedCollectionPreview,
} from "@/types/collection";

/**
 * Spec 026: content collections, and the links that share them.
 *
 * ⚠️ There is deliberately **no** "list my shares" or "shares in this world"
 * call here, and none may be added. No-enumeration is one of the invariants
 * ADR-069's determination is conditional on — adding a listing re-opens it.
 * `worldCollections` lists a world's *collections*, which a DM of that world
 * may already see; it does not list share links.
 */

// `/api/graphql` sits behind a router-level auth gate. The collection preview
// is readable without an account (FR-009a, ADR-070), so it goes to the same
// unauthenticated route spec 015 built for the takedown notice — as do the
// other three share previews since ADR-071. Copying does not — see
// `copySharedCollectionToWorld` below, which is where viewing and copying
// deliberately diverge (FR-009b).
const GRAPHQL_PUBLIC_ENDPOINT = "/api/graphql/public";

const COLLECTION_FIELDS = `
  id
  worldId
  name
  description
  memberCount
  createdAt
  updatedAt
`;

const MEMBER_FIELDS = `
  id
  collectionId
  memberType
  memberId
  sortOrder
`;

const SHARE_LINK_FIELDS = `
  id
  collectionId
  shareCode
  revoked
`;

export function getWorldCollections(
  worldId: string,
): Promise<CollectionRecord[]> {
  return postGraphQL<{ worldCollections: CollectionRecord[] }>(
    `
      query WorldCollections($worldId: UUID!) {
        worldCollections(worldId: $worldId) {
          ${COLLECTION_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldCollections);
}

export function getCollectionMembers(
  collectionId: string,
): Promise<CollectionMemberRecord[]> {
  return postGraphQL<{ collectionMembers: CollectionMemberRecord[] }>(
    `
      query CollectionMembers($collectionId: UUID!) {
        collectionMembers(collectionId: $collectionId) {
          ${MEMBER_FIELDS}
        }
      }
    `,
    { collectionId },
  ).then((data) => data.collectionMembers);
}

/** DM-of-the-world only, enforced server-side. */
export function createCollection(input: {
  worldId: string;
  name: string;
  description?: string | null;
}): Promise<CollectionRecord> {
  return postGraphQL<{ createCollection: CollectionRecord }>(
    `
      mutation CreateCollection($input: CreateCollectionInput!) {
        createCollection(input: $input) {
          ${COLLECTION_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createCollection);
}

export function updateCollection(input: {
  collectionId: string;
  name?: string | null;
  description?: string | null;
}): Promise<CollectionRecord> {
  return postGraphQL<{ updateCollection: CollectionRecord }>(
    `
      mutation UpdateCollection($input: UpdateCollectionInput!) {
        updateCollection(input: $input) {
          ${COLLECTION_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.updateCollection);
}

/**
 * FR-013: deletes the collection, never its artifacts.
 *
 * A collection is a list of references. Deleting one removes the list; every
 * scene, actor, item, lore entry and ability it named is untouched, and the
 * server has a test saying so.
 */
export function deleteCollection(collectionId: string): Promise<boolean> {
  return postGraphQL<{ deleteCollection: boolean }>(
    `
      mutation DeleteCollection($collectionId: UUID!) {
        deleteCollection(collectionId: $collectionId)
      }
    `,
    { collectionId },
  ).then((data) => data.deleteCollection);
}

/**
 * FR-001a/FR-001b: the server refuses a member it will not share and says
 * why — a GM-only ability, an artifact from another world, the hundredth-and-
 * first member. Callers should surface `GraphQLRequestError.message` verbatim
 * rather than replacing it with a generic failure, because the refusal
 * sentence is the only thing that tells the author what to do next.
 */
export function addCollectionMember(input: {
  collectionId: string;
  memberType: string;
  memberId: string;
}): Promise<CollectionMemberRecord> {
  return postGraphQL<{ addCollectionMember: CollectionMemberRecord }>(
    `
      mutation AddCollectionMember($input: AddCollectionMemberInput!) {
        addCollectionMember(input: $input) {
          ${MEMBER_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.addCollectionMember);
}

/** Removes the reference. The artifact itself is not touched (FR-013). */
export function removeCollectionMember(
  collectionId: string,
  memberId: string,
): Promise<boolean> {
  return postGraphQL<{ removeCollectionMember: boolean }>(
    `
      mutation RemoveCollectionMember($collectionId: UUID!, $memberId: UUID!) {
        removeCollectionMember(
          collectionId: $collectionId
          memberId: $memberId
        )
      }
    `,
    { collectionId, memberId },
  ).then((data) => data.removeCollectionMember);
}

/**
 * FR-010a: the active share link for a collection you own, or `null`.
 *
 * Not the enumeration FR-020 forbids — it takes one collection id the caller
 * already has authority over and returns that collection's own share. It
 * exists because without it FR-010's "the owner MUST be able to revoke" only
 * held inside the browser session that created the link: the code was shown
 * once, and closing the tab removed the ability to revoke it for good.
 */
export function getCollectionShareLink(
  collectionId: string,
): Promise<CollectionShareLinkRecord | null> {
  return postGraphQL<{
    collectionShareLink: CollectionShareLinkRecord | null;
  }>(
    `
      query CollectionShareLink($collectionId: UUID!) {
        collectionShareLink(collectionId: $collectionId) {
          ${SHARE_LINK_FIELDS}
        }
      }
    `,
    { collectionId },
  ).then((data) => data.collectionShareLink);
}

export function createCollectionShareLink(
  collectionId: string,
): Promise<CollectionShareLinkRecord> {
  return postGraphQL<{
    createCollectionShareLink: CollectionShareLinkRecord;
  }>(
    `
      mutation CreateCollectionShareLink($collectionId: UUID!) {
        createCollectionShareLink(collectionId: $collectionId) {
          ${SHARE_LINK_FIELDS}
        }
      }
    `,
    { collectionId },
  ).then((data) => data.createCollectionShareLink);
}

/**
 * FR-010/FR-011: a soft revoke.
 *
 * The row is flagged, never deleted — a deleted row cannot tell "revoked"
 * apart from "never existed". Copies already taken are **not** affected and
 * cannot be recalled; the interface must say so at the moment of revoking
 * (FR-011), not in a help page somewhere.
 */
export function revokeCollectionShareLink(shareId: string): Promise<boolean> {
  return postGraphQL<{ revokeCollectionShareLink: boolean }>(
    `
      mutation RevokeCollectionShareLink($shareId: UUID!) {
        revokeCollectionShareLink(shareId: $shareId)
      }
    `,
    { shareId },
  ).then((data) => data.revokeCollectionShareLink);
}

/**
 * FR-009a: readable **with no account at all**, which is why this one call
 * goes to the public endpoint.
 *
 * This was the divergence from the three shipped shares, which each required a
 * session. ADR-071 closed it: `sharedAbility`, `sharedItem` and `sharedActor`
 * now read anonymously through this same endpoint. Pointing any of the four at
 * `/api/graphql` would make its page require a login and quietly undo that.
 */
export function getSharedCollection(
  shareCode: string,
): Promise<SharedCollectionPreview> {
  return postGraphQL<{ sharedCollection: SharedCollectionPreview }>(
    `
      query SharedCollection($shareCode: String!) {
        sharedCollection(shareCode: $shareCode) {
          name
          description
          members {
            memberType
            name
          }
          countsByType {
            memberType
            count
          }
          withheldCount
        }
      }
    `,
    { shareCode },
    { endpoint: GRAPHQL_PUBLIC_ENDPOINT },
  ).then((data) => data.sharedCollection);
}

/** Reuses the world-agnostic query the actor/item/ability share pages use. */
export function getMyDmWorlds(): Promise<DmWorldSummary[]> {
  return postGraphQL<{ myDmWorlds: DmWorldSummary[] }>(
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

/**
 * FR-009b: **authenticated**, unlike the preview above.
 *
 * Viewing and copying are different acts with different requirements, and this
 * is exactly where they diverge — so this call goes to `/api/graphql` and a
 * signed-out visitor must be sent to sign in before reaching it.
 *
 * The copy is a deep copy into a world the caller runs: every row arrives
 * owned by them, with no live link back to the source. The receipt says what
 * arrived and what was lost on the way.
 */
export function copySharedCollectionToWorld(
  shareCode: string,
  destinationWorldId: string,
): Promise<CopyReceipt> {
  return postGraphQL<{ copySharedCollectionToWorld: CopyReceipt }>(
    `
      mutation CopySharedCollectionToWorld(
        $shareCode: String!
        $destinationWorldId: UUID!
      ) {
        copySharedCollectionToWorld(
          shareCode: $shareCode
          destinationWorldId: $destinationWorldId
        ) {
          created {
            memberType
            id
            name
          }
          fidelityNotes
        }
      }
    `,
    { shareCode, destinationWorldId },
  ).then((data) => data.copySharedCollectionToWorld);
}
