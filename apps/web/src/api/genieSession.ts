import { withCsrf } from "@/api/auth";

/**
 * Genie session-loop GraphQL client — spec 018 User Story 7. The backend
 * (src/server/src/graphql/queries/genie_session.rs,
 * src/server/src/graphql/mutations_genie_session.rs) has always had full
 * query/mutation support for this; nothing in apps/web ever called it.
 * apps/web/src/engine/world/sync/genieSession.ts (inbound world_events
 * NOTIFY dispatch) intentionally doesn't do this fetching itself — this
 * module is the "host page" client it expects to exist.
 */

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

export type GenieSessionStatus = "ACTIVE" | "WON" | "LOST";

export interface GeniePuzzleClockRecord {
  id: string;
  sessionId: string;
  label: string;
  segmentsCurrent: number;
  segmentsMax: number;
  resolvedAt: string | null;
}

export interface GenieSessionRecord {
  id: string;
  worldId: string;
  wishesRemaining: number;
  doomClockCurrent: number;
  doomClockMax: number;
  status: GenieSessionStatus;
  puzzleClocks: GeniePuzzleClockRecord[];
}

export interface GenieResourceHoldingRecord {
  actorId: string;
  resourceType: string;
  quantity: number;
}

export interface GenieTradeProposalRecord {
  id: string;
  sessionId: string;
  fromActorId: string;
  fromResourceType: string;
  fromQuantity: number;
  toActorId: string;
  toResourceType: string;
  toQuantity: number;
  status: string;
}

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({ query, variables }),
  });

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }
  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }
  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }
  return payload.data;
}

const SESSION_FIELDS = `
  id
  worldId
  wishesRemaining
  doomClockCurrent
  doomClockMax
  status
  puzzleClocks {
    id
    sessionId
    label
    segmentsCurrent
    segmentsMax
    resolvedAt
  }
`;

const GENIE_SESSION_QUERY = `
  query GenieSession($worldId: UUID!) {
    genieSession(worldId: $worldId) {
      ${SESSION_FIELDS}
    }
  }
`;

/** Returns the world's active Genie session, or `null` if none has been
 * started yet. Readable by any world member, not just the GM. */
export async function fetchGenieSession(worldId: string): Promise<GenieSessionRecord | null> {
  const data = await postGraphQL<{ genieSession: GenieSessionRecord | null }>(
    GENIE_SESSION_QUERY,
    { worldId },
  );
  return data.genieSession;
}

const GENIE_RESOURCE_HOLDINGS_QUERY = `
  query GenieResourceHoldings($sessionId: UUID!, $actorId: UUID!) {
    genieResourceHoldings(sessionId: $sessionId, actorId: $actorId) {
      actorId
      resourceType
      quantity
    }
  }
`;

export async function fetchGenieResourceHoldings(
  sessionId: string,
  actorId: string,
): Promise<GenieResourceHoldingRecord[]> {
  const data = await postGraphQL<{ genieResourceHoldings: GenieResourceHoldingRecord[] }>(
    GENIE_RESOURCE_HOLDINGS_QUERY,
    { sessionId, actorId },
  );
  return data.genieResourceHoldings;
}

const GENIE_TRADE_PROPOSALS_QUERY = `
  query GenieTradeProposals($actorId: UUID!) {
    genieTradeProposals(actorId: $actorId) {
      id
      sessionId
      fromActorId
      fromResourceType
      fromQuantity
      toActorId
      toResourceType
      toQuantity
      status
    }
  }
`;

/** Pending trade proposals naming `actorId` as the recipient. Caller must
 * control `actorId` (server-enforced). */
export async function fetchGenieTradeProposals(
  actorId: string,
): Promise<GenieTradeProposalRecord[]> {
  const data = await postGraphQL<{ genieTradeProposals: GenieTradeProposalRecord[] }>(
    GENIE_TRADE_PROPOSALS_QUERY,
    { actorId },
  );
  return data.genieTradeProposals;
}

const START_GENIE_SESSION_MUTATION = `
  mutation StartGenieSession($input: StartGenieSessionInput!) {
    startGenieSession(input: $input) {
      ${SESSION_FIELDS}
    }
  }
`;

/** GM-only (server-enforced): starts a new Genie session for the world. */
export async function startGenieSession(
  worldId: string,
  doomClockMax: number,
): Promise<GenieSessionRecord> {
  const data = await postGraphQL<{ startGenieSession: GenieSessionRecord }>(
    START_GENIE_SESSION_MUTATION,
    { input: { worldId, doomClockMax } },
  );
  return data.startGenieSession;
}

const SPEND_WISH_MUTATION = `
  mutation SpendWish($sessionId: UUID!, $narrativeEffect: String!) {
    spendWish(sessionId: $sessionId, narrativeEffect: $narrativeEffect) {
      ${SESSION_FIELDS}
    }
  }
`;

/** GM-only (server-enforced): spends one wish from the pool. */
export async function spendWish(
  sessionId: string,
  narrativeEffect: string,
): Promise<GenieSessionRecord> {
  const data = await postGraphQL<{ spendWish: GenieSessionRecord }>(SPEND_WISH_MUTATION, {
    sessionId,
    narrativeEffect,
  });
  return data.spendWish;
}

const ADVANCE_DOOM_CLOCK_MUTATION = `
  mutation AdvanceDoomClock($sessionId: UUID!, $delta: Int!) {
    advanceDoomClock(sessionId: $sessionId, delta: $delta) {
      ${SESSION_FIELDS}
    }
  }
`;

/** GM-only (server-enforced). */
export async function advanceDoomClock(
  sessionId: string,
  delta: number,
): Promise<GenieSessionRecord> {
  const data = await postGraphQL<{ advanceDoomClock: GenieSessionRecord }>(
    ADVANCE_DOOM_CLOCK_MUTATION,
    { sessionId, delta },
  );
  return data.advanceDoomClock;
}

const CREATE_PUZZLE_CLOCK_MUTATION = `
  mutation CreatePuzzleClock($sessionId: UUID!, $label: String!, $segmentsMax: Int!) {
    createPuzzleClock(sessionId: $sessionId, label: $label, segmentsMax: $segmentsMax) {
      id
      sessionId
      label
      segmentsCurrent
      segmentsMax
      resolvedAt
    }
  }
`;

/** GM-only (server-enforced). */
export async function createPuzzleClock(
  sessionId: string,
  label: string,
  segmentsMax: number,
): Promise<GeniePuzzleClockRecord> {
  const data = await postGraphQL<{ createPuzzleClock: GeniePuzzleClockRecord }>(
    CREATE_PUZZLE_CLOCK_MUTATION,
    { sessionId, label, segmentsMax },
  );
  return data.createPuzzleClock;
}

const ADVANCE_PUZZLE_CLOCK_MUTATION = `
  mutation AdvancePuzzleClock($clockId: UUID!, $delta: Int!) {
    advancePuzzleClock(clockId: $clockId, delta: $delta) {
      id
      sessionId
      label
      segmentsCurrent
      segmentsMax
      resolvedAt
    }
  }
`;

/** GM-only (server-enforced). */
export async function advancePuzzleClock(
  clockId: string,
  delta: number,
): Promise<GeniePuzzleClockRecord> {
  const data = await postGraphQL<{ advancePuzzleClock: GeniePuzzleClockRecord }>(
    ADVANCE_PUZZLE_CLOCK_MUTATION,
    { clockId, delta },
  );
  return data.advancePuzzleClock;
}

const PROPOSE_RESOURCE_TRADE_MUTATION = `
  mutation ProposeResourceTrade(
    $sessionId: UUID!
    $fromActorId: UUID!
    $fromResourceType: String!
    $fromQuantity: Int!
    $toActorId: UUID!
    $toResourceType: String!
    $toQuantity: Int!
  ) {
    proposeResourceTrade(
      sessionId: $sessionId
      fromActorId: $fromActorId
      fromResourceType: $fromResourceType
      fromQuantity: $fromQuantity
      toActorId: $toActorId
      toResourceType: $toResourceType
      toQuantity: $toQuantity
    ) {
      id
      sessionId
      fromActorId
      fromResourceType
      fromQuantity
      toActorId
      toResourceType
      toQuantity
      status
    }
  }
`;

export interface ProposeResourceTradeInput {
  sessionId: string;
  fromActorId: string;
  fromResourceType: string;
  fromQuantity: number;
  toActorId: string;
  toResourceType: string;
  toQuantity: number;
}

/** Caller must control fromActorId (server-enforced). */
export async function proposeResourceTrade(
  input: ProposeResourceTradeInput,
): Promise<GenieTradeProposalRecord> {
  const data = await postGraphQL<{ proposeResourceTrade: GenieTradeProposalRecord }>(
    PROPOSE_RESOURCE_TRADE_MUTATION,
    input,
  );
  return data.proposeResourceTrade;
}

const ACCEPT_RESOURCE_TRADE_MUTATION = `
  mutation AcceptResourceTrade($proposalId: UUID!) {
    acceptResourceTrade(proposalId: $proposalId) {
      actorId
      resourceType
      quantity
    }
  }
`;

/** Caller must control the proposal's toActorId (server-enforced); rejects
 * self-accept. Returns the updated holdings for both parties. */
export async function acceptResourceTrade(
  proposalId: string,
): Promise<GenieResourceHoldingRecord[]> {
  const data = await postGraphQL<{ acceptResourceTrade: GenieResourceHoldingRecord[] }>(
    ACCEPT_RESOURCE_TRADE_MUTATION,
    { proposalId },
  );
  return data.acceptResourceTrade;
}

const SPEND_RESOURCE_ON_PUZZLE_CLOCK_MUTATION = `
  mutation SpendResourceOnPuzzleClock(
    $clockId: UUID!
    $actorId: UUID!
    $resourceType: String!
    $quantity: Int!
  ) {
    spendResourceOnPuzzleClock(
      clockId: $clockId
      actorId: $actorId
      resourceType: $resourceType
      quantity: $quantity
    ) {
      id
      sessionId
      label
      segmentsCurrent
      segmentsMax
      resolvedAt
    }
  }
`;

/** Caller must control actorId (server-enforced). */
export async function spendResourceOnPuzzleClock(
  clockId: string,
  actorId: string,
  resourceType: string,
  quantity: number,
): Promise<GeniePuzzleClockRecord> {
  const data = await postGraphQL<{ spendResourceOnPuzzleClock: GeniePuzzleClockRecord }>(
    SPEND_RESOURCE_ON_PUZZLE_CLOCK_MUTATION,
    { clockId, actorId, resourceType, quantity },
  );
  return data.spendResourceOnPuzzleClock;
}
