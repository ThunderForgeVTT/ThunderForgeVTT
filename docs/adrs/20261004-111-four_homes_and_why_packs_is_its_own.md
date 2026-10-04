# ADR-111: Four Homes, and Why `packs/` Is Its Own

**Date:** 2026-10-04
**Status:** **ACCEPTED** 2026-10-04. Proven by spec 065 Stories 1 to 3 — every Rust workspace member sits under `apps/`, `crates/` or `packs/systems/`, the repository root has no `src/`, and `scripts/check-layout.mjs` refuses a commit that breaks either — and by spec 066, whose `scripts/check-packs.mjs` holds each pack to one shape. Decision 5 is scheduled, not done.
**Participants:** ThunderForgeVTT Team
**Related:** spec 065 (FR-001 to FR-011), spec 066, [ADR-029](./20260504-029-runtime_module_loading_and_security.md) (no runtime module loading), [ADR-062](./20260902-062-packs_extend_the_engine_with_data_not_code.md) (packs extend the engine with data, not code), [ADR-063](./20260903-063-a_pack_owns_the_tables_it_writes.md) (a pack owns the tables it writes)

---

## Problem Statement

Until 2026-10-04 the four most important crates in the project — the server
library, the core models, the engine and the canvas core — sat under a root
`src/`, beside an `apps/` and a `crates/` that each held some of the rest.
Nobody decided that. It is where the first crate was put, and the next three
followed it.

The cost was that the layout answered no question. Where does a new library
go? Three places had libraries in them. Is this directory something a person
runs? `src/server` was a library and `apps/server` the binary that wrapped
it. Two servers lived in `crates/` and built binaries. The system packs had
grown the same way: seven carried an engine crate nothing depended on and
five a web package nothing discovered, because the first pack had them and
each new one was copied from the last.

## Decision

1. **There are four homes, and nothing else is one.**

   | Home | Holds |
   | --- | --- |
   | `apps/` | Something a person runs: a binary crate, a web entry point. |
   | `crates/` | A Rust library. |
   | `packages/` | Shared web code. |
   | `packs/systems/<id>/` | A game system. |

   The repository root has no `src/`.

2. **An app is thin.** A binary in `apps/` stitches libraries together; a
   crate in `crates/` builds no binary. A member that breaks this on purpose
   is named in `ALLOWED_MISFITS` with its reason, and an entry that no longer
   applies fails the check, so the list cannot outlive what it excuses. It is
   empty today.

3. **`packs/` is its own home because a pack is a vertical slice.** A game
   system's manifest, its server crate, its web code and its seed content
   change together and are owned together. Splitting them by language — the
   server crate to `crates/`, the sheet to `packages/` — would put one
   system in three places and make "what does Roll for Shoes consist of" a
   search instead of a directory listing. The shape of that slice is the
   pack contract (`packs/systems/README.md`), and spec 066's check enforces
   it.

4. **The boundary keeps out-of-tree packs possible, and commits to nothing.**
   Because a pack is one directory with a declared shape and a short,
   written list of what it touches outside itself, it could one day live in
   its own repository or be installed. That is a direction the boundary
   leaves open, not a plan: ADR-029 still stands, there is no runtime module
   loader, and a pack with a server crate is compiled in.

5. **No app imports from another app; what two apps share moves to
   `packages/`. The extraction waits until after the first field test.**
   The API client, the world store, the design system and the game-system
   panels still live in `apps/web` (spec 065 Story 4). Moving them touches
   most of the web source and proves nothing a player would notice, so it
   is scheduled behind play-critical work. Until then the check refuses any
   new app-to-app import, so the debt cannot grow.

6. **The rule is a check, not a convention.** `scripts/check-layout.mjs` and
   `scripts/check-packs.mjs` run before every commit. A crate in the wrong
   place, a root `src/`, a pack with an `engine/` directory or an unlisted
   entry is refused in the commit that adds it.

7. **Records are left as written.** Past specs and ADRs name the old paths
   and are not rewritten; they say what was true when they were decided.
   Living documents — the agent guide, the contributor guide, the pack
   contract — point at paths that exist.

## Rationale

The layout drifted without anyone choosing to let it, twice: once for the
crates and once for the packs. Both times the mechanism was the same — the
next thing was put where the last thing was. A written rule does not stop
that; the person copying a neighbour does not read the rule. A check that
fails their commit does, which is the same reason file length and e2e slices
are checks.

The homes are by *role*, not by language, wherever role and language
disagree. `apps/` holds both Rust binaries and web entry points because the
question a reader asks is "what can I run". `packs/` cuts across languages
for the same reason: the question is "what is this system".

## Consequences

**Positive**

- A new crate, app or pack has exactly one place to go, and the commit that
  puts it elsewhere fails.
- A game system is one directory. Adding one touches a listed, short set of
  files outside it.
- The workspace went from 39 members to 32 with no loss of behaviour.

**Negative**

- `apps/web` is not thin yet. Decision 5 is a debt with a date, and the hero
  builder and engine sandbox cannot share web code cleanly until it is paid.
- `packs/systems/<id>/server` is a library that lives outside `crates/`. The
  rule for where a Rust library goes has one exception, and this ADR is
  where it is explained.
- Paths in past specs and ADRs no longer resolve. That is accepted by
  Decision 7.

## Alternatives Considered

**Split packs by language** — server crates under `crates/`, sheets under
`packages/`. Three homes instead of four and no exception to the library
rule. Rejected: it optimises the rule over the reader, and a system would
have no single place.

**Do the web extraction now.** It would land the pack-extension work on the
final web layout. Rejected for timing: it is the largest change in the spec
and none of it is visible at a table.

**Design `packs/` for installation now.** Rejected: ADR-029 decided against
runtime loading, and no pack outside the repository exists to design for.
The boundary is kept clean so the question can be asked later at low cost.
