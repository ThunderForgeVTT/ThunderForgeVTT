# Phase 1 Data Model: Concurrent Client Sessions

Three kinds of state, and the line between them is the design: one durable
table that gains columns, one in-process registry that deliberately has no
table, and one manifest declaration that is content, not data.

## 1. `user_sessions` — durable, gains description

Exists today as `id, user_id, expires_at, revoked_at, created_at`, with no
unique constraint on `user_id`. Several live rows per user are already
representable; only the login path prevented it.

| Column | Change | Why |
|---|---|---|
| `last_seen_at` | **new**, timestamp, not null | FR-005: "which of these is the one I am using?" is unanswerable without it. Updated coarsely (at most once a minute) so a live session does not write on every request. |
| `client_description` | **new**, nullable text | FR-005: a coarse, human-recognisable origin — browser family and platform as the client reported them. Coarse on purpose; see Privacy below. |
| `ended_reason` | **new**, nullable text | FR-026 and the audit trail: distinguishes `signed_out`, `ended_by_user`, `password_changed`, `bound_exceeded`, `expired`. `revoked_at` alone cannot say which. |

`revoked_at` keeps its meaning exactly. Nothing about lifetime or expiry
changes — FR-025 forbids lengthening either.

**Provenance**: the table predates the `created_by`/`updated_by` convention and
is inherently self-owned (`user_id` is the owner); no provenance columns are
added, matching the existing shape.

**Privacy**: `client_description` is derived from the User-Agent and stored
coarsely (e.g. "Chrome on Linux"). No IP address is stored. Spec 035 set the
precedent that access records "record the act and never the person", and the
same test shape applies here: a rendered session row must contain no address.

### State transitions

```text
created ──(request)──> live ──(sign out)─────────> ended: signed_out
                        │
                        ├──(ended from the list)──> ended: ended_by_user
                        ├──(password changed)─────> ended: password_changed
                        ├──(11th session opens)───> ended: bound_exceeded   [LRU]
                        └──(expires_at passes)────> ended: expired
```

A login creates a row and ends none of them. That single removal is the
feature.

**Bound**: at most 10 live sessions per account. The 11th ends the least
recently used, which is why `last_seen_at` is not merely cosmetic.

## 2. The play-field claim — in-process, no table

```text
PlayFieldRegistry
  account_id -> ClaimEntry { client_id, world_id, claimed_at, notifier }
```

- **One entry per account**, not per world. A person is at one table.
- **`client_id`** is the opaque per-page-load id the client generates —
  the same notion `peer_signaling.rs` already uses ("a session here is one
  live client connection, identified by an opaque id the client generates per
  page load"). Never persisted, never linked to a user record.
- **The entry is owned by a guard** returned at registration and held by the
  play-field stream. When the stream drops — navigation, close, crash, network
  loss — the entry goes with it. This is FR-030 by construction, and it is the
  same mechanism `PeerRegistry::register` uses for spec 028's FR-050.
- **Takeover** is a guarded swap: the incoming claim replaces the entry and the
  displaced holder is notified through `notifier`. Concurrent claims serialise,
  so exactly one holder exists afterwards (SC-009).

**Known boundary, inherited not introduced**: the registry is per server
process, as presence and the peer registry already are. A multi-process
deployment needs a shared registry for all three together; this feature does
not make that worse and does not fix it.

## 3. `checks` — a manifest declaration

Content in `packs/systems/<id>/system.json`, parsed into shared types in
`crates/thunderforge-canvas-core/src/system_rules.rs` so the server and the
engine read one definition. Not a database table: a system's rules are
declared by its pack, never stored per world.

```text
CheckDeclaration
  id           string, unique within the system      "athletics"
  label        string, shown to a person             "Athletics"
  group        string, optional, for grouping        "skills"
  formula      dice formula with placeholders        "1d20 + @modifier"
  bindings     placeholder -> where the value comes from on the actor
```

- **A pack may declare none.** Seven of the eight ship without one today and
  keep working; their sheets offer no check (FR-037).
- **`dnd5e` gains a block** generated from the `abilities` and `skills` it
  already declares — each skill already names its governing ability, so the
  bindings are a transcription rather than a new rule.
- **The engine never interprets a check**, exactly as it never interprets
  `TokenAttributes`. Only the server resolves bindings, and only the existing
  authoritative roll path produces a result.

## Entity relationships

```text
Account ──1:N──> Session (bounded at 10 live)
Account ──0:1──> PlayFieldClaim ──> Client (one page load)
Session ──1:N──> LiveSubscription  (ends with the session)
World   ──1:1──> GameSystem ──0:N──> CheckDeclaration
Actor   ──N:1──> World;  a check resolves against one Actor's values
```

## Validation rules, traced to requirements

| Rule | Requirement |
|---|---|
| A login ends no existing session | FR-001 |
| At most 10 live sessions; the 11th ends the LRU | FR-004 |
| Ending one session leaves the others live | FR-003, FR-006 |
| A password change ends all but the acting session | FR-008 |
| An ended session is refused on its next request and its streams close | FR-009, FR-010 |
| At most one play-field claim per account | FR-027 |
| A claim is released when its client's stream ends | FR-030 |
| Peer registration requires a held claim | FR-038 |
| A check resolves server-side against the actor's own values | FR-035, FR-036 |
| A system declaring no check offers none | FR-037 |
| An offline companion records nothing, queues nothing | FR-040 |
