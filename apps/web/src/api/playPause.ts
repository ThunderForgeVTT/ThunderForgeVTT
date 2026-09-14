import { GraphQLRequestError, postGraphQL } from "@/api/graphqlClient";
import { WORLD_PLAY_PAUSED } from "@/api/playPauseSignal";

/**
 * Spec 051: pausing a world's play, as the web app asks for it
 * (contracts/graphql.md).
 *
 * Two audiences, and the split is in the documents, not in what a page
 * chooses to render:
 *
 * - **Members** read `worldPlayState`, whose type has no field a reason could
 *   be put in. The notice cannot show grounds because it cannot ask for them
 *   (FR-011).
 * - **Operators** read and write the record, grounds and triggers included,
 *   through `admin_user`-guarded fields.
 */

export { WORLD_PLAY_PAUSED };

/**
 * Whether an error is a refusal because an operator paused the world.
 *
 * On the code, never the message, which is written for people and may be
 * reworded. Accepts anything a `catch` can hold.
 */
export function isPlayPaused(error: unknown): boolean {
  return (
    error instanceof GraphQLRequestError && error.hasCode(WORLD_PLAY_PAUSED)
  );
}

// ----- Member-facing ------------------------------------------------------

/** One stretch of paused play: when, and nothing else. */
export interface PlayPauseSpan {
  pausedAt: string;
  liftedAt: string | null;
}

export interface WorldPlayState {
  paused: boolean;
  /** `null` when not paused. */
  pausedAt: string | null;
  /** Newest first. */
  history: PlayPauseSpan[];
}

/**
 * Whether a world's play is paused, and since when. Answers while paused, to
 * members only.
 */
export function getWorldPlayState(worldId: string): Promise<WorldPlayState> {
  return postGraphQL<{ worldPlayState: WorldPlayState }>(
    `
      query WorldPlayState($worldId: UUID!) {
        worldPlayState(worldId: $worldId) {
          paused
          pausedAt
          history { pausedAt liftedAt }
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldPlayState);
}

// ----- Operator-facing ----------------------------------------------------

export type PauseTriggerKind = "TAKEDOWN" | "OPERATOR" | "ABUSE_REPORT";
export type PauseRequestState = "PENDING" | "APPROVED" | "DECLINED";
export type PauseDecision = "APPROVE" | "DECLINE";

export interface OperatorName {
  id: string;
  name: string;
}

export interface PauseTrigger {
  kind: PauseTriggerKind;
  moderationActionId: string | null;
  /** The moderation case the action belongs to; cases open by this id. */
  caseId: string | null;
  entityType: string | null;
  entityId: string | null;
  note: string | null;
  recordedAt: string;
}

export interface PlayPause {
  id: string;
  worldId: string;
  /** The name when it was paused, which outlives the world. */
  worldName: string;
  worldExists: boolean;
  pausedBy: OperatorName;
  pausedAt: string;
  grounds: string;
  requestId: string | null;
  triggers: PauseTrigger[];
  liftedBy: OperatorName | null;
  liftedAt: string | null;
  liftGrounds: string | null;
  playedNow: boolean;
}

export interface PauseRequest {
  id: string;
  worldId: string;
  worldName: string;
  worldExists: boolean;
  raisedAt: string;
  state: PauseRequestState;
  playedNow: boolean;
  triggers: PauseTrigger[];
  decidedBy: OperatorName | null;
  decidedAt: string | null;
  decisionNote: string | null;
}

export interface PauseCandidateWorld {
  id: string;
  name: string;
  ownerName: string;
  playedNow: boolean;
  paused: boolean;
}

/** A page of the record, newest first; `nextCursor` is `null` on the last. */
export interface Page<T> {
  nodes: T[];
  nextCursor: string | null;
}

export interface PauseOutcome {
  pause: PlayPause;
  /** An active pause existed; these grounds were added to it as a trigger. */
  alreadyPaused: boolean;
}

export interface PauseDecisionOutcome {
  request: PauseRequest;
  /** False when another operator decided first; `request` carries theirs. */
  decidedHere: boolean;
  pause: PlayPause | null;
}

const TRIGGER_FIELDS = `
  kind
  moderationActionId
  caseId
  entityType
  entityId
  note
  recordedAt
`;

const PLAY_PAUSE_FIELDS = `
  id
  worldId
  worldName
  worldExists
  pausedBy { id name }
  pausedAt
  grounds
  requestId
  triggers { ${TRIGGER_FIELDS} }
  liftedBy { id name }
  liftedAt
  liftGrounds
  playedNow
`;

const PAUSE_REQUEST_FIELDS = `
  id
  worldId
  worldName
  worldExists
  raisedAt
  state
  playedNow
  triggers { ${TRIGGER_FIELDS} }
  decidedBy { id name }
  decidedAt
  decisionNote
`;

/** Worlds whose name contains `search`, or whose id it is. */
export function searchPauseCandidates(
  search: string,
  first = 20,
): Promise<PauseCandidateWorld[]> {
  return postGraphQL<{ playPauseCandidates: PauseCandidateWorld[] }>(
    `
      query PlayPauseCandidates($search: String!, $first: Int) {
        playPauseCandidates(search: $search, first: $first) {
          id
          name
          ownerName
          playedNow
          paused
        }
      }
    `,
    { search, first },
  ).then((data) => data.playPauseCandidates);
}

export function listPlayPauses(
  options: {
    active?: boolean;
    worldId?: string;
    first?: number;
    after?: string;
  } = {},
): Promise<Page<PlayPause>> {
  return postGraphQL<{ playPauses: Page<PlayPause> }>(
    `
      query PlayPauses($active: Boolean, $worldId: UUID, $first: Int, $after: String) {
        playPauses(active: $active, worldId: $worldId, first: $first, after: $after) {
          nodes { ${PLAY_PAUSE_FIELDS} }
          nextCursor
        }
      }
    `,
    {
      active: options.active ?? null,
      worldId: options.worldId ?? null,
      first: options.first ?? null,
      after: options.after ?? null,
    },
  ).then((data) => data.playPauses);
}

export function listPlayPauseRequests(
  options: {
    state?: PauseRequestState;
    first?: number;
    after?: string;
  } = {},
): Promise<Page<PauseRequest>> {
  return postGraphQL<{ playPauseRequests: Page<PauseRequest> }>(
    `
      query PlayPauseRequests($state: PauseRequestState, $first: Int, $after: String) {
        playPauseRequests(state: $state, first: $first, after: $after) {
          nodes { ${PAUSE_REQUEST_FIELDS} }
          nextCursor
        }
      }
    `,
    {
      state: options.state ?? "PENDING",
      first: options.first ?? null,
      after: options.after ?? null,
    },
  ).then((data) => data.playPauseRequests);
}

/** Pause a world's play now. Grounds are required and recorded. */
export function pauseWorldPlay(
  worldId: string,
  grounds: string,
): Promise<PauseOutcome> {
  return postGraphQL<{ pauseWorldPlay: PauseOutcome }>(
    `
      mutation PauseWorldPlay($worldId: UUID!, $grounds: String!) {
        pauseWorldPlay(worldId: $worldId, grounds: $grounds) {
          pause { ${PLAY_PAUSE_FIELDS} }
          alreadyPaused
        }
      }
    `,
    { worldId, grounds },
  ).then((data) => data.pauseWorldPlay);
}

export function decidePlayPauseRequest(
  requestId: string,
  decision: PauseDecision,
  note: string,
): Promise<PauseDecisionOutcome> {
  return postGraphQL<{ decidePlayPauseRequest: PauseDecisionOutcome }>(
    `
      mutation DecidePlayPauseRequest($requestId: UUID!, $decision: PauseDecision!, $note: String!) {
        decidePlayPauseRequest(requestId: $requestId, decision: $decision, note: $note) {
          request { ${PAUSE_REQUEST_FIELDS} }
          decidedHere
          pause { ${PLAY_PAUSE_FIELDS} }
        }
      }
    `,
    { requestId, decision, note },
  ).then((data) => data.decidePlayPauseRequest);
}

export function liftWorldPlayPause(
  pauseId: string,
  grounds: string,
): Promise<PlayPause> {
  return postGraphQL<{ liftWorldPlayPause: PlayPause }>(
    `
      mutation LiftWorldPlayPause($pauseId: UUID!, $grounds: String!) {
        liftWorldPlayPause(pauseId: $pauseId, grounds: $grounds) {
          ${PLAY_PAUSE_FIELDS}
        }
      }
    `,
    { pauseId, grounds },
  ).then((data) => data.liftWorldPlayPause);
}
