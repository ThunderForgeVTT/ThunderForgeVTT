# System Packs Extend the Engine With Data, Not Code

- **Date**: 2026-09-02
- **Status**: Accepted
- **Spec**: `specs/032-pack-architecture/` (FR-004, FR-027, FR-031, SC-004)

## Context

The question that prompted this: *can the engine dynamically load systems and
packs based on what is selected, to reduce the toil on engine size?* It is the
right instinct — a renderer that carries every ruleset it might meet is a
renderer that grows without bound — and the answer turns out to be that the
architecture already avoids that, by a route that also happens to sidestep the
question this project cannot currently answer.

Three measurements shape it.

**The engine bundle is 4.92MB brotli** (2026-09-01, after spec 031; 4.15MB
before). The recorded threshold of concern is **100MB compressed** — so it sits
at roughly five per cent of budget, and growth below that line is expected
rather than alarming.

**None of that is packs.** Eight system packs ship engine crates —
`dnd5e-engine`, `genie-engine` and six more — and **nothing depends on any of
them**. No `Cargo.toml` in the workspace lists one. They are compiled as
workspace members and linked into nothing. Splitting them out of the bundle
would remove zero bytes, because they were never in it.

**What the 4.92MB actually is, is Bevy**: the render pipeline, wgpu, text, UI,
sprite. That is the floor of drawing a canvas at all, and no arrangement of
packs moves it.

## Decision

**A system pack extends the product by declaring, not by executing.** The
engine holds no per-system knowledge, gains no bytes when a system is added,
and loads nothing at runtime that is a pack's code.

Concretely, and this is already true after spec 032:

- A system's manifest declares what it has — attributes, resources, movement,
  tracks, ladders, player-named slots, text.
- Its own crate computes what it derives, server-side, through one shared
  contract (ADR-060).
- Both halves resolve to `identifier -> value` pairs that travel to the client,
  and **nothing downstream interprets them**. The engine draws a bar because a
  value said it was a pool, not because it knows what health is.
- An interface pack's canvas half already arrives at runtime as a
  `set_display_appearance` command carrying an `AppearanceOverride`.

So the dynamic part the question was reaching for exists. It is dynamic in
data, which needs no module loading, no ABI, and no answer to ADR-029.

## Why not dynamic code loading

Three reasons, in increasing order of how much they matter.

**It would not help.** The premise is that packs contribute to bundle size.
They contribute nothing. A mechanism to unload zero bytes is a mechanism with
no benefit to weigh its cost against.

**It is not really available.** A Bevy plugin shares an ECS world, which means
shared memory and a stable ABI across the boundary. Rust has no stable ABI, and
wasm's dynamic linking is not where that story ends. The workable shape —
separate modules exchanging messages — is not a plugin; it is a second process
wearing one's clothes, and everything it touches has to be serialised anyway.
At which point it is data, which is what we already send.

**It is the question this project has deliberately not answered.** Loading a
pack's code at runtime is exactly what ADR-029 governs, and ADR-029 is an empty
file. Spec 032's whole shape — an interface pack that is data, a system pack
that declares — exists so that the useful half could ship without that decision
being forced. Reintroducing code loading to save bytes that were never there
would spend a decision nobody needs to make.

## What this does not preclude

**Assets, which are the real weight.** Maps, token art and audio are
legitimately large and legitimately per-world, and they already load
dynamically: Bevy's asset server fetches them, and spec 028's OPFS cache keeps
them on the device. If engine size is the worry, this is the lever, and it is
already built.

**A system pack's own web module.** User Story 2 — a system contributing its
own character sheet as code — remains a real want and remains behind ADR-029.
That is a browser ES module, not an engine plugin, and it is a different
boundary with a different risk profile.

**Revisiting this if a system needs the engine to behave differently.** Nothing
shipping does. Every system so far differs in what it *has*, not in how a
canvas draws it — and the moment one genuinely needs its own rendering
behaviour, ADR-029 has to be answered before anything else can be.

## Why the base pack is not bundled either

A related suggestion was to compile Forge into the build so a pack is always
present. It is rejected for a reason that is not about size.

FR-007 says the base pack must have **no capability another pack cannot have**.
A pack compiled into the binary exists before the network does, which is a
capability no other pack can ever have, and it is the hardest kind to undo once
a design leans on it. `specs/032-pack-architecture/research.md` §3 rejected it
on the same ground.

The behaviour that suggestion wants already exists by another route. Forge is
`apps/web/src/styles/globals.css` written down as a manifest, and a test keeps
the two identical. If the base pack cannot be fetched, the page keeps the
stylesheet it already has — which is Forge. The fallback is not missing; it is
just not a privilege.

## Consequences

- Adding a system costs **zero engine bytes**, which is the property SC-004
  measures from the other direction ("touches only that system's own pack
  directory").
- The engine cannot be made to depend on a system, because it has nothing to
  depend on. A pull request teaching it one is visible as new knowledge rather
  than as a new dependency, which `scripts/check-system-registry.mjs` already
  polices on the server side.
- The eight pack engine crates currently linked into nothing are, on this
  reading, either future US2 work or dead. That is worth deciding rather than
  leaving ambiguous — six of them were emptied of their duplicate trait in
  spec 032 T010a and now contain almost nothing.
- If the bundle ever does become a problem, the answer is Bevy's feature set
  and the asset pipeline, not pack loading. The measurements above are the
  starting point for that conversation, and they should be re-measured before
  it, not quoted from here.
