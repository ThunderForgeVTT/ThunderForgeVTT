# ADR-025: Pack Crate Naming Convention

**Status:** Accepted — written 2026-09-04, describing a convention that was
already in the tree

**Decision Date:** 2026-05-04 (stub) · **Written:** 2026-09-04

**Related:** ADR-063 (a pack owns the tables it writes), ADR-061, ADR-066,
`specs/032-pack-architecture/`

## Context

This file was created as an empty stub on 2026-05-04 and stayed empty while the
thing it was meant to describe was built anyway. By spec 032 every bundled
system pack had up to two Rust crates, and they were named by whoever added
them. The convention that resulted is real and consistent enough to write down
— and it contains one trap that is worth an ADR by itself.

## Decision

A bundled pack's Rust crates are named `<short-id>-server` and
`<short-id>-engine`, living at `packs/systems/<pack-id>/server` and
`packs/systems/<pack-id>/engine`, and listed as Cargo workspace members. Each
sets a `lib.name` of the same string with hyphens replaced by underscores.

**`<short-id>` is not always `<pack-id>`.** As of 2026-09-04:

| Directory (`<pack-id>`) | Crate (`<short-id>-server`) |
| --- | --- |
| `genie` | `genie-server` |
| `dnd5e` | `dnd5e-server` |
| `pathfinder2e` | `pathfinder2e-server` |
| `fate_core` | `fate-server` |
| `cypher_system` | `cypher-server` |
| `blades_in_the_dark` | `blades-server` |
| `year_zero_engine` | `yze-server` |

Four of seven differ. `fate_core` drops a word, `cypher_system` drops a word,
`blades_in_the_dark` keeps one, and `year_zero_engine` becomes an initialism.
None of that is wrong — a crate name is a Rust identifier that people type, and
`year_zero_engine_server` is nobody's friend — but it is *undocumented*, and
that is where the trap is.

## The consequence, stated plainly

**The pack id is the directory name, and nothing else.** Not the crate name,
not the `lib.name`, not the manifest's `title`.

Every discovery mechanism in this product agrees on that and would break
quietly if someone assumed otherwise:

- `/api/systems` lists the `packs/systems` directory, and the directory entry
  *is* the id.
- `check-system-registry.mjs` reads its list of forbidden identifiers from
  `readdirSync(packs/systems)`.
- `systemActorSheets.ts` and `systemPanels.ts` (ADR-066) glob
  `packs/systems/*/web/src/...` and take the id off the path.

So a change that renames a *directory* changes the system id and breaks stored
`worlds.game_system_id` values; a change that renames a *crate* breaks the
build and nothing else. Those are very different blast radii, and knowing which
is which is the whole value of this record.

## Consequences

- A new pack picks a short, typeable crate prefix. It does not have to match
  the directory, and it must not be *assumed* to.
- Anything mapping a crate back to a system must go through the directory, not
  through string manipulation of the crate name.
- `src/app/src/system_packs.rs` holds one `use <crate> as _;` per pack — the
  linkage lines ADR-061's check exempts — and those name *crates*. That file is
  the only place in shared code where the crate names legitimately appear.
