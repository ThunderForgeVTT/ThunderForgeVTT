# Runtime Module Loading and Security

- **Date**: 2026-05-04 (opened) / 2026-09-03 (decided)
- **Status**: Accepted
- **Spec**: `specs/032-pack-architecture/` (FR-004, FR-005, FR-013 to FR-017),
  `specs/031-playability/` (T076)

## Context

This file was opened on 2026-05-04 and stayed empty for four months. In that
time it became the gate on User Story 2 of spec 032, on spec 031's T076, and on
FR-017's standing restriction — three pieces of work waiting on a decision
nobody had written down.

The question is narrow to state and easy to widen by accident:

> On what terms may the product execute code it did not compile?

Executable extension would reach three places, and they are not equally
dangerous.

- **The server.** A pack's code here would run beside the database connection
  pool, the object-storage credentials and every world's content. Bundled packs
  link statically; a third-party one would mean dynamic loading, with a
  capability boundary that does not currently exist anywhere in the product.
- **The engine.** `packs/systems/*/engine` crates are compiled to wasm32 with
  the rest of the engine. Loading a pack's code into a *running* wasm engine is
  not a thing this architecture does, and ADR-062 records why it does not need
  to.
- **The browser.** Mounting a pack-supplied component means running somebody
  else's JavaScript in a player's session, with that session's cookies and its
  authenticated access to every world the player can reach.

### What has actually been decided, three times, without being written here

Every time this question has come up in practice, the answer has been the same,
and it has worked:

- **ADR-059** — an interface pack is data, not a module. The format has nowhere
  to put code, which is *why* interface packs shipped without waiting on this
  file.
- **ADR-062** — system packs extend the engine by declaring, not executing. The
  engine holds no per-system knowledge and loads no pack code at runtime.
- **Spec 032, Increment A** — the `SystemRules` contract is implemented by
  crates that are Cargo workspace members, compiled into the binary. Discovery
  is by `inventory`, through the linker, not by loading anything.

Three surfaces, three times, one answer. This ADR ratifies it rather than
inventing something new, because the alternative — leaving it unwritten — is
what produced a four-month gate on work that was never actually blocked.

## Decision

**Code that this product did not compile is not executed. Packs from outside
the product are data.**

Concretely:

1. **A pack obtained from anywhere other than this repository may contribute
   data only** — manifests, declarations, layouts, tokens, tables. It may not
   contribute executable behaviour of any kind, on any of the three surfaces
   above. The formats enforce this by construction rather than by review:
   `deny_unknown_fields` throughout `pack_system_spec`, so a key the format does
   not name is a rejection and there is nowhere for code to go.

2. **Executable extension is bundled-only.** `packs/systems/*/server` and
   `packs/systems/*/engine` are Cargo workspace members. Their code is reviewed
   in this repository, compiled with the product, and shipped as part of it.
   That is not module loading; it is our code, in our binary, and it carries the
   same trust as any other file here.

3. **FR-017's restriction is the decision, not an interim measure.** The spec
   worded it as holding "until those terms exist as a decision of record". These
   are those terms. The restriction stands, and it stands on purpose.

### What this unblocks

This is the half that matters, and it is why writing the ADR was worth more
than continuing to wait for it.

The gate was never "no pack may contribute behaviour". It was "we have not said
whose code may run". Having said it, **a bundled pack contributing behaviour is
allowed**, because a bundled pack's code is already compiled into the product
and reviewed like the rest of it.

So the following stop being blocked:

- **Spec 032, User Story 2** — system packs mounting their own functional
  surfaces (FR-004, FR-005, FR-013 to FR-016), for bundled packs.
- **Spec 032, T014a2** — `graphql.rs` branches on `game_system_id == "genie"`
  to insert a session row at world creation. The fix is a world-creation hook
  a pack registers, which is a bundled pack contributing behaviour, which this
  permits. It comes off `check-system-registry.mjs`'s `KNOWN` list when the
  hook lands.
- **Spec 031, T076** — system-supplied turn structure. Worth checking whether
  it needs behaviour at all: "a ruleset without rounds shows no round counter"
  reads like a manifest declaration, in which case it was never inside this
  gate.

## What we are building towards

This is a *not yet*, not a *never*, and the distinction is worth being concrete
about so that the next person can tell whether the conditions have been met
rather than having to re-open the argument.

Third-party executable packs become a question worth reopening when **all** of
the following are true:

1. **There is demand that data cannot serve.** So far, every want has been met
   by widening a declaration format — values, layouts, groups, tracks, ladders,
   player-named slots. The honest trigger is a real ruleset that genuinely
   cannot be expressed as data, not a hypothetical one.

2. **There is a capability boundary to put code inside.** For the server that
   means a sandbox with an explicit allow-list — no ambient database handle, no
   object storage credential, no network — and a written statement of what a
   pack may ask for and how it asks. For the browser it means the same for a
   session's credentials. Neither exists today, and building one before there is
   demand is how a product acquires a security surface it does not need.

3. **Failure is containable and attributable.** FR-016 already requires that a
   failure inside a pack-contributed surface leaves the rest of the session
   usable and names the responsible pack. That property is cheap to hold for
   bundled code and is the hard part for third-party code.

4. **There is a review or signing story.** "Where did this pack come from and
   who vouches for it" has no answer in this product yet.

Until then the direction of travel is the one that has worked three times:
**make the data formats richer rather than making the trust boundary wider.**
Spec 032's Increment E is the model — Fate, Cypher and 5e all gained real
character sheets, and not one line of pack code runs to draw them.

## Consequences

- **The product ships only its own packs.** A third-party system pack is not
  installable, and that is a stated position rather than a missing feature.
  `packs/interface/README.md` and spec 032's FR-017 already say so; they are now
  saying something that has been decided.
- **Adding a ruleset means a contribution to this repository.** That is a real
  cost — it makes the project the bottleneck for its own ecosystem — and it buys
  a product with no third-party code execution surface at all. For a self-hosted
  application that holds a table's campaign, that trade is worth making now and
  worth revisiting later.
- **The data formats carry the growth.** Every ruleset that cannot be expressed
  is pressure on `pack_system_spec` and `system_rules`, which is where the
  pressure belongs. When a format change cannot absorb a real system, that is
  the signal in condition 1 above.
- **This ADR is falsifiable.** If a bundled pack ever needs to load code at
  runtime to do its job, or if a format change is rejected as impossible rather
  than merely awkward, this decision is the thing to bring back.
