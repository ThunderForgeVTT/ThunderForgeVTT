import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 034 (T034): the client for the lore-synchronisation GraphQL surface,
 * per `specs/034-lore-git-sync/contracts/graphql-lore-sync.md`.
 *
 * Two absences in this file are load-bearing rather than accidental:
 *
 * - **Nothing here writes a lore entry.** The first delivery mirrors outward
 *   only, and the property that makes it safe by construction is that no
 *   operation exists to bring a repository's contents back in.
 * - **No credential appears at any depth**, and neither does `installationRef`
 *   nor `hostKind`. FR-035 forbids returning credentials to a client, and
 *   FR-004c keeps the host's identity at the grant boundary — a client able to
 *   read the host is a client able to branch on it, which is the thing FR-004
 *   exists to prevent.
 */

/**
 * FR-029's three words, plus the enforcement state FR-041c requires to be
 * distinguishable from them.
 *
 * `DEACTIVATED` is deliberately not folded into `NEEDS_ATTENTION`: a Game
 * Master told to "check the connection" for something only an administrator
 * can lift will keep trying to fix it.
 */
export type LoreSyncState =
  | "WORKING"
  | "NEEDS_ATTENTION"
  | "NEVER_CONFIGURED"
  | "DEACTIVATED";

/** Something the mirror could not represent, recorded per occurrence so
 * SC-008's losses can be enumerated rather than discovered. */
export interface LoreFidelityNote {
  id: string;
  kind: string;
  detail: string;
  loreEntryId?: string | null;
  firstSeenAt: string;
  lastSeenAt: string;
}

export interface LoreRepositoryConnection {
  id: string;
  worldId: string;
  repositoryRef: string;
  branch: string;
  directory: string;
  incomingEnabled: boolean;
  state: LoreSyncState;
  /** Plain language, naming the remedy (FR-029). Never a raw host error. */
  stateReason?: string | null;
  /** FR-038's gate. Null means synchronisation has never been allowed to
   * begin, and the background task will not pick this connection up. */
  noticeAcknowledgedAt?: string | null;
  lastSyncedAt?: string | null;

  /**
   * FR-040a: what was **observed** at the last run, not a guarantee. Null
   * before the first observation — which is not the same as private, and must
   * never be shown as though it were (FR-037a).
   */
  repositoryIsPublic?: boolean | null;
  /** When that observation was made, so a stale one can be shown as stale. */
  visibilityCheckedAt?: string | null;

  fidelityNotes: LoreFidelityNote[];
}

export interface LoreSyncRun {
  id: string;
  startedAt: string;
  finishedAt?: string | null;
  outcome?: string | null;
  entriesWritten: number;
  /** In terms a Game Master can act on. */
  failureReason?: string | null;
  attempt: number;
}

/**
 * FR-036b/FR-036c. Queried **before** any connection affordance is rendered,
 * never after one fails: an instance whose operator has registered no
 * application must not present the feature as broken, and a Game Master must
 * never be shown a flow that cannot complete.
 */
export interface RepositoryIntegrationStatus {
  configured: boolean;
  /** What the operator must do. Never a stack trace. */
  operatorGuidance?: string | null;
}

/** One permission the grant asks for, in the user's words. FR-036 requires
 * these to be shown *before* the user grants anything, and FR-036e requires
 * the issue-opening permission to arrive carrying its own reason. */
export interface GrantedPermission {
  id: string;
  summary: string;
  reason: string;
}

/** Where the user is sent to grant access. Opaque to everything but the
 * consent step — the permissions travel with the URL precisely so a consent
 * screen cannot be rendered without them. */
export interface ConnectionGrantHandoff {
  url: string;
  permissions: GrantedPermission[];
}

export interface CompleteConnectionInput {
  worldId: string;
  /** The host's opaque return payload, echoed back unread by this client. */
  grantRef: string;
  state: string;
  directory?: string | null;
  branch?: string | null;
}

/** FR-031's two choices. There is deliberately no third that reconciles
 * silently, because reconciling would mean merging prose (FR-024). */
export type DivergenceResolution = "OVERWRITE_REMOTE" | "ABANDON_CONNECTION";

export interface ResolveDivergenceInput {
  worldId: string;
  resolution: DivergenceResolution;
}

const FIDELITY_NOTE_FIELDS = `
  id
  kind
  detail
  loreEntryId
  firstSeenAt
  lastSeenAt
`;

/** One list, so every operation returning a connection agrees about it. */
const CONNECTION_FIELDS = `
  id
  worldId
  repositoryRef
  branch
  directory
  incomingEnabled
  state
  stateReason
  noticeAcknowledgedAt
  lastSyncedAt
  repositoryIsPublic
  visibilityCheckedAt
  fidelityNotes {
    ${FIDELITY_NOTE_FIELDS}
  }
`;

const RUN_FIELDS = `
  id
  startedAt
  finishedAt
  outcome
  entriesWritten
  failureReason
  attempt
`;

/** The world's connection, or null. Readable by any world member: hiding a
 * broken connection from the players who can see the lore it mirrors helps
 * nobody. */
export function getLoreRepositoryConnection(
  worldId: string,
): Promise<LoreRepositoryConnection | null> {
  return postGraphQL<{
    loreRepositoryConnection: LoreRepositoryConnection | null;
  }>(
    `
      query LoreRepositoryConnection($worldId: UUID!) {
        loreRepositoryConnection(worldId: $worldId) {
          ${CONNECTION_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.loreRepositoryConnection);
}

/** Recent attempts, newest first. Owner-level (FR-002): a run's failure
 * reason can name repository details a player has no business seeing. */
export function getLoreSyncRuns(
  worldId: string,
  limit?: number,
): Promise<LoreSyncRun[]> {
  return postGraphQL<{ loreSyncRuns: LoreSyncRun[] }>(
    `
      query LoreSyncRuns($worldId: UUID!, $limit: Int) {
        loreSyncRuns(worldId: $worldId, limit: $limit) {
          ${RUN_FIELDS}
        }
      }
    `,
    { worldId, limit: limit ?? null },
  ).then((data) => data.loreSyncRuns);
}

/**
 * Whether this instance can arrange a repository grant at all.
 *
 * Instance-wide rather than per-world, and answered without a world id,
 * because the answer is a property of the deployment's configuration — it is
 * the same for every world on the instance and is the first thing the
 * connection surface asks.
 */
export function getInstanceRepositoryIntegration(): Promise<RepositoryIntegrationStatus> {
  return postGraphQL<{
    instanceRepositoryIntegration: RepositoryIntegrationStatus;
  }>(`
    query InstanceRepositoryIntegration {
      instanceRepositoryIntegration {
        configured
        operatorGuidance
      }
    }
  `).then((data) => data.instanceRepositoryIntegration);
}

/** Starts the grant. The one place a host-specific concept legitimately
 * appears (FR-004b) — and it is opaque here: this client forwards the URL and
 * renders the permissions, and reads nothing else about it. */
export function beginLoreRepositoryConnection(
  worldId: string,
): Promise<ConnectionGrantHandoff> {
  return postGraphQL<{
    beginLoreRepositoryConnection: ConnectionGrantHandoff;
  }>(
    `
      mutation BeginLoreRepositoryConnection($worldId: UUID!) {
        beginLoreRepositoryConnection(worldId: $worldId) {
          url
          permissions {
            id
            summary
            reason
          }
        }
      }
    `,
    { worldId },
  ).then((data) => data.beginLoreRepositoryConnection);
}

/** Finishes the grant and creates the row. Refuses, creating nothing, when
 * the world already has a connection (FR-001), when the repository directory
 * is claimed by another world (FR-033), or when the grant covers more than
 * the one repository being connected (FR-036a). */
export function completeLoreRepositoryConnection(
  input: CompleteConnectionInput,
): Promise<LoreRepositoryConnection> {
  return postGraphQL<{
    completeLoreRepositoryConnection: LoreRepositoryConnection;
  }>(
    `
      mutation CompleteLoreRepositoryConnection($input: CompleteConnectionInput!) {
        completeLoreRepositoryConnection(input: $input) {
          ${CONNECTION_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.completeLoreRepositoryConnection);
}

/**
 * Records FR-038's acknowledgement, after which the background task may pick
 * the connection up.
 *
 * The notice's wording is a client concern; the *gate* is not, because a
 * client-side-only gate is not a gate. Nothing here should treat a local
 * checkbox as having started synchronisation — only this mutation's success
 * does.
 */
export function acknowledgeLoreSyncNotice(
  worldId: string,
): Promise<LoreRepositoryConnection> {
  return postGraphQL<{
    acknowledgeLoreSyncNotice: LoreRepositoryConnection;
  }>(
    `
      mutation AcknowledgeLoreSyncNotice($worldId: UUID!) {
        acknowledgeLoreSyncNotice(worldId: $worldId) {
          ${CONNECTION_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.acknowledgeLoreSyncNotice);
}

/** Deletes the connection and its working clone. The world's lore and the
 * repository's contents are both left entirely intact (FR-005) — a removal is
 * the platform forgetting, not a retraction. */
export function removeLoreRepositoryConnection(
  worldId: string,
): Promise<boolean> {
  return postGraphQL<{ removeLoreRepositoryConnection: boolean }>(
    `
      mutation RemoveLoreRepositoryConnection($worldId: UUID!) {
        removeLoreRepositoryConnection(worldId: $worldId)
      }
    `,
    { worldId },
  ).then((data) => data.removeLoreRepositoryConnection);
}

/** Asks for a run now rather than at the next scheduled pass. Rate-limited
 * server-side: this is a convenience for someone who has just fixed a
 * credential, not a way to drive synchronisation by hand. */
export function retryLoreSync(worldId: string): Promise<LoreSyncRun> {
  return postGraphQL<{ retryLoreSync: LoreSyncRun }>(
    `
      mutation RetryLoreSync($worldId: UUID!) {
        retryLoreSync(worldId: $worldId) {
          ${RUN_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.retryLoreSync);
}

/** FR-031: an explicit choice between overwriting the divergent remote and
 * abandoning the connection, required before the platform writes again. */
export function resolveLoreSyncDivergence(
  input: ResolveDivergenceInput,
): Promise<LoreSyncRun> {
  return postGraphQL<{ resolveLoreSyncDivergence: LoreSyncRun }>(
    `
      mutation ResolveLoreSyncDivergence($input: ResolveDivergenceInput!) {
        resolveLoreSyncDivergence(input: $input) {
          ${RUN_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.resolveLoreSyncDivergence);
}

/**
 * FR-024/FR-026/FR-027's three shapes, kept distinct rather than collapsed into
 * a single "change": what a reviewer must decide differs per kind — an update
 * replaces text, a deletion removes an entry, and a new entry creates one — and
 * a UI that could not tell them apart would have to guess which question to ask.
 */
export type LoreIncomingKind = "UPDATE" | "NEW_ENTRY" | "DELETION";

/**
 * One proposal found in the repository, waiting on a person (FR-023). Nothing
 * here has touched the world's lore: a pending change is a description of what
 * *would* happen, and it stays that way until accepted.
 *
 * Both sides of the text travel together — `incomingBody` and `currentBody` —
 * so a conflict can be shown as two whole texts. Deliberately no merged or
 * unified field exists anywhere in this type: FR-024 forbids merging prose, and
 * the way to keep a client from rendering a merge is to never hand it one.
 */
export interface LorePendingChange {
  id: string;
  kind: LoreIncomingKind;
  /** Where in the repository it came from. A label, never an identity. */
  repositoryPath: string;
  proposedTitle?: string | null;
  incomingBody?: string | null;
  /** FR-024's flag: the entry also changed in the app since the last run. */
  alsoChangedInApp: boolean;
  detectedAt: string;
  /** Null for a NEW_ENTRY — a file with no recognised identifier is matched to
   * nothing (FR-027), not even by path or title. */
  loreEntryId?: string | null;
  currentTitle?: string | null;
  currentBody?: string | null;
}

const PENDING_CHANGE_FIELDS = `
  id
  kind
  repositoryPath
  proposedTitle
  incomingBody
  alsoChangedInApp
  detectedAt
  loreEntryId
  currentTitle
  currentBody
`;

/** What the repository proposes, and nothing more: this query reports, it never
 * applies (FR-023). An empty list is the ordinary case. */
export function getLorePendingChanges(
  worldId: string,
): Promise<LorePendingChange[]> {
  return postGraphQL<{ lorePendingChanges: LorePendingChange[] }>(
    `
      query LorePendingChanges($worldId: UUID!) {
        lorePendingChanges(worldId: $worldId) {
          ${PENDING_CHANGE_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.lorePendingChanges);
}

/**
 * Applies one change, as one explicit act by one person with authority
 * (FR-023). Per change rather than in bulk: an "accept all" would let a
 * conflict (FR-024) and a deletion (FR-026) be decided by a single click that
 * asked neither question.
 *
 * The server records the result as an ordinary revision attributed to the
 * accepting user (FR-025), which is why this returns only success — the entry's
 * new state is read back through the ordinary lore surface, not this one.
 */
export function acceptLoreIncomingChange(
  worldId: string,
  changeId: string,
): Promise<boolean> {
  return postGraphQL<{ acceptLoreIncomingChange: boolean }>(
    `
      mutation AcceptLoreIncomingChange($worldId: UUID!, $changeId: UUID!) {
        acceptLoreIncomingChange(worldId: $worldId, changeId: $changeId)
      }
    `,
    { worldId, changeId },
  ).then((data) => data.acceptLoreIncomingChange);
}

/** Refuses one change. The world's lore is left exactly as it was, and a
 * declined deletion is undone in the repository on the next synchronisation
 * (FR-026) — declining is the safe answer, not a destructive one. */
export function declineLoreIncomingChange(
  worldId: string,
  changeId: string,
): Promise<boolean> {
  return postGraphQL<{ declineLoreIncomingChange: boolean }>(
    `
      mutation DeclineLoreIncomingChange($worldId: UUID!, $changeId: UUID!) {
        declineLoreIncomingChange(worldId: $worldId, changeId: $changeId)
      }
    `,
    { worldId, changeId },
  ).then((data) => data.declineLoreIncomingChange);
}
