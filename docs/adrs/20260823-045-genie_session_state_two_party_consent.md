# ADR-045: Genie Session State — Two-Party-Consent Authorization for Session Resource Trades

**Date:** 2026-08-23
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Spec 018 (Genie, User Story 7) introduces the first genuinely new
session-scoped, shared-party-state mechanic in the codebase: a Session
Wish Pool, a Doom Clock, Puzzle Clocks, and Session Resources (Insight,
Favor, Essence) that players gather and trade with one another during
play (FR-013, FR-015, FR-017, FR-018).

Every other write-authorization pattern already established in this
codebase is single-actor: a "DM-only" mutation checks that the caller is
the world's Owner/GM (`is_dm_of_world`, first established for specs
011/013's lore/item content mutations); a "self-ownership" mutation
checks that the caller controls the one actor/token/document being
modified (e.g. `moveOwnToken`'s `owner_user_id = requester` check,
`world_actor_permissions`' Viewer/Editor/Owner ownership block). In both
shapes, exactly one authenticated party decides whether the mutation
proceeds.

`proposeResourceTrade`/`acceptResourceTrade` (FR-017/FR-018) cannot fit
either shape. Trading a Session Resource is, by design, a *negotiation
between two players* — spec.md's Clarifications explicitly frame Session
Resources as "the Catan-style negotiation layer" distinct from the
GM-adjudicated Session Wish Pool (FR-013/FR-014). Routing every trade
through the GM (the simplest reuse of the existing DM-only pattern)
would turn a fast, informal, at-the-table negotiation into a GM
bottleneck, defeating the point of the mechanic (research.md R8). But
letting a single player unilaterally execute a trade — debiting a
counterpart's holdings without that counterpart's own action — is not
authorization at all; it's one player editing another player's state.

## Decision

**Two-party consent**, implemented as a propose/accept pair rather than
a single mutation:

1. **`proposeResourceTrade(sessionId, fromActorId, fromResourceType,
   fromQuantity, toActorId, toResourceType, toQuantity)`** — callable by
   the controller of *either* named actor (checked via
   `world_actors.owned_by == caller`, or the world's GM, per
   `caller_controls_actor` in
   `src/server/src/graphql/mutations_genie_session.rs`). Writes a
   `world_genie_trade_proposals` row with `status = 'pending'` and
   `created_by = caller`. No holdings change yet.

2. **`acceptResourceTrade(proposalId)`** — callable only by a party to
   the trade who is **not** the proposal's `created_by`. The proposer
   attempting to accept their own proposal is rejected outright, not
   silently ignored. Only on acceptance are both actors'
   `world_genie_resource_holdings` rows updated, atomically, in one
   database transaction (data-model.md's "Lifecycle (trade)" note:
   "nothing is written to `world_genie_resource_holdings` for a
   proposal that's never accepted").

A pending proposal is persisted in a small dedicated table
(`world_genie_trade_proposals`, `src/server/migrations/
2026-08-23-190000-0000_create_genie_session_tables/up.sql`) rather than
held in server memory — consistent with how the rest of this server
persists state, and so a pending proposal survives a server restart and
is visible identically regardless of which server instance a given
player's request lands on.

## Alternatives Considered

- **Route every trade through the GM** (reuse the existing DM-only
  pattern verbatim): rejected — contradicts the explicit point of
  Session Resources as a player-to-player negotiation layer distinct
  from the GM-adjudicated Wish Pool (research.md R8, spec.md
  Clarifications Q5).
- **A single `tradeResource` mutation, callable by either party,
  executing immediately**: rejected — this is not consent at all, just
  unilateral action; nothing would stop one player from draining a
  counterpart's holdings without their agreement.
- **A client-side "both players must click a confirm dialog"
  convention, enforced only in the UI**: rejected — matches this
  codebase's established server-authoritative principle (mirrored, for
  example, in ADR-044's dice-roll trust boundary): a policy enforced
  only by well-behaved clients is not enforcement. The two-step
  propose/accept shape makes the consent check a structural property of
  the API, not a UI convention a modified client could bypass.
- **In-memory-only proposal state** (no `world_genie_trade_proposals`
  table): considered, since a proposal is short-lived by nature; rejected
  because it would not survive a server restart and would not scale
  across multiple server instances behind a load balancer — a small,
  persisted table costs little and avoids both failure modes, matching
  how the rest of this server already persists all other state.

## Rationale (Y-Statement)

In the context of adding Session Resource trading, a player-to-player
negotiation mechanic with no precedent in this codebase's existing
single-actor authorization shapes (DM-only, self-ownership), facing the
requirement that a trade be genuinely bilateral rather than GM-mediated
or unilaterally executable by either side, we decided to implement a
persisted propose/accept pair where only the named non-proposing party
may accept, to achieve real two-party consent enforced server-side as a
structural API property, since this codebase's Constitution Principle
III trust-boundary convention requires authorization to be enforced by
the server rather than relying on client cooperation, and no existing
authorization helper (`is_dm_of_world`, `effective_actor_permission`)
already expressed "two specific, distinct parties must each act."

## Consequences

- **Positive**: a trade cannot complete without an affirmative action
  from both named parties — the proposer's `created_by` and the
  counterpart's explicit `acceptResourceTrade` call are both required,
  server-enforced, and unit-tested (`acceptresourcetrade_rejects_self_accept`,
  `accept_resource_trade_rejects_insufficient_holding_and_succeeds_when_funded`
  in `mutations_genie_session.rs`'s test module).
- **Positive**: `spendResourceOnPuzzleClock` (a single-actor "spend my
  own holdings" mutation, FR-017) correctly does *not* need this
  pattern — it reuses the existing self-ownership shape
  (`caller_controls_actor`), since there is no counterpart to negotiate
  with. This ADR's pattern is scoped specifically to actions that debit
  *someone else's* state as a side effect.
- **Negative**: a pending proposal has no automatic expiry implemented
  in this pass — `contracts/genie-session-loop.md`'s `GenieTradeProposal`
  type sketches an "expires if not accepted within a session-configurable
  window" field, but T032-T036's implementation only models
  `pending`/`accepted`/`rejected`/`expired` as a `status` column with no
  background job driving the `pending → expired` transition yet. A
  future pass should add that (or an explicit `rejectResourceTrade`
  mutation, currently also absent — a stale pending proposal today just
  sits unresolved until accepted).
- **Follow-up**: this two-party-consent shape is the first of its kind
  in the codebase (research.md R8 anticipated this) and is a reasonable
  template for any future feature needing genuine player-to-player
  consent (e.g. a base-platform item-trading feature, if one is ever
  built) — it should not be re-derived from scratch; look here first.
