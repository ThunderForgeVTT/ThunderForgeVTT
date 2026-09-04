# ADR-026: Pack Architecture and Pack Type Standard

**Status:** Superseded — this question was answered by spec 032 and ADRs 059
through 066, none of which knew this stub existed

**Decision Date:** 2026-05-04 (stub) · **Recorded superseded:** 2026-09-04

## Why this file says nothing

It was created empty on 2026-05-04, to hold the standard for what a pack is and
what types of pack exist. It stayed empty for four months while that standard
was decided in full somewhere else.

This is written rather than the file being deleted, because an empty ADR with a
promising title is worse than no ADR: someone looking for "Pack Architecture and
Pack Type Standard" finds it, finds nothing, and concludes the question is open.
It is not open. It is one of the most thoroughly decided questions in this
repository.

## Where the answer actually lives

**The author-facing contract**, which is the thing to read first and is written
so that reading the source is unnecessary:

- `packs/systems/README.md` — what a system pack is and everything it may
  declare.
- `packs/interface/README.md` — what an interface pack is, and why it is the
  half of spec 032 that could ship while the other half waited.

**The standard itself**: `specs/032-pack-architecture/`. FR-002 is the load
bearing sentence — *a pack is a system pack or an interface pack, never both,
and the directory it lives in is what decides*. That closed, two-member
definition is the "pack type standard" this file was named for.

**The decisions underneath it**, in the order they were taken:

| ADR | What it settled |
| --- | --- |
| 029 | Outside code is not run. Only a bundled pack may contribute behaviour. |
| 059 | An interface pack is data, not a module. |
| 060 | One system contract, with declared values. |
| 061 | System rules are discovered, not registered. |
| 062 | Packs extend the engine with data, not code. |
| 063 | A pack owns the tables it writes; `src/server` became a library so it could. |
| 064 | Ability vocabulary is contributed. |
| 066 | A bundled pack ships its own web surfaces, found at build time. |

ADR-025 (crate naming) and ADR-027 (packaging and manifest contract) are the
two neighbours that are about packs and are *not* superseded.

## A collision that was found and resolved, not inherited

`specs/026-content-collections/` describes a user-authored, world-scoped,
link-shared set of items, actors and lore. It was called a **content pack**
until 2026-09-04, which would have been a third meaning of the word and would
have falsified FR-002's "never both" sentence as written.

It is now a **collection** — `content_collections`, `/collection/<code>` — and
FR-002 stands unamended. Recorded here rather than only in that spec, because
the value is in the rule that produced the answer: **"pack" in this repository
means a directory under `packs/` that is compiled into the product.** Anything
a user authors at runtime is not a pack, whatever it bundles.

"Bundle" was rejected as the replacement, for a reason worth keeping so nobody
proposes it again: `bundle` appears in this codebase only as **"bundled"**,
which carries ADR-029's distinction between code compiled into the product and
code that is not. A user-authored "bundle" that is emphatically not bundled
would be the worst name available.
