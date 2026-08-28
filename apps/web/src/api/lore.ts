import { postGraphQL, postGraphQLMultipart } from "@/api/graphqlClient";
import type {
  LoreEntryRecord,
  LoreImageAssetRecord,
  LoreLinkTargetRecord,
  LorePermissionRecord,
  LoreRevisionRecord,
} from "@/types/lore";
import type { ActorPermissionLevel } from "@/types/actor";

const LORE_ENTRY_FIELDS = `
  id
  worldId
  title
  slug
  content
  renderedHtml
  currentRevisionId
  myPermissionLevel
  moderated
  moderationCaseId
  createdBy
  createdAt
  updatedAt
  linkedFrom {
    id
    title
    slug
  }
`;

/**
 * uploadLoreImage sends `file` as an `Upload!` scalar, which requires the
 * GraphQL multipart request spec rather than a plain JSON body — mirrors
 * `api/assets.ts`'s `postGraphQLMultipart` for `uploadCanvasImage`.
 */

/** FR-001: every lore entry in the world, visible to any member. */
export function getWorldLoreEntries(
  worldId: string,
): Promise<LoreEntryRecord[]> {
  return postGraphQL<{ worldLoreEntries: LoreEntryRecord[] }>(
    `
      query WorldLoreEntries($worldId: UUID!) {
        worldLoreEntries(worldId: $worldId) {
          ${LORE_ENTRY_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldLoreEntries);
}

/** FR-014: the canonical detail-page lookup — `null` for a stale/missing slug. */
export function getLoreEntry(
  worldId: string,
  slug: string,
): Promise<LoreEntryRecord | null> {
  return postGraphQL<{ loreEntry: LoreEntryRecord | null }>(
    `
      query LoreEntry($worldId: UUID!, $slug: String!) {
        loreEntry(worldId: $worldId, slug: $slug) {
          ${LORE_ENTRY_FIELDS}
        }
      }
    `,
    { worldId, slug },
  ).then((data) => data.loreEntry);
}

type CreateLoreEntryInput = {
  worldId: string;
  title: string;
  content?: string;
};

/** DM-only (FR-002). */
export function createLoreEntry(
  input: CreateLoreEntryInput,
): Promise<LoreEntryRecord> {
  return postGraphQL<{ createLoreEntry: LoreEntryRecord }>(
    `
      mutation CreateLoreEntry($input: CreateLoreEntryInput!) {
        createLoreEntry(input: $input) {
          ${LORE_ENTRY_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createLoreEntry);
}

type UpdateLoreEntryInput = {
  loreEntryId: string;
  title?: string;
  content?: string;
  /** REQUIRED whenever `content` is provided (FR-019). */
  expectedCurrentRevisionId?: string | null;
};

/**
 * Requires Editor or Owner effective permission (FR-003). When `content`
 * is provided, `expectedCurrentRevisionId` must be the revision id the
 * caller was editing against — a mismatch (someone else saved first) is
 * rejected outright with a "CONFLICT"-coded error (FR-019); the caller
 * must reload and retry, not silently overwrite.
 */
export function updateLoreEntry(
  input: UpdateLoreEntryInput,
): Promise<LoreEntryRecord> {
  return postGraphQL<{ updateLoreEntry: LoreEntryRecord }>(
    `
      mutation UpdateLoreEntry($input: UpdateLoreEntryInput!) {
        updateLoreEntry(input: $input) {
          ${LORE_ENTRY_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.updateLoreEntry);
}

/** Owner-level (entry-level, not DM-only) permission required (FR-021). */
export function deleteLoreEntry(loreEntryId: string): Promise<boolean> {
  return postGraphQL<{ deleteLoreEntry: boolean }>(
    `
      mutation DeleteLoreEntry($loreEntryId: UUID!) {
        deleteLoreEntry(loreEntryId: $loreEntryId)
      }
    `,
    { loreEntryId },
  ).then((data) => data.deleteLoreEntry);
}

/** FR-007a: autocomplete candidates for the editor's `[[`-trigger popover. */
export function getLoreLinkTargets(
  worldId: string,
  prefix: string,
): Promise<LoreLinkTargetRecord[]> {
  return postGraphQL<{ loreLinkTargets: LoreLinkTargetRecord[] }>(
    `
      query LoreLinkTargets($worldId: UUID!, $prefix: String!) {
        loreLinkTargets(worldId: $worldId, prefix: $prefix) {
          id
          title
          kind
        }
      }
    `,
    { worldId, prefix },
  ).then((data) => data.loreLinkTargets);
}

/** FR-017: full revision history, newest first. */
export function getLoreEntryRevisions(
  loreEntryId: string,
): Promise<LoreRevisionRecord[]> {
  return postGraphQL<{ loreEntryRevisions: LoreRevisionRecord[] }>(
    `
      query LoreEntryRevisions($loreEntryId: UUID!) {
        loreEntryRevisions(loreEntryId: $loreEntryId) {
          id
          loreEntryId
          contentMarkdown
          renderedHtml
          authorId
          restoredFromRevisionId
          createdAt
        }
      }
    `,
    { loreEntryId },
  ).then((data) => data.loreEntryRevisions);
}

/** FR-018: appends a new revision recording the restore. */
export function restoreLoreRevision(
  revisionId: string,
): Promise<LoreEntryRecord> {
  return postGraphQL<{ restoreLoreRevision: LoreEntryRecord }>(
    `
      mutation RestoreLoreRevision($revisionId: UUID!) {
        restoreLoreRevision(revisionId: $revisionId) {
          ${LORE_ENTRY_FIELDS}
        }
      }
    `,
    { revisionId },
  ).then((data) => data.restoreLoreRevision);
}

/** DM-only (FR-003). Returns only explicit ownership-block rows. */
export function getLoreEntryPermissions(
  loreEntryId: string,
): Promise<LorePermissionRecord[]> {
  return postGraphQL<{ loreEntryPermissions: LorePermissionRecord[] }>(
    `
      query LoreEntryPermissions($loreEntryId: UUID!) {
        loreEntryPermissions(loreEntryId: $loreEntryId) {
          loreEntryId
          userId
          level
          updatedAt
        }
      }
    `,
    { loreEntryId },
  ).then((data) => data.loreEntryPermissions);
}

/** DM-only. UPSERT on (loreEntryId, userId). */
export function setLorePermission(
  loreEntryId: string,
  userId: string,
  level: ActorPermissionLevel,
): Promise<LorePermissionRecord> {
  return postGraphQL<{ setLorePermission: LorePermissionRecord }>(
    `
      mutation SetLorePermission($input: SetLorePermissionInput!) {
        setLorePermission(input: $input) {
          loreEntryId
          userId
          level
          updatedAt
        }
      }
    `,
    { input: { loreEntryId, userId, level } },
  ).then((data) => data.setLorePermission);
}

/** DM-only. Idempotent — reverts the member to default Viewer. */
export function removeLorePermission(
  loreEntryId: string,
  userId: string,
): Promise<boolean> {
  return postGraphQL<{ removeLorePermission: boolean }>(
    `
      mutation RemoveLorePermission($loreEntryId: UUID!, $userId: UUID!) {
        removeLorePermission(loreEntryId: $loreEntryId, userId: $userId)
      }
    `,
    { loreEntryId, userId },
  ).then((data) => data.removeLorePermission);
}

/**
 * FR-008/009/010: uploads `file` (a pasted/dropped image) to be
 * processed server-side (normalized rendition + thumbnail, both WebP)
 * and stored. Throws with a message suitable for direct display.
 */
export async function uploadLoreImage(
  loreEntryId: string,
  file: Blob,
): Promise<LoreImageAssetRecord> {
  const mutation = `
    mutation UploadLoreImage($loreEntryId: UUID!, $file: Upload!) {
      uploadLoreImage(loreEntryId: $loreEntryId, file: $file) {
        id
        loreEntryId
        url
        thumbnailUrl
        byteSize
        createdAt
      }
    }
  `;
  const data = await postGraphQLMultipart<{
    uploadLoreImage: LoreImageAssetRecord;
  }>(mutation, { loreEntryId }, file, "file");
  return data.uploadLoreImage;
}
