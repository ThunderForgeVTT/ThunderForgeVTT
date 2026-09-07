# Peer Reachability Belongs to the Play Field

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/036-concurrent-client-sessions/` (FR-038 – FR-041)
- **Amends**: ADR-052 (the client may hold, continue, and distribute)
- **Follows**: ADR-073 (an account may be signed in many times, and be at the
  table once)

## The decision

A client may register for peer signalling only while its account holds the
**play-field claim for the world that subscription names**. Companion surfaces
— a character sheet on a second screen, the compendium, the lore — never enter
the peer registry, and therefore cannot be addressed by a peer at all.

## Why this is a fourth separation, not a new rule

ADR-052's substance was that three capabilities people had been treating as
one are separable: a client may **hold** content, may **continue** without the
server, and may **distribute** to peers. Each was granted deliberately and
scoped narrowly.

What it never had to say is **which surface** does any of that, because there
was only ever one surface. ADR-073 created a second: an account may now have
several live clients, exactly one of which is at the table. So the question
arrives for the first time, and this is the answer — hold, continue and
distribute all belong to the play field.

## Why it must be enforced at registration

Peer reachability *is* registration. `peer_signaling`'s registry has always
been the address book: a client that is not in it cannot be listed by the
roster query, cannot be named as a destination, and its sends fall out because
`sendPeerSignal` already requires the sending session to be one the caller
registered.

So gating registration makes FR-038 a property of the system rather than a
check on a message path. That matters because **the server must not look
inside what it relays**: spec 028 FR-044 has it pass opaque strings and never
interpret them, deliberately, and inspecting payloads to enforce this would
undo that in order to protect it.

## Why the claim must name the same world

A claim is not "this account is playing somewhere". It is *this client is at
this world's table* — which is why a claim carries a world at all.

Accepting a claim on a different world would join two scopes that are
deliberately separate. An account holding the table in a scratch world would
gain peer reachability in every world it happened to open a companion window
on, and the capability would follow the **account** rather than the **table**
— which is FR-038 leaking by the back door, with the additional insult of
being invisible.

It costs nothing real. Only one client per account can be at a table
(FR-027), so "play field on world A, companion on world B" genuinely has no
play field on world B.

## What happens to a claim released mid-session

**Nothing, to streams already open.** The rule is enforced when a client
registers, not on every signal.

Three reasons, in order of weight. The displaced window is told by its own
claim stream and tears its own transfer down, so the ordinary case needs no
server action. What aborting would actually kill is a transfer of bytes the
requester is already entitled to — they are in the fetch list the server
computed, and verified against a fingerprint before being stored — so killing
it costs bandwidth and buys no authority. And re-checking per signal is the
design ADR-052's own reasoning rejects, for the same reason it made the
subscription the registry: a rule applied once, structurally, beats a rule
applied everywhere, remembered.

The residue is stated rather than hidden: **a former play field can finish a
channel it opened, and cannot open a new one.**

## The race, and why the grace period exists

`playField` and `peerSignals` are two subscriptions on one socket, and nothing
orders them. A client that opened peer signalling first would be refused for a
claim it is about to hold.

So the gate watches the account's claim *before* reading it — a claim taken
between the two arrives on the watch rather than falling into the gap — and
then waits a short grace for one to appear. A companion is never registered
whichever way the race falls; it only receives a slower refusal. Only an
account with no claim on that world waits at all.

## Consequences

- **Positive.** A companion window cannot become a second peer endpoint for
  one person, which is the concrete thing spec 036 was narrowed to prevent.
- **Positive.** Spec 038's shared tab audio inherits this for free: it rides
  peer connections, so "a companion may not share" needs no second mechanism.
- **Cost.** The refusal is not instantaneous — a client with no claim waits
  out the grace before being told. Acceptable because the only client that
  waits is one that would be refused anyway.
- **Cost.** `holds_play_field` now watches on every peer registration, which
  is what exposed a slow leak in the claim registry's listener map. Closed in
  the same change; recorded here because the pressure came from this decision.
- **Obligation.** If peer transfer is ever offered from a surface that is not
  the play field, this ADR is what has to be revisited — not the code that
  happens to implement it.
