# Quickstart: proving the Sharing Attestation

How to demonstrate each user story against a running stack, and which automated
case covers it afterwards. Every scenario is written so a person can do it by
hand, because "we recorded the agreement" is a claim somebody will eventually
have to make out loud.

## Prerequisites

```bash
make dev          # services, migrations, seeds; admin/admin, user1/user1, user2/user2
```

The seed must set a notice contact. With FR-053 enforced, an instance without
one refuses every publish, so a fresh stack that cannot share anything is this
requirement working rather than a broken environment — check
`THUNDERFORGE_NOTICE_CONTACT` before debugging anything else.

Full suite, when you want the automated answer:

```bash
node scripts/e2e-parallel.mjs --shards=2
```

Per-target checks, per Principle V:

```bash
cargo test --workspace -j 4          # server + crates + packs
make lint                            # lint-host + lint-wasm + file length
pnpm --filter @thunderforge/web test
```

`make lint`'s wasm half matters here for a negative reason: this feature must
not have touched the engine. If it fails, something legal-shaped has ended up in
a crate that compiles to wasm.

## Scenario A — Every path asks, every time (US1, FR-001 / FR-002 / FR-003)

1. As `user1`, make a collection with something in it and share it. **Expected**:
   the agreement appears; the link exists only after you agree.
2. Share an **actor**. Then an **item**. Then an **ability**. **Expected**: each
   asks, with the same words, before any link exists. Before this feature all
   three minted a code and put it on your clipboard.
3. Share the same collection a second time. **Expected**: asked again.
4. Decline. **Expected**: no link, and the collection you assembled is exactly
   as you left it.

**Covered by**: `apps/web/e2e/sharing-attestation.spec.ts`.

## Scenario B — The record outlives what it was about (US2, FR-006 / FR-007)

1. Share something, note the share code.
2. As `admin`, look up the attestation for that thing.
3. **Expected**: it names the person, the moment, the version identity, and the
   thing published — and the words themselves, not a link to whatever the terms
   say now.
4. Revoke the share link. Look the attestation up again.
5. **Expected**: still there, unchanged. The record outlives the share.

**Covered by**: `apps/web/e2e/sharing-attestation.spec.ts`.

## Scenario C — The server is the party that requires it (US3, FR-011 / FR-014)

This one is the point of the feature and takes a terminal, not a browser.

```bash
# Signed in as user1, with a session cookie in a jar:
#   1. no attestation at all
curl -X POST .../api/graphql -d '{"query":"mutation{createCollectionShareLink(collectionId:\"…\"){shareCode}}"}'
#   2. an attestation naming a version that does not exist
#      … attestation:{termsVersionId:"sharing-terms@0000000000000000"} …
```

**Expected**: both refused, no link created, and the message says the agreement
is needed and names no valid version identity. Case 1 is refused by the schema
(the argument is non-null); case 2 by the gate.

Then break it on purpose: delete the `require_attestation` call from
`create_collection_share_link_impl` and watch the server test fail before the
e2e does.

**Covered by**: `apps/web/e2e/sharing-attestation.spec.ts` and the impl-level
tests in each of the four `mutations_*_shares.rs`.

## Scenario D — The words change and old agreements do not (US4, FR-008 / FR-017)

1. Share something. Note the version identity on its attestation.
2. Edit `legal/sharing-terms.md` — change a sentence, not the comment. Restart
   the server.
3. Share something else. **Expected**: a different version identity.
4. Read the first attestation again.
5. **Expected**: the **old** words, unchanged, and the old identity. Nothing
   about the first share was retroactively re-agreed.

Then edit only the HTML comment at the top and restart. **Expected**: the
version identity does **not** change — normalisation is doing its job.

**Covered by**: `apps/web/e2e/sharing-attestation.spec.ts` for the behaviour,
and unit tests in `src/server/src/legal/mod.rs` for the hashing.

## Scenario E — Strikes cost publishing before they cost the account (US5, FR-018 / FR-019)

1. As `admin`, file and uphold a takedown against something `user2` shared.
2. As `user2`, open the standing page. **Expected**: one strike, what it was,
   when it stops counting, and how many remain.
3. Drive `user2` to a second strike. **Expected**: publishing is refused, in
   terms naming the process — and `user2` can still open their worlds, edit
   their content and play. Losing the ability to publish is not losing your
   content.
4. Restore one case. **Expected**: publishing works again, with nobody editing
   a database by hand.

**Covered by**: `apps/web/e2e/account-standing.spec.ts`.

## Scenario F — A takedown reaches the copies (US6, FR-022 / FR-023)

**Requires ADR-079 to be accepted.** If it is not, this scenario does not exist
and FR-022 through FR-023d are withdrawn.

1. As `user1`, share a collection containing a lore entry and an ability.
2. As `user2`, adopt it into a world of your own. Edit the copied lore entry and
   add a note of your own beside it.
3. As `admin`, file and uphold a takedown against `user1`'s lore entry.
4. In `user2`'s world: **expected** — the copied entry shows the takedown
   placeholder, `user2` has a notice saying a copy was disabled and why, worded
   so it is clear they are not accused of anything, and **their own note and the
   rest of their world are untouched**. Nothing was deleted.
5. Follow `user1`'s share link. **Expected**: refused, as a dead link.
6. As `user1`, file a counter-notice; move `restoration_due_at` past.
7. **Expected**: both the source and `user2`'s copy come back, intact, without
   `user2` asking for anything.

**Covered by**: `apps/web/e2e/takedown-reach.spec.ts`.

## Scenario G — Three strikes, the window, and both remedies (US7, FR-030 / FR-034)

1. Drive `user2` to three upheld, unrestored strikes. **Expected**: a notice at
   each of the first two, then disablement, and the third notice says plainly
   what happens at the end of thirty days and that it is irreversible.
2. Sign in as `user2`. **Expected**: the standing page, the download and the
   appeal — and nothing else. Try a world, a mutation, anything.
3. Download the data. **Expected**: it arrives, and the window is exactly where
   it was. Then file an appeal. **Expected**: accepted. Doing one did not
   forfeit the other, in either order.
4. As `admin`, uphold the appeal. **Expected**: `user2` is back, the deletion is
   cancelled, the overturned strike no longer counts, and nothing anywhere still
   treats the account as disabled.
5. Repeat to disablement, move `deletion_due_at` past with an appeal still open,
   and run the sweep. **Expected**: nothing is deleted (FR-034).
6. Repeat, let a strike age past the lookback while disabled, run the sweep.
   **Expected**: the account is restored without anybody asking (FR-035).

**Covered by**: `apps/web/e2e/account-standing.spec.ts`.

## Scenario H — Running it makes it yours (US8, FR-041 / FR-053)

1. Bring up a **fresh** instance — an empty database, no admin.
2. Walk first-run setup. **Expected**: you are shown the operator
   responsibilities and cannot finish without acknowledging them.
3. As that administrator, look up the acknowledgement. **Expected**: who, when,
   which version — the same record a share attestation gets, in the same table.
4. Find the statement again inside the running instance, without a repository.
5. With no notice contact set, try to share something. **Expected**: refused,
   with the administrator told exactly what is missing — and a world, a scene
   and a roll all still work. An instance with nobody to notify is a private
   instance, not a broken one.

**Covered by**: `apps/web/e2e/operator-acknowledgement.spec.ts`.

## Making the guards fail on purpose

House habit, and it applies to five of them here. Before believing any of these,
break the thing once and watch the test bite:

- delete the `require_attestation` call from one of the four impls → Scenario C
  must fail, and so must the SDL guard test;
- make `Attestation.terms` return the compiled-in constant instead of the
  archived row → Scenario D must fail;
- give a child case the adopter's `account_id` → the test asserting an adopter
  accrues no strike must fail;
- add `authenticated_user_even_if_disabled` to a fifth resolver → the allowlist
  test must fail;
- add any query that reads `content_adoptions` → the SDL test that keeps
  ADR-069's determination true must fail.

If any of those passes, the guard is decorative and the requirement it stands
for is not being enforced.
