# Sessions, the play field, and companion surfaces

One person, several screens. This document says what each of those screens is,
what it may do, and where the line falls — because until spec 036 the answer
was "one, and opening a second signs the first out".

Written for whoever next has to decide whether a new surface may do something.

---

## Sessions

An account may be signed in on several clients at once. This is the ordinary
case, not an attack: a Game Master with the table on one monitor and their
notes on a laptop, a player with the map on a television and a character sheet
on a phone.

It was not always allowed. `create_session` revoked every live session for an
account on each sign-in, deliberately, "to reduce session replay risk". The
cost was that one account could be signed in in exactly one place; it
interrupted a live demonstration, and it is why every cross-client test in the
suite once had to use two different accounts. **ADR-073** removed it.

What replaces that protection is not a smaller version of it — it is a control
a person can operate:

| | |
|---|---|
| **How many** | At most ten live sessions per account. Reaching the bound ends the **least recently used**, rather than refusing the new sign-in: refusing the new thing is the failure this change exists to remove. |
| **Seeing them** | `/settings/security` lists every live session, most recently used first, with the one you are on marked. |
| **Ending one** | Individually revocable. Ending one leaves the others untouched; ending the current one signs this browser out, which is what somebody who pressed it meant. |
| **Ending all** | Ends every session for the account, **including the one that asked**. The button says so. |
| **A password change** | Ends every session except the one that made the change. |

### What a session says about itself, and what it never says

A session carries a **coarse client description** — "Firefox on Linux" — and
two timestamps. Nothing else.

The description is assembled server-side from a fixed vocabulary in
`thunderforge_axum_auth_core::client_description`. It is never a slice of the
`User-Agent`: a header crafted to smuggle text into somebody's session list has
nothing to smuggle it through, and a client we do not recognise is left
**unnamed** rather than guessed at.

There is no address, and there will not be one. A session list is the most
tempting place in the product to show an IP, because it looks like security.
Spec 035's rule holds: the instance records the act, never the person.
"Firefox on Linux, two hours ago" answers the question somebody actually has;
adding an address turns the same screen into a location history nobody asked
this instance to keep.

### An ended session stops immediately

Two things, not one:

1. Its next request is refused by the auth middleware.
2. **Its live streams end.** `graphql::session_lifetime::until_session_ends`
   wraps the world-event, presence and play-field subscriptions and closes them
   once the session is no longer live.

The second used to be missing, and its absence was worse than it sounds. The
membership gate on a subscription is checked **once**, when the subscription
opens — right for membership, wrong for revocation. A client whose session was
ended an hour ago went on receiving every world event the account could see for
as long as its socket stayed open, which is precisely the client somebody
revokes a session because they no longer trust.

It is a poll rather than a signal, on a few seconds. A broadcast would be
faster and would only reach the process holding the socket; sessions are ended
by whichever server handled the mutation. The database is what every process
shares, and it answers **expiry** with the same mechanism.

---

## The play field, and everything else

Several clients, but **exactly one play field**.

The play field is the table: an engine, a canvas, and the event stream that
drives them. Admitting two of them per person is what the claim prevents —
two engines rendering one world for one person is a way to get two answers.

Everything else that account has open is a **companion surface**: a character
sheet, a compendium page, the settings screen. A companion is a full client
with a full session. It is not a lesser session; it is a different job.

### Claiming is subscribing

The claim is held by the `playField` subscription for as long as that stream is
open — subscribing *is* claiming. There is no separate mutation, and no cleanup
job that could be skipped on a crash: the claim is released when the stream
drops, whatever dropped it, including a revoked session.

However many windows of one account ask for the play field, one of them has it,
and the others are told who does.

### Where the peer line falls (ADR-075)

Peer-to-peer continuation belongs to **the play field alone**.

A companion surface may not open a peer connection or send anything over one.
This is enforced at registration rather than by inspecting payloads — the
server deliberately never interprets what it relays — so `peerSignals` refuses
a caller holding no play-field claim.

The reason is that ADR-052's continuation scope was written for a table that
has lost its server and is carrying on among the people in the room. A
companion is not in that room in the relevant sense: it is a second window
belonging to one of them, and letting it become a second peer endpoint would
mean one person appearing twice in a mesh that is counting participants.

### A companion that has lost the server refuses

An adjudicated action is one only the server may decide — a check rolled from a
sheet, for instance. A companion that cannot reach the server:

- **refuses** the action,
- **says** it has lost the server,
- **names the play field** as where to retry,
- and **records nothing** — not locally, not queued for replay, not at the
  table.

The queueing is the part worth being firm about. A roll held and replayed on
reconnect is a result decided at a time nobody chose, arriving at a table that
has moved on. Refusing is the honest answer, and it is why
`GraphQLRequestError` distinguishes `transport` — "the server never heard you"
— from a server that heard and refused. Only the first sends somebody to
another screen.

This does **not** widen what a disconnected play field may decide. ADR-052's
scope is exactly what it was.

---

## What is shared and what is not

Shared, and identical on every client of every account that can see it: the
world. Which scene it is on, where the tokens are, whose turn it is, what has
been rolled.

Not shared: what a particular screen is *looking at*. A sheet open on a phone
does not move anybody else's view, and a companion scrolling a compendium is
not dragging the table through it.

The rule for a new surface is that one: if it changes the world, every client
sees it, and only the server decides it. If it changes what this screen shows,
it is this screen's business alone.

---

## See also

- **ADR-073** — concurrent sessions and the single play-field claim
- **ADR-075** — the peer boundary belongs to the play field
- **ADR-074** — system-declared checks and sheet-initiated rolls
- **ADR-044** — the server is the only party that may produce a roll
- `specs/036-concurrent-client-sessions/spec.md`
