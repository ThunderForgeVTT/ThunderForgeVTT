import { postGraphQL } from "@/api/graphqlClient";

/**
 * Genie session-loop GraphQL client — spec 018 User Story 7. The backend
 * (src/server/src/graphql/queries/genie_session.rs,
 * src/server/src/graphql/mutations_genie_session.rs) has always had full
 * query/mutation support for this; nothing in apps/web ever called it.
 * apps/web/src/engine/world/sync/genieSession.ts (inbound world_events
 * NOTIFY dispatch) intentionally doesn't do this fetching itself — this
 * module is the "host page" client it expects to exist.
 */

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

export type GenieShopPriceKind = "RESOURCE" | "ITEM";
export type GenieRewardRecipientMode = "TRIGGERING_ACTOR" | "WHOLE_PARTY";

export interface GenieShopListingRecord {
  id: string;
  actorId: string;
  itemId: string;
  priceKind: string;
  priceResourceType: string | null;
  priceResourceAmount: number | null;
  priceItemId: string | null;
  priceItemQuantity: number | null;
  stockQuantity: number;
}

export interface GeniePuzzleClockRewardRecord {
  id: string;
  clockId: string;
  triggerSegment: number;
  rewardResourceType: string | null;
  rewardResourceAmount: number | null;
  rewardItemId: string | null;
  rewardItemQuantity: number | null;
  recipientMode: string;
  grantedAt: string | null;
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
  mutation AdvancePuzzleClock($clockId: UUID!, $delta: Int!, $actorId: UUID) {
    advancePuzzleClock(clockId: $clockId, delta: $delta, actorId: $actorId) {
      id
      sessionId
      label
      segmentsCurrent
      segmentsMax
      resolvedAt
    }
  }
`;

/** GM-only (server-enforced). `actorId` is optional (spec 020 FR-006a) —
 * attributes a `"triggering_actor"`-mode Puzzle Clock reward crossed by
 * this advance to that actor; omitted, such a reward falls back to a
 * whole-party split. */
export async function advancePuzzleClock(
  clockId: string,
  delta: number,
  actorId?: string,
): Promise<GeniePuzzleClockRecord> {
  const data = await postGraphQL<{ advancePuzzleClock: GeniePuzzleClockRecord }>(
    ADVANCE_PUZZLE_CLOCK_MUTATION,
    { clockId, delta, actorId: actorId ?? null },
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

const DECLINE_RESOURCE_TRADE_MUTATION = `
  mutation DeclineResourceTrade($proposalId: UUID!) {
    declineResourceTrade(proposalId: $proposalId) {
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

/** Caller must control one of the two named actors and not be the
 * proposer (server-enforced). */
export async function declineResourceTrade(
  proposalId: string,
): Promise<GenieTradeProposalRecord> {
  const data = await postGraphQL<{ declineResourceTrade: GenieTradeProposalRecord }>(
    DECLINE_RESOURCE_TRADE_MUTATION,
    { proposalId },
  );
  return data.declineResourceTrade;
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

// ============================================================================
// Spec 020: grants, NPC shops, Puzzle Clock rewards
// ============================================================================

const GENIE_SHOP_LISTINGS_QUERY = `
  query GenieShopListings($actorId: UUID!) {
    genieShopListings(actorId: $actorId) {
      id
      actorId
      itemId
      priceKind
      priceResourceType
      priceResourceAmount
      priceItemId
      priceItemQuantity
      stockQuantity
    }
  }
`;

/** Readable by any world member (FR-004's shop is player-facing). Empty
 * for an NPC with no configured listings — the shop panel should render
 * nothing in that case (Scenario 6). */
export async function fetchGenieShopListings(actorId: string): Promise<GenieShopListingRecord[]> {
  const data = await postGraphQL<{ genieShopListings: GenieShopListingRecord[] }>(
    GENIE_SHOP_LISTINGS_QUERY,
    { actorId },
  );
  return data.genieShopListings;
}

const GRANT_SESSION_RESOURCE_MUTATION = `
  mutation GrantSessionResource($sessionId: UUID!, $actorId: UUID!, $resourceType: String!, $amount: Int!) {
    grantSessionResource(sessionId: $sessionId, actorId: $actorId, resourceType: $resourceType, amount: $amount) {
      actorId
      resourceType
      quantity
    }
  }
`;

/** GM-only (server-enforced, FR-001). Rejects if no active session exists. */
export async function grantSessionResource(
  sessionId: string,
  actorId: string,
  resourceType: string,
  amount: number,
): Promise<GenieResourceHoldingRecord> {
  const data = await postGraphQL<{ grantSessionResource: GenieResourceHoldingRecord }>(
    GRANT_SESSION_RESOURCE_MUTATION,
    { sessionId, actorId, resourceType, amount },
  );
  return data.grantSessionResource;
}

const SHOP_LISTING_FIELDS = `
  id
  actorId
  itemId
  priceKind
  priceResourceType
  priceResourceAmount
  priceItemId
  priceItemQuantity
  stockQuantity
`;

const CREATE_SHOP_LISTING_MUTATION = `
  mutation CreateShopListing(
    $actorId: UUID!
    $itemId: UUID!
    $priceKind: GenieShopPriceKind!
    $priceResourceType: String
    $priceResourceAmount: Int
    $priceItemId: UUID
    $priceItemQuantity: Int
  ) {
    createShopListing(
      actorId: $actorId
      itemId: $itemId
      priceKind: $priceKind
      priceResourceType: $priceResourceType
      priceResourceAmount: $priceResourceAmount
      priceItemId: $priceItemId
      priceItemQuantity: $priceItemQuantity
    ) {
      ${SHOP_LISTING_FIELDS}
    }
  }
`;

export interface CreateShopListingInput {
  actorId: string;
  itemId: string;
  priceKind: GenieShopPriceKind;
  priceResourceType?: string;
  priceResourceAmount?: number;
  priceItemId?: string;
  priceItemQuantity?: number;
}

/** GM-only (server-enforced, FR-004). Exactly one of the resource or item
 * price pair must be provided, matching priceKind. */
export async function createShopListing(input: CreateShopListingInput): Promise<GenieShopListingRecord> {
  const data = await postGraphQL<{ createShopListing: GenieShopListingRecord }>(
    CREATE_SHOP_LISTING_MUTATION,
    {
      actorId: input.actorId,
      itemId: input.itemId,
      priceKind: input.priceKind,
      priceResourceType: input.priceResourceType ?? null,
      priceResourceAmount: input.priceResourceAmount ?? null,
      priceItemId: input.priceItemId ?? null,
      priceItemQuantity: input.priceItemQuantity ?? null,
    },
  );
  return data.createShopListing;
}

const PURCHASE_FROM_SHOP_MUTATION = `
  mutation PurchaseFromShop($listingId: UUID!, $buyerActorId: UUID!) {
    purchaseFromShop(listingId: $listingId, buyerActorId: $buyerActorId) {
      ${SHOP_LISTING_FIELDS}
    }
  }
`;

/** Caller must control buyerActorId (server-enforced). Atomic: verifies
 * afford, deducts/transfers price, transfers the item, and decrements
 * stock with a race-safe conditional UPDATE (FR-005/FR-005a) — a
 * concurrent buyer racing for the last unit gets a clean "out of stock"
 * error. */
export async function purchaseFromShop(
  listingId: string,
  buyerActorId: string,
): Promise<GenieShopListingRecord> {
  const data = await postGraphQL<{ purchaseFromShop: GenieShopListingRecord }>(
    PURCHASE_FROM_SHOP_MUTATION,
    { listingId, buyerActorId },
  );
  return data.purchaseFromShop;
}

const PUZZLE_CLOCK_REWARD_FIELDS = `
  id
  clockId
  triggerSegment
  rewardResourceType
  rewardResourceAmount
  rewardItemId
  rewardItemQuantity
  recipientMode
  grantedAt
`;

const CONFIGURE_PUZZLE_CLOCK_REWARD_MUTATION = `
  mutation ConfigurePuzzleClockReward(
    $clockId: UUID!
    $triggerSegment: Int!
    $rewardResourceType: String
    $rewardResourceAmount: Int
    $rewardItemId: UUID
    $rewardItemQuantity: Int
    $recipientMode: GenieRewardRecipientMode!
  ) {
    configurePuzzleClockReward(
      clockId: $clockId
      triggerSegment: $triggerSegment
      rewardResourceType: $rewardResourceType
      rewardResourceAmount: $rewardResourceAmount
      rewardItemId: $rewardItemId
      rewardItemQuantity: $rewardItemQuantity
      recipientMode: $recipientMode
    ) {
      ${PUZZLE_CLOCK_REWARD_FIELDS}
    }
  }
`;

export interface ConfigurePuzzleClockRewardInput {
  clockId: string;
  triggerSegment: number;
  rewardResourceType?: string;
  rewardResourceAmount?: number;
  rewardItemId?: string;
  rewardItemQuantity?: number;
  recipientMode: GenieRewardRecipientMode;
}

const GENIE_PUZZLE_CLOCK_REWARDS_QUERY = `
  query GeniePuzzleClockRewards($clockId: UUID!) {
    geniePuzzleClockRewards(clockId: $clockId) {
      ${PUZZLE_CLOCK_REWARD_FIELDS}
    }
  }
`;

/** Readable by any world member. */
export async function fetchGeniePuzzleClockRewards(clockId: string): Promise<GeniePuzzleClockRewardRecord[]> {
  const data = await postGraphQL<{ geniePuzzleClockRewards: GeniePuzzleClockRewardRecord[] }>(
    GENIE_PUZZLE_CLOCK_REWARDS_QUERY,
    { clockId },
  );
  return data.geniePuzzleClockRewards;
}

/** GM-only (server-enforced, FR-006). Exactly one of the resource or item
 * reward pair must be provided. */
export async function configurePuzzleClockReward(
  input: ConfigurePuzzleClockRewardInput,
): Promise<GeniePuzzleClockRewardRecord> {
  const data = await postGraphQL<{ configurePuzzleClockReward: GeniePuzzleClockRewardRecord }>(
    CONFIGURE_PUZZLE_CLOCK_REWARD_MUTATION,
    {
      clockId: input.clockId,
      triggerSegment: input.triggerSegment,
      rewardResourceType: input.rewardResourceType ?? null,
      rewardResourceAmount: input.rewardResourceAmount ?? null,
      rewardItemId: input.rewardItemId ?? null,
      rewardItemQuantity: input.rewardItemQuantity ?? null,
      recipientMode: input.recipientMode,
    },
  );
  return data.configurePuzzleClockReward;
}
