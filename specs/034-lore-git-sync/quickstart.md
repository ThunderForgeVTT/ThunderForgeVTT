# Quickstart: Validating Lore Repository Synchronisation

**Feature**: `034-lore-git-sync` · **Date**: 2026-09-04

How to prove this feature works end to end. Scenarios map to the spec's success
criteria; details live in [`data-model.md`](./data-model.md) and
[`contracts/`](./contracts/).

---

## Prerequisites

1. **A `git` binary on PATH.** New with this feature (research R1) — the server
   had no external binary dependency before it. `git --version` must answer.
2. **A registered application on the repository host**, with its private key
   available to the instance as operator configuration (FR-036a, R5).
3. **A throwaway repository** you can install that application on. Use an empty
   one for the first pass and a repository with existing files for the safety
   pass; do not use one you care about.
4. The usual stack: `pnpm dev` with Postgres and RustFS up.

**Before anything else, confirm the unconfigured path.** With no application
registered, `instanceRepositoryIntegration` must answer `configured: false` with
operator guidance, and a world's settings must offer nothing connectable
(FR-036b). Check this *first* — it is the state every self-hosted instance
starts in, and the easiest to leave broken because nobody developing the feature
ever sees it.

---

## Scenario 1 — The mirror (User Story 1, SC-001, SC-002)

1. Create a world with lore entries arranged in a tree, at least one nested two
   deep, at least one carrying tags, and at least one with an embedded image.
2. Connect the repository through the world's settings. Time it: SC-001 says
   under 5 minutes without reading documentation.
3. Acknowledge the FR-037 notice. **Confirm nothing synchronises before you do**
   (FR-038) — a connection with no acknowledgement must never be picked up.
4. Wait for the first run, then clone the repository.

**Expected**: one markdown file per non-disabled entry, in directories mirroring
the tree; front matter carrying `id`, `title`, `tags`, `updated`; bodies
byte-identical to what the app shows; images under `_images/` referenced
relatively.

**Then open the clone in a plain markdown viewer with no network access**
(SC-011). Every entry renders, images included, and links between entries
resolve. This is the check that catches absolute URLs pointing back at the
platform — the most likely way FR-012 and FR-014 get quietly half-implemented.

## Scenario 2 — Edits, renames, attribution (SC-003, SC-004)

1. Edit an entry in the app. A commit appears within 60 seconds (SC-003)
   containing only that file, its message naming the entry.
2. Check `git log` — committer is `ThunderForge VTT`, author is the member who
   made the edit, **and no personal email address appears anywhere** (FR-017).
3. Rename an entry, then move it to a different parent.
4. `git log --follow` on the file. **History survives both** (FR-010, SC-004).
   A delete-plus-create here is the failure this scenario exists to catch.
5. Restore an earlier revision in the app. The commit identifies itself as a
   restore (FR-019).

## Scenario 3 — Failure leaves the world alone (User Story 2, SC-005, SC-006)

Run each of these and confirm, every time, that lore reading and editing in the
app are indistinguishable from a world with no connection:

| Break it by | Expected |
|---|---|
| Stopping the host / cutting network | Connection shows `NEEDS_ATTENTION` with a cause and a remedy; retries back off (FR-030); the Game Master is told once, not repeatedly |
| Uninstalling the application at the host | Same, and named as a revoked grant rather than a generic failure (FR-036d) |
| Force-pushing the branch to unrelated history | Run stops with `stopped_for_divergence`; **nothing is overwritten** until an explicit choice (FR-031) |
| Deleting the repository | Connection marked broken; a different repository can be connected with no loss of in-app lore |

Then restore each cause and confirm every edit made during the outage arrives,
in order, with no duplicates and none lost (SC-005) — without you reconstructing
anything by hand (FR-030).

**SC-006 is the one to be strict about**: zero instances of in-app lore being
altered, hidden, or lost across every one of these. If any failure mode touches
a world's content, the feature is a liability rather than a convenience and the
"our path stays first-class" framing is not real.

## Scenario 4 — Existing files are sacred (SC-007)

1. Connect a repository that already contains files, including some inside the
   directory the world will use.
2. Expected: the first run **stops with an explanation** rather than resolving
   the collision itself (FR-032).
3. Confirm zero files outside the world's directory were modified, then confirm
   the same for files inside it. `git status` on a clone taken beforehand is the
   check.
4. Attempt to connect a second world to the same repository and directory.
   Refused (FR-033).

## Scenario 5 — Moderation and fidelity (SC-008, SC-009)

1. Add an entry that cross-links to an actor, and one that links to another
   lore entry. Confirm the lore link resolves in the clone and the actor link
   stays readable *and* appears in `unresolvable_links` (FR-013).
2. Confirm a fidelity note exists for the permission flattening (FR-037) and
   that the Game Master can see it — SC-008 requires losses enumerated, not
   discovered.
3. Disable an entry through moderation. After the next run it is **absent** from
   the repository (SC-009), the rest of the world still synchronises (FR-015),
   and the owner is told the content may already exist outside the platform's
   control and that removing it there is theirs to do (FR-040).

## Scenario 6 — Credentials stay invisible (SC-010)

With a connection working:

- `grep` the server logs for any credential value. Nothing.
- Read every GraphQL response the client receives. No credential field exists at
  any depth — the contract has nowhere to put one.
- **Check the process table while a run is in flight.** No token in `argv`
  (research R1) — a process listing is worse than a log, and embedding the token
  in the remote URL is the easy mistake this catches.
- Query `loreRepositoryConnection`. No `installationRef`, no `hostKind`
  (FR-004c).

---

## Measured, 2026-09-04

**Planning a 200-entry world takes ~97ms** (`lore_sync::scale_tests`). SC-001
allows five minutes to connect and see a world of that size, and SC-003 allows
sixty seconds for an edit to reach the repository. Planning is a rounding error
against both, which means those budgets are spent on the network — which is
what the criteria assume and what a slow one would spend them on anyway.

The test keeps a deliberately loose ceiling of ten seconds. It runs on whatever
machine is building, beside every other test, against a shared database; a
tight bound would fail for reasons unrelated to the code, and a test that fails
for unrelated reasons is one people learn to ignore.

## The seam check (FR-004a)

Not a runtime scenario — a review step, and the spec requires it be *pointed at*
rather than claimed.

Name every location that knows which host is in use. **Run 2026-09-04, and it
passes**: `crates/thunderforge-repo-host` (the adapter, which is the grant
boundary) and `src/server/src/repo_host.rs` (the effects half), and nothing
else. `src/server/src/markdown/mod.rs` matches the grep and is a false
positive — "GitHub-flavored markdown" is a format's name, not an integration.

One thing the review tightened rather than passed: `disassociate.rs` was
handing an installation reference through to `repo_host`. It never read or
branched on it, so FR-004c's letter held — but "arguably not reading it" is how
a boundary starts eroding, so `repo_host::open_issue_for_connection` now takes
the connection and splits the reference itself. The rule is checkable by
grepping for the field name rather than by judging each use. Then confirm nothing in path mapping, commit synthesis, attribution,
divergence detection, or verification appears on that list. **If the
synchronising job knows it is talking to GitHub, the seam has stopped existing
whatever the requirements say** (FR-004c).

A cheap mechanical version of the same check: `grep -ri github src/server/src/lore_sync/`
should return nothing.

The crate's own tests are the other half. Because it carries no `reqwest` and no
account credential, its refresh-window arithmetic, scope validation and JWT
claim construction are testable outright — `cargo test -p thunderforge_repo_host`
must pass with no network and no GitHub App configured.

---

## Before implementation begins

**FR-042.** The constitution requires an on-record, owner-accepted determination
of whether this constitutes a centralized public repository under spec 015's
policy. The spec supplies the reasoning; a person must accept it. No scenario
here substitutes for that, and no code should be written until it exists.
