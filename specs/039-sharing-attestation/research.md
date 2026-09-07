# Phase 0 Research: The Sharing Attestation

Twelve decisions. Each records what the codebase does today, what was chosen,
and what was rejected — because most of these have a plausible wrong answer
that looks simpler, and three of them end somewhere the spec did not expect.

Everything asserted here about existing code was read out of the file. Line
numbers are as of 2026-09-07.

---

## R1 — How the server comes to know what the terms say

**What is there today**: it does not. `legal/*.md` reaches the product through
exactly one path — `apps/web/src/legal/legalDocuments.ts`, which does

```ts
const DISCOVERED = import.meta.glob<string>("../../../../legal/*.md", {
  query: "?raw", import: "default", eager: true,
});
```

resolved by Vite at build time. Nothing in `src/server` reads `legal/` at all;
grep for `include_str!` in the server finds one hit, and it is
`config/realm-defaults.json`. So the words the person agrees to live only in a
browser bundle, and the party FR-014 says must record the agreement has never
seen them.

**Decision**: the server compiles them in, on the precedent already set.

```rust
// src/server/src/legal/mod.rs
pub const SHARING_TERMS: &str = include_str!("../../../../legal/sharing-terms.md");
pub const OPERATOR_RESPONSIBILITIES: &str =
    include_str!("../../../../legal/operator-responsibilities.md");
```

and the web stops using its own copy **for the attestation surface only**,
fetching the text and its version from the server via `sharingTerms`. The
policy pages (`/legal/terms`, `/legal/privacy`, `/legal/dmca`) keep their glob;
they render prose and record nothing.

**Rationale**: `admin.rs:31–37` already argues this exact case for
`realm-defaults.json` and the argument transfers verbatim — "a seed read from
disk is a seed that can be absent at exactly the moment it is needed, which is
the first boot, on someone else's machine. Compiled in, the file is editable,
reviewable and versioned, and cannot go missing." FR-026's property is
preserved exactly: editing the markdown still changes what the product says
with no code change. It is now two build inputs rather than one.

The second half — the client rendering server-supplied text rather than its own
— is what makes it impossible for the two to disagree. If the web rendered its
bundle's copy and posted the server's version identity, a mismatched deploy
would produce an attestation to words nobody was shown, which is worse than no
attestation because it looks like evidence.

**Alternatives rejected**:

- *Read `legal/*.md` from disk at runtime.* Reintroduces the failure
  `admin.rs` already wrote down: a container whose working directory does not
  contain a `legal/` directory boots fine and refuses every share, or worse,
  serves an empty agreement.
- *Duplicate the text into a Rust constant.* Two sources of truth for a legal
  document, diverging silently, and a reviewer who reads `legal/` reviews the
  wrong one.
- *Have the client send the text it displayed and the server store that.* The
  client then decides what was agreed to, which is FR-014 inverted.

---

## R2 — What a version identity is

**What is there today**: nothing. No front matter, no date, no "last updated"
line, no hash. Grep across Rust, TypeScript and SQL for
`terms_version|termsVersion|policy_version|accepted_terms|document_hash`
returns no hits. The only revision signal in the repository is prose in
`legal/README.md`'s status table.

**Decision**: the version identity is the **content hash of the document's own
words** — `sha256` of the text after the leading HTML comment is stripped and
the result trimmed, rendered as the first 16 hex characters, e.g.
`sharing-terms@4f2a9c1e77b03d58`. Computed at startup, never written by hand.

**Rationale**: FR-015 says the version "changes when the words change", and a
hash is that sentence expressed as arithmetic. A hand-maintained `v3` in front
matter satisfies FR-015 only while everybody remembers to bump it, and the one
time somebody does not is the revision that matters. Normalising away the HTML
comment matters because every file in `legal/` opens with one explaining what
it is and who has reviewed it — `legalDocuments.ts`'s `sectionsOf` already
strips exactly that before rendering, so the Rust side normalises identically
and a note added to the comment does not mint a spurious version.

**Alternatives rejected**:

- *A `version:` field in front matter.* Puts a correctness requirement on a
  reviewer editing prose, which is the person least placed to carry it.
- *The git commit that last touched the file.* Not available in a built
  container, not stable across a rebase, and not a property of the words.
- *An incrementing integer in the database.* Requires deciding, at boot, whether
  today's text is "the same version" as yesterday's — which is the hash
  question with extra steps and a wrong answer available.

---

## R3 — Where a version, once agreed, is retrievable from

**What is there today**: nothing to retrieve from. A compiled-in constant holds
one version — today's — and FR-016 asks that *every version ever attested to*
stay retrievable, forever, including after the file has been revised twice more.

**Decision**: a `terms_versions` archive, written at **startup**, before any
attestation to that version can exist. `ensure_terms_versions_recorded(state)`
runs from `src/app/src/main.rs` beside `ensure_admin_bootstrap_code` and
`ensure_instance_identity`, hashes each compiled-in document, and inserts the
full body if that hash is not already present. Idempotent by primary key.

An attestation stores the version id. Reading an old attestation joins to the
archive and returns the words as they were.

**Rationale**: the ordering is the whole design. Because the archive is written
when the server that could accept the attestation starts, there is no window in
which an attestation names a version whose text was never captured. FR-008 and
FR-017 then need no retention rule and no migration when the file changes —
revising `legal/sharing-terms.md` and redeploying simply adds a row.

**Consequence accepted**: an operator who deploys a build, never shares
anything, and redeploys with revised terms accumulates an archived version
nobody used. Harmless — the table is small and its rows are the size of a
paragraph.

**This decision forces a change to the prose, and that change is a policy
edit.** `legal/collection-sharing-terms.md` says "collection" throughout and
`WorldCollectionsPage.tsx` is its only reader. FR-002 puts the same agreement in
front of somebody sharing a single actor. Showing a person the word
"collection" while they share a character sheet is the kind of detail that makes
a recorded agreement worth less than it looks. So the file is renamed to
`legal/sharing-terms.md` and reworded to speak about *what you are sharing*
rather than *your collection*. **That is new wording in a legal document and
needs the review `legal/README.md` already demands.** The rename also mints a
new version identity, which is correct and is exactly the transition US4 exists
to prove.

**Alternatives rejected**:

- *Store the full text on every attestation row.* Simplest, and duplicates a
  document once per share. The archive costs one join and makes "show me every
  version this instance has ever published" a query rather than a scan.
- *Write the archive lazily, on the first attestation.* Two writers racing the
  same first share, and a `terms_versions` row created inside a share
  transaction that may roll back.
- *Keep one document and add a type-neutral sentence.* Considered seriously.
  Rejected because the second section — "A copy someone takes is theirs, and
  cannot be recalled" — has to change anyway (§ R6), so the document is being
  revised regardless and half-revising it is the worse outcome.

---

## R4 — Where the requirement is enforced

**What is there today**: in a React component, and only one of them.
`WorldCollectionsPage.tsx:49` holds `const SHARE_TERMS =
legalSections("collection-sharing-terms")`, renders it into a
`data-testid="share-terms"` block, and puts a button under it reading "I have
the right to share this — create a link". That button is the entire control.
`create_collection_share_link_impl(state, user_id, is_admin, collection_id)`
takes four arguments and none of them is an agreement. The three singleton
paths render nothing at all: `ActorDetailPage.tsx`, `ItemDetailPage.tsx` and
`AbilityDetailPage.tsx` import neither `LegalProse` nor `legalDocuments`, and
their share buttons mint a code and copy it to the clipboard.

**Decision**: one gate, in the server, called by all four impls before a code
is minted and recorded in the same transaction that mints it.

```rust
// src/server/src/publishing.rs
pub async fn require_attestation(
    state: &AppState,
    subject: Uuid,
    kind: PublishableKind,
    target: Uuid,
    offered: &AttestationInput,
) -> GraphQLResult<PendingAttestation>;
```

Each `create_*_share_link_impl` gains one `attestation: AttestationInput`
argument and one call. The `PendingAttestation` is written alongside the share
row inside the existing transaction, so there is no state in which a share link
exists without its attestation.

**Rationale**: Principle III already says authorization belongs at the data
boundary rather than in a component, and the checklist's own note calls FR-011
and FR-014 the requirements most likely to be quietly dropped — the work *looks*
finished when the dialog appears on four pages. Putting the gate in the impl,
not the resolver, means the unit tests that already exercise these impls are
where the refusal is proved, and a future caller that bypasses the GraphQL
layer is refused too.

**How FR-002 covers a content type nobody has written yet**: an SDL guard test,
in the style of the existing
`the_access_surface_is_registered_under_the_names_the_client_uses`, walks the
built schema and asserts that **every mutation whose name matches
`create*ShareLink` takes an `attestation` argument of the right type**. A fifth
share path added later fails that test on the day it is written, which is what
"without a separate decision" has to mean in practice. A requirement that only
lives in a spec is a requirement the fourth implementer does not read.

**Alternatives rejected**:

- *A GraphQL directive or middleware over the whole mutation root.* Would need
  to know which mutations publish, which is the same list, expressed once
  removed from the code that uses it, and invisible in the impl a person reads.
- *A checkbox the client must tick before enabling the button.* That is what
  exists, and it is what this feature was written because of.
- *Requiring the attestation on the resolver rather than the impl.* Leaves the
  impls callable without one, and the impls are the tested surface.

---

## R5 — What a refusal is allowed to say

**What is there today**: the share modules already have this instinct. All
failure paths in `mutations_collection_shares.rs` collapse to one constant —
`pub const UNAVAILABLE: &str = "This collection link is no longer available";`
— so that a probe cannot distinguish "revoked" from "never existed" (FR-009d,
and spec 027 FR-011 before it).

**Decision**: two refusal strings, neither of which names a valid version.

- Missing or unrecognised attestation → *"This share needs the current sharing
  agreement. Reload the page and try again."*
- Publishing suspended or account disabled → the standing message, which names
  the process and links to the person's own standing page.

**Rationale**: FR-013 asks for a refusal that says what is missing without
helping fabricate one. Echoing "expected version `sharing-terms@4f2a…`" would
hand a script exactly the string it needs. "Reload the page" is both the honest
diagnosis (the client's copy is stale) and the actual fix, and the version is
obtainable one legitimate request away through `sharingTerms` — which is not a
leak, because knowing the current version is not what an attestation is
evidence of. What it is evidence of is that a person was shown the words and
acted; the record of that is the server's to write.

**Alternatives rejected**:

- *Refuse silently and return a dead link.* Punishes a legitimate client with a
  stale bundle, and there is no reason to be coy — the requirement is public.
- *Name the expected version in the error.* See above.

---

## R6 — Reaching copies already adopted, and what it costs

**This is the expensive one.** It is argued at length because the alternative
to arguing it is discovering it in Phase 2.

**What is there today**: nothing, by explicit design.
`src/server/src/collections/copy.rs` opens with

> FR-012 forbids any referential link back to the source. So the copies carry
> no source id, and the receipt this returns is **not stored** — a row naming
> both the source collection and the records made from it is exactly the link
> the one-time-deep-copy invariant exists to prevent. The receipt is handed to
> the person who copied and then forgotten.

That is accurate. `NewWorldAbility`, the `world_items` insert, `copy_actor`'s
insert and `copy_lore`'s insert carry no source column, none exists on the
tables, and the `HashMap<Uuid, Uuid>` maps that remember the correspondence are
local to the transaction and dropped when it commits. **There is no query that
could find an adopted copy.** The only accidental thread is the shared
`asset_id` on `world_items.icon_asset_id`, `world_actor_images.asset_id` and
`scenes.background_asset_id`: copies point at the same stored bytes, so a
takedown against an *uploaded file* already reaches every copy of it. A takedown
against *text* — a transcribed stat block, a rules paragraph in a lore entry —
reaches nothing, and text is the case the spec was written about.

**Decision**: record the adoption. One row per copied entity, written inside
`copy.rs`'s existing transaction:

```text
content_adoptions(id, source_entity_type, source_entity_id,
                  copy_entity_type, copy_entity_id,
                  destination_world_id, adopted_by, adopted_at)
```

read by `moderation::reach` and by **nothing else** — no query, no field, no
subscription, no route, no admin listing. A takedown against an entity walks
this table transitively (a copy of a copy is reachable) and opens a **child
case** per copy; a counter-notice or a withdrawal fans the same way back.

**Rationale**: FR-022 and FR-023 are not satisfiable by any other means. The two
alternatives are content matching, which spec 039 puts out of scope in as many
words ("proactive content inspection, fingerprinting or licence detection"), and
doing nothing, which makes FR-023 a paragraph rather than a behaviour.

**What it costs, stated plainly.** Spec 026 FR-012's claim was that a copy is
independent. After this it is independent *to its adopter* — they own it, edit
it, share it onward, and nothing in the product tells them or anyone else where
it came from — and traceable *to moderation*. Those are different claims, and
only the first one has been made to users so far. ADR-094 has to say so.

**Why this is not the repository ADR-069 argued against**, in the terms ADR-069
actually used: it is unreadable by users; it is unenumerable in both directions
even for an administrator (the only access is a bounded walk from an entity id
that a notice already named); it indexes nothing that was not already published
by somebody's own act; and it exists to make a takedown work rather than to make
content findable. The full determination is in plan.md's guardrail section, and
it needs the accountable owner's signature the way ADR-069 did.

**If the owner declines it**, FR-022 through FR-023d are not buildable and
should be struck from the spec rather than left as requirements nothing
implements. That is a real outcome of this research, not a hedge.

**Why child cases rather than more rows in the source's case**: because
`repeat_infringer_flags_impl` groups by `case_id` and takes the latest event per
case, and because `effective_status` takes the latest event per *entity*. Adding
copy rows to the source's case would make "the latest event in this case"
depend on which copy was touched last, corrupting the counting FR-027 requires
be reused unchanged. Child cases keep both functions exactly as they are. The
link is one new nullable column, `content_moderation_actions.parent_case_id`.

**Why a child case carries `account_id = NULL`**: FR-023b says the adopter must
not be accused of anything, and `account_id` is precisely the column
repeat-infringer counting reads. An adopter who took a copy in good faith must
not accrue a strike for somebody else's upload. The child case's action type is
`content_disabled_as_copy`, which `is_disabled_status` treats as disabled and
`repeat_infringer_flags_impl` never sees, because it has no account.

**Restoration needs no new code.** `effective_status` already restores lazily
when the latest event for an entity is `counter_notice_forwarded` and
`restoration_due_at` has passed. Fanning a `counter_notice_forwarded` row into
each child case, carrying the same `restoration_due_at`, means each copy comes
back on its own next read, through the mechanism that already exists, with no
adopter asking. That is FR-023d, and it is the requirement the checklist
correctly predicted would be missed.

**FR-023c is satisfied by the shape**: a child case names one entity id. It
disables the copy, not the world, not the collection the adopter put it in, not
the adopter's own edits to neighbouring records.

**Alternatives rejected**:

- *Store the copy receipt.* It names a whole collection and every record made
  from it — a richer index than the one needed, and much closer to the shape
  ADR-069 weighed.
- *Hash content and match.* Fingerprinting; out of scope; and defeated by an
  adopter renaming a field.
- *Ask adopters to check.* A takedown that depends on somebody volunteering is
  not a takedown.

---

## R7 — What "disabled, not deleted" means for an adopted copy

**Decision**: exactly what it already means for any moderated entity, because
the enforcement primitive does not distinguish. No content table has a
`disabled` column — `collections/resolve.rs`'s header argues explicitly against
caching moderation state — and a disabled entity is one whose latest
`content_moderation_actions` row is a disabled status. So a disabled copy:

- disappears from list queries, through `filter_visible`;
- returns a **placeholder** from single-entity queries, for every caller
  including its owner — `GraphQLItem::moderated_placeholder`,
  `queries/lore.rs`'s `"[Content removed in response to a takedown notice]"`,
  and the ability equivalent;
- is still a row in the adopter's world, still in whatever collection they put
  it in, still counted, and comes back intact on restoration.

**Rationale**: FR-023a asks that a copy stop being usable and served without
being silently removed, and the placeholder *is* that distinction already built
— it is visible, it says why, and it is not a hole where a record was.
Re-implementing "disabled" for copies would produce a second notion of the word
in one codebase.

**One thing must be added**: the adopter has to be told (FR-023b), and being
shown a placeholder next time they happen to look is not being told. See § R9.

---

## R8 — Strikes, the ladder, and the window, without re-tuning anything

**What is there today**: the counting, complete, and no consequence.
`moderation::repeat_infringer_threshold()` defaults to **3**,
`repeat_infringer_lookback_days()` to **365**, both from environment variables
with a silent fallback. `repeat_infringer_flags_impl` counts, per account,
cases within the lookback whose latest event is `content_disabled` or
`content_remains_disabled`, and returns the account ids at or above the
threshold. It feeds exactly one thing: a list of ids on
`ModerationReviewPage.tsx` for an admin to click. **No account-level state
exists anywhere** — the `users` table has no `disabled_at`, no `status`, no
`suspended_at`, and there is no account deletion of any kind outside
`deleteMyData`.

**Decision**: standing is **derived** from that counting, exactly as content
status is derived from the same table; only the *consequence* is stored.

- `moderation::strike_count(conn, account_id)` is extracted from
  `repeat_infringer_flags_impl`'s body, which then calls it. One definition of
  a strike, used by both, so FR-027 is true by construction rather than by
  agreement.
- `standing_of(state, account_id) -> Standing { strikes, may_publish,
  termination }` composes that count with the ladder and with any open
  `account_terminations` row.
- The ladder is three new environment values beside the three that exist, same
  parse-or-default shape: `MODERATION_STRIKE_WARN_AT` (1),
  `MODERATION_STRIKE_SUSPEND_PUBLISHING_AT` (2),
  `MODERATION_TERMINATION_WINDOW_DAYS` (30),
  `MODERATION_TERMINATION_REQUIRES_HUMAN` (**true**).
- Only the window is a row: `account_terminations`, holding when it opened,
  when deletion is due, the appeal, and how it closed.

**Rationale**: FR-035 is the requirement that decides this. "A strike that ages
past the lookback while an account is disabled MUST be recounted, and an account
that falls below the threshold MUST be restored without the person asking" is
free if standing is derived — the count simply changes when the calendar does —
and is a scheduled job if standing is stored. Storing `may_publish` would mean
maintaining it against every event that could change it, which is the class of
bug `collections/resolve.rs` refuses to introduce for content and there is no
reason to introduce for accounts.

**`MODERATION_TERMINATION_REQUIRES_HUMAN` defaults to true**, and that is a
considered default rather than caution. FR-040 requires that an operator be
able to demand a human decision instead of automatic deletion, and spec 039's
own edge cases include "the instance is self-hosted and the operator does not
want automatic deletion at all". Six friends on a private instance should not
have a timer that deletes one of them. The shipped behaviour is therefore: the
window opens, the account is disabled, the person is told and has their thirty
days — and at the end the termination lands in an administrator's queue.
Automatic execution is a switch an operator throws.

**Alternatives rejected**:

- *A `disabled_at` column on `users`.* Cheap, and then FR-035 needs a job to
  clear it and every path that could change a strike count needs to remember to.
- *A new strikes table.* Duplicates `content_moderation_actions` and creates the
  possibility of the two disagreeing about how many strikes somebody has.
- *Re-tuning the threshold or lookback.* Out of scope by the spec's own
  Assumptions, and unnecessary: the default is already three.

---

## R9 — Telling somebody something happened, when there is no way to tell them

**What is there today**: nothing. There is no `notifications` table anywhere in
the schema or the migrations. There is no mailer — no `lettre`, no SMTP, no
template, nothing in `Cargo.toml` or the source — and spec 040 opens by saying
so in as many words: "no mail subsystem exists anywhere in this codebase". The
only push mechanism is `world_events` + `pg_notify`, which is world-scoped and
which the moderation module never calls, so a GM learns their content was
disabled by navigating to it and finding a placeholder.

**This makes several of spec 039's requirements unbuildable as written**, and
naming that is more useful than quietly approximating them. FR-028 ("the person
MUST be told each time a strike is recorded"), FR-023b ("the adopter MUST be
told"), FR-024 ("a takedown MUST notify the person who shared") and FR-030
("the person MUST be told what happens at the end of that period") all assume a
channel that does not exist.

**Decision**: build the durable half here and leave delivery to spec 040.

```text
account_notices(id, account_id, kind, subject_ref, payload, created_at, read_at)
```

Every event that owes somebody an explanation writes a row: a strike recorded, a
copy disabled in your world, a share of yours taken down, a window opened, an
appeal resolved. The rows are shown on the person's standing page and counted
in a header badge; a disabled account sees them on the one page it can reach.
Nothing is deleted when read.

**Rationale**: a notice that exists as a row is a notice the product can prove
it produced, which is what FR-028's "nobody may reach the third strike having
never been told about the first two" actually requires. A notice sent by mail
and not recorded proves nothing and, today, is not sendable at all. When spec
040 delivers mail, it delivers *these rows*; the table is the queue.

**Stated honestly in the plan and in the ADR**: until spec 040 ships, "told"
means "told the next time they open the product". For a person who has stopped
opening it, that is not good enough, and the thirty-day window is exactly the
case where it matters most. This is a real limitation of shipping US7 before
spec 040 and it is why the task plan sequences the two.

**Alternatives rejected**:

- *Emit `world_events`.* World-scoped, and a strike is about an account. It
  would also put moderation state on a fan-out channel every client reads.
- *Wait for spec 040.* Blocks the whole of US5 and US7 on a feature that has no
  plan yet, and the durable record is the half that carries the evidentiary
  weight.
- *Email directly from here.* Builds a mail subsystem inside a moderation
  feature, which is how the codebase ended up with configuration in five places.

---

## R10 — Scheduling thirty days when there is no scheduler

**What is there today**: two shapes, and both have precedent.

*Lazy on read* — moderation's own auto-restoration. `effective_status`
(`moderation/mod.rs:102`) sees a `counter_notice_forwarded` row whose
`restoration_due_at` has passed and **materialises a real `content_restored`
row on the read path** before answering. Its header says why: "evaluated lazily
here rather than via a background job … so the audit trail stays complete
without new scheduler infrastructure." Session expiry, OAuth session expiry,
invite expiry and world-invite state are all the same idea in simpler form: a
predicate in the `WHERE` clause.

*A spawned task* — `src/app/src/main.rs` starts four, and
`lore_sync/schedule.rs:108` describes the pattern as house style: "the shape
`main.rs` already uses four times — a `spawn_*_task` in the library, called from
the binary, owning its own schedule and staying off every hot path. No new
infrastructure, deliberately." The others are `spawn_session_cleanup_task`
(60s), the presence sweep (60s) and `spawn_content_hash_backfill_task`.

**Decision**: one idempotent function, three callers, no job runner.

```rust
// src/server/src/moderation/standing.rs
pub async fn run_due_standing_work(state: &AppState) -> Result<SweepReport, String>;
pub fn spawn_standing_task(state: AppState);   // 300s, from main.rs, the fifth
```

`run_due_standing_work` opens terminations for accounts newly at the threshold,
closes terminations whose strikes have fallen below it (FR-035), and executes
deletions that are past due **only** when no appeal is open (FR-034) and
`MODERATION_TERMINATION_REQUIRES_HUMAN` is false. It is called from three
places: the periodic task, the admin moderation query, and the sign-in of an
account that has a termination — so the person who signs in on day thirty-one
sees the truth rather than a stale window.

**Rationale**: the lazy shape was the first choice and it does not work here,
for one reason worth writing down. Every other lazily-evaluated rule in this
codebase has a reader: content gets read, sessions get presented, invites get
redeemed. **A deleted account is the thing nobody reads.** A person avoiding
deletion avoids it by not signing in, and a purely lazy design rewards exactly
that. So the tick exists — as the fifth instance of a pattern the codebase
already documents as "no new infrastructure, deliberately", not as a scheduler.

Everything the tick does is reachable without it, which is what keeps it
testable: an e2e test moves `deletion_due_at` into the past and calls the admin
surface, rather than waiting five minutes.

**A known property of the lazy restoration, inherited not introduced**: it
writes on a read path with no transaction and no advisory lock, so two
concurrent reads past the due date can both insert a `content_restored` row.
The fan-out in § R6 multiplies the entities this can happen to. It is
pre-existing, it is benign (a duplicate restoration restores), and it is
recorded in data-model.md rather than fixed by this feature.

**Alternatives rejected**:

- *Purely lazy, on the account's own next sign-in.* See above.
- *An external cron hitting an admin route.* Makes correct behaviour depend on
  an operator's crontab; a self-hosted instance that never writes one has a
  window that is a lie. The route exists anyway, for an operator who wants it.
- *A job-queue crate.* A dependency, a table, a worker and a failure mode, for
  one job that runs every five minutes and does nothing most times.

---

## R11 — What a disabled account may still do, and two gaps found on the way

**What is there today**: `resolve_authenticated_user`
(`auth_middleware.rs:242`) is the single choke point — it already joins `users`
to `user_sessions`, so a standing check costs one more column. Both middleware
layers and the whole GraphQL surface funnel through it via
`graphql/helpers.rs`'s `authenticated_user` and `admin_user`.

**Decision**: `AuthenticatedUser` gains a `standing` field, and
`resolve_authenticated_user` **still resolves the session**. The refusal happens
one layer up, as a positive allowlist:

- `authenticated_user(ctx)` refuses a disabled account.
- `authenticated_user_even_if_disabled(ctx)` is called by exactly four
  surfaces: `exportMyData`, `myStanding`, `fileAppeal`, and sign-out — plus the
  `GET /api/user/data/export` route.

**Rationale**: FR-031 requires that a disabled account retain the download and
the appeal and be able to do nothing else. A filter in the middleware that hides
the session would 401 the export and destroy the remedy. An allowlist mirrors
the `authenticated_user` / `admin_user` split that already exists, and it fails
**closed**: a mutation added next year is refused for a disabled account unless
somebody deliberately opts it in.

**Two gaps found while reading, both recorded rather than assumed away**:

1. **`export_my_data` does not export most of a person's data.**
   `export_user_data_payload` (`users/mod.rs:158`) returns worlds, world tokens
   and world events created by the user; `scenes`, `actors`, `assetPacks` and
   `gameSystems` are permanent empty vectors, and `apps/web/e2e/user-data-export.spec.ts`
   pins that as a known gap, citing this spec. FR-032's letter ("what the
   existing export provides") is satisfiable today; SC-010's substance ("can
   download everything it owns") is not. Filling in actors, items, abilities,
   lore and collections is scoped into US7 — a remedy missing the person's
   characters is not the remedy the spec promises.
2. **`delete_user_data_owned` deletes worlds other people are in.** It loads
   `worlds.created_by == user_id` and deletes every one of them along with their
   events and tokens. FR-038 forbids destroying other people's worlds as a side
   effect of one account's disablement, so deletion must skip a world with live
   members other than the account and hand it to an administrator. That is a
   change to an existing ADR-011 contract and is called out as such.

Two further defects in the moderation programme were found and are worth
recording even though only one is this feature's business:

- `collections/mod.rs`'s `moderation_entity_type` returns `None` for `"scene"`,
  which is documented, deliberate and named by spec 038's FR-026c as the gap
  audio must not repeat. Consequence for this feature: a scene in a shared
  collection is never withheld and never blocked from being copied, so the
  takedown reach of § R6 has a hole shaped like a scene. Recorded; closing it is
  spec 015's or 038's, not this one's.
- `lore_sync/plan.rs:109` and `lore_sync/incoming.rs:242` call `filter_visible`
  with the entity type `"lore_entry"`, while every moderation row uses
  `"world_lore_entry"`. The strings never match, so the Git-mirror sync paths do
  not actually filter taken-down lore. That is a live takedown-reach bug, FR-022
  is exactly the requirement it violates, and it is a one-line fix carried in
  US6.

---

## R12 — The operator acknowledgement, and the prose that does not exist yet

**What is there today**: a real first-run flow — `admin_bootstrap.rs` prints a
one-time setup code at startup when no admin exists, and `admin_setup.rs`'s
`admin_setup_basic` takes a username, an email and a password. Instance-level
settings are singleton rows (`admin_bootstrap_setup`, `auth_security_settings`,
`instance_access_settings`, `instance_identity`). There is no operator identity,
no notice contact, and `legal/terms-of-service.md` carries nine `[OPERATOR]`
markers and `privacy-policy.md` five.

**Decision (the record)**: the operator acknowledgement is **an attestation**,
in the same table, with `purpose = 'operator'`. FR-043 says it is recorded "on
the same terms as a sharing attestation" and the cheapest way to be sure of that
is for it to be the same rows and the same code. The subject is the
administrator completing setup; the target is the instance. `admin_setup_basic`
will not complete without it.

**Decision (the prose)**: `legal/operator-responsibilities.md` is **new** and
has to be written. FR-049 requires it live in `legal/` with everything else, and
nothing like it exists — `terms-of-service.md` speaks to a person *using* an
instance, and this speaks to the person *running* one.

**So the spec's claim that this feature "writes no policy" is not quite true,
and this is where.** Three pieces of prose change:

| Document | Change | Why |
|---|---|---|
| `legal/collection-sharing-terms.md` → `legal/sharing-terms.md` | reworded to cover four publishing paths, not one | § R3, FR-002 |
| the same document, second section | "A copy someone takes is theirs, and **cannot be recalled**" becomes false the day FR-023 ships | § R6 |
| `legal/operator-responsibilities.md` | net-new document | FR-041, FR-049 |

The second row is the sharp one. The shipped terms currently tell a person that
a copy someone takes cannot be reached, and FR-023 makes that untrue for
copies disabled by a takedown. Leaving both in place would mean the product
tells people something it no longer does. **All three need the review
`legal/README.md` already demands, and none of them is a decision this plan is
competent to make.** They are tasks with a reviewer, not tasks with a
developer.

**Decision (the notice contact)**: FR-050 to FR-057 — collecting the operator
identity and the notice contact — belong to **spec 040**, which says so and
whose FR-001 to FR-009 own the setup screen. This feature owns only the *rule*:
`publishing::require_notice_contact()` refuses every publishing path when the
instance has no notice contact (FR-053), while play remains untouched. Until 040
lands, the value is read from `THUNDERFORGE_NOTICE_CONTACT` with the same
parse-or-default shape the moderation values use, and 040 replaces the reader
without touching the rule.

**A consequence the e2e harness will notice immediately**: with FR-053 enforced,
a seeded instance with no notice contact refuses every existing share test.
`src/server/seeds/` must set one, and that is a task, not a surprise to be
debugged at 2am.

**Alternatives rejected**:

- *A separate `operator_acknowledgements` table.* Two implementations of "who
  agreed to which version when", and FR-043 exists precisely to prevent that.
- *Reusing `instance_identity`.* The checklist already rejected it and is
  right: it holds one UUID for spec 034's binding records and has a documented
  hole about database copies.
- *Putting the operator statement in `terms-of-service.md`.* Different reader,
  different moment, and it would be shown to every user of every instance.
