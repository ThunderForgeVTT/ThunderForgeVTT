# `wasm-pack` Remains the Engine's Build Driver, and Cargo Features Reach It Through `--`

- **Date**: 2026-09-01
- **Status**: Accepted
- **Scope**: `src/engine/Cargo.toml`, `scripts/shared.mjs`

## Context

The engine is a `cdylib` compiled to `wasm32-unknown-unknown` and consumed by
`apps/web` as `@thunderforge/engine`. `scripts/shared.mjs` drives that build
through `wasm-pack` and has done since the engine existed, with one deliberate
piece of policy already encoded in it: `engineProfile()` chooses `--dev`
(seconds to build, 220MB to ship) for the dev loop and `--release` (~7 minutes,
24.7MB, 4.15MB after brotli) for everything else, because the gap is mostly the
wasm `name` section rather than code.

Two things brought the toolchain itself up for decision on the same day.

**We needed a Cargo feature to be profile-dependent.** Bevy's `debug` feature is
the sole gate on `bevy_utils::DebugName`. With it off — which is our state
today, confirmed by `cargo tree -e features`, since we build `bevy` with
`default-features = false` and an explicit list — every Bevy diagnostic that
names a system, component or resource emits a constant instead:

```rust
#[cfg(not(feature = "debug"))]
const FEATURE_DISABLED: &str = "<Enable the debug feature to see the name>";
```

So `error[B0002]` arrives with both subjects redacted — the resource *and* the
system, because `system_meta.name` is a `DebugName` too — and `resource does
not exist` never says which one. Thirty-three files in `bevy_ecs` alone name a
subject this way.

That is the failure this crate has already paid for twice. `bevy_log` is
enabled *precisely* so Bevy's errors reach the browser console; the manifest
records a forbidden asset path and a duplicate-camera warning being logged
every frame and reaching no one, and a day lost to the missing `*_render`
halves drawing nothing without complaint. Routing those messages to the console
and then reading a placeholder where the subject should be is most of the way
back to not having them.

But the names are `type_name` string **data**, not symbols. They survive the
stripping the release build depends on, so this is a feature that must be on in
development and off in the shipped bundle — a profile-dependent feature, which
is not a shape the build script previously had to express.

**And that raised whether `wasm-pack` should be driving the build at all.**
`wasm-pack` has no `--features` flag of its own; its `--help` declares
`[EXTRA_OPTIONS]...` as "List of extra options to pass to `cargo build`". The
alternative on the table was to drop it for the two commands it stands in for:
`cargo build --target wasm32-unknown-unknown` followed by `wasm-bindgen`, which
would put every Cargo flag directly in our hands.

## Decision

### 1. `wasm-pack` stays

It is doing three things this repository currently has no other provision for,
each verified on the development machine rather than assumed:

- **Resolving and vendoring a version-matched `wasm-bindgen` CLI.** The binary
  is not installed here (`wasm-bindgen: command not found`), and its version
  must match the `wasm-bindgen` *crate* exactly — pinned at `0.2.127` in
  `Cargo.lock`. Going direct makes that a pin we own and a hard error at bind
  time when the two drift, which is the way this setup breaks months later
  when someone bumps the crate for an unrelated reason.
- **Supplying `wasm-opt`.** Also not on PATH; `wasm-pack` downloads its own and
  runs it on release builds, and we set no `[package.metadata.wasm-pack]` that
  changes this. The 4.15MB brotli figure is downstream of that step. Direct
  `cargo build` plus `wasm-bindgen` produces neither, so the failure mode is
  not a broken build — it is silently shipping a much larger one.
- **Generating the `package.json`** that `buildEngine` then rewrites, setting
  `pkg.name` to the scoped `@thunderforge/engine` the web app imports. Bare
  `wasm-bindgen --target web` emits none.

### 2. Cargo features are passed as trailing extra options after `--`

```
wasm-pack build ./ --dev --target web --out-dir ../../dist/engine \
  --scope thunderforge --out-name engine -- --features debug-names
```

Verified end to end rather than by reading the help text: invoking it with a
deliberately bogus feature name produced

```
[INFO]: 🌀  Compiling to Wasm...
error: the package 'thunderforge_engine' does not contain this feature: definitely-not-a-real-feature
```

— cargo's feature resolver rejecting it, not `wasm-pack`'s argument parser,
which is the proof that the flags arrive where they are meant to.

The separator is required. `[EXTRA_OPTIONS]` is positional, so an un-separated
`--features` is parsed as a `wasm-pack` flag and rejected.

### 3. The feature is declared by us and gated on the profile

`src/engine/Cargo.toml` gains `debug-names = ["bevy/debug"]`, off by default,
and `buildEngine` appends it only when `profile === "dev"`. The dev bundle is
already 71% unmangled symbols, so the marginal cost lands exactly where it is
free and never in the bundle whose size we guard.

`--no-default-features` is deliberately **not** used. It would be a no-op that
implies a default set exists: `bevy` is already declared
`default-features = false`, and our `[features]` block has no `default` key.

### 4. Direct `cargo build` + `wasm-bindgen` is the documented escape hatch,
with a named trigger

Not "never" — *not for this*. The migration becomes correct when we need a flag
`wasm-pack` cannot express through `--`: custom `RUSTFLAGS` or `-Z build-std`,
realistically for shared-memory atomics and engine threading. At that point it
is done deliberately, with the `wasm-bindgen` CLI pin and `wasm-opt` handled as
first-class concerns, rather than as a side effect of chasing a feature flag we
could already pass.

## Rationale (Y-Statement)

In the context of building the Bevy engine to wasm for both the dev loop and
the shipped bundle, facing the need for a Cargo feature that is on in
development and absent from release, we decided **to keep `wasm-pack` and pass
features as trailing extra options after `--`**, and neglected **replacing it
with direct `cargo build` + `wasm-bindgen`**, to achieve **profile-dependent
feature selection without taking ownership of a version-matched `wasm-bindgen`
CLI, a `wasm-opt` invocation, and a hand-authored `package.json`**, accepting
**that flags `wasm-pack` cannot forward will one day force the migration we
are declining today**.

## Consequences

**E2E runs do not get the names.** They build `--release`, by the existing and
still-correct policy that a 57MB unoptimized code section compiled by the
browser on every page load is worse. So a Bevy panic in an e2e failure log
shows the placeholder, and the way to read it is a re-run with
`ENGINE_PROFILE=dev`. This is a real sharp edge and is written down here
because the alternative is someone finding it mid-diagnosis.

**The build cache invalidates correctly.** `getEngineInputsHash` already hashes
`ENGINE_CARGO_TOML`, so adding the feature changes `pkg.sum` and the next build
genuinely rebuilds. This mattered enough to check: the script's own comment
records that a stale bundle "does not fail, it passes for the wrong reason".

**Two build shapes now exist rather than one.** `--dev` and `--release` differ
by a feature as well as a profile. The divergence is one line and one
condition, and it is the minimum that expresses "diagnostics in development,
size in production".

**We remain exposed to `wasm-pack`'s release cadence** for `wasm-bindgen` CLI
compatibility. That is the standing cost of (1), and the thing to weigh again
if it ever lags a `wasm-bindgen` bump we need.

## Alternatives Considered

- **Direct `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen`.**
  Rejected for now, not on principle — see Decision 4. It solves a problem we
  do not have (feature passing already works) at the cost of three we would
  then own: the CLI version pin, `wasm-opt`, and `package.json` generation.
- **Enable `bevy/debug` unconditionally.** Rejected: `type_name` strings are
  data and survive symbol stripping, so this taxes the shipped bundle the
  manifest explicitly guards, for a benefit only developers can use.
- **Bevy's `dev` feature (`debug` + `bevy_dev_tools` + `file_watcher`).**
  Rejected as a superset that misses. `file_watcher` is meaningless on wasm,
  and `bevy_dev_tools` duplicates `RenderProbePlugin` and the `EngineMonitor`
  readout we already have. `debug` alone is the part with no local equivalent.
- **A `[package.metadata.wasm-pack]` profile block instead of a script
  condition.** Rejected: it configures `wasm-opt`, not Cargo features, so it
  cannot express this decision at all.
- **Leaving the names off and reading Bevy source at diagnosis time.** Rejected
  as what we have been doing. It converts every engine-side error into a
  source-reading exercise, which is the cost the `bevy_log` decision was
  already taken to avoid.

## Related Decisions

- **ADR-032** — Canvas Rendering Strategy (Bevy), which put this build here.
- **ADR-038** — the canvas-core split, driven by the same underlying constraint
  that shapes engine diagnostics: `src/engine` targets wasm with no test
  runner, so its `#[cfg(test)]` modules compile and never execute, and what the
  engine cannot test it must instead be able to *report*.
- **ADR-053** — Generated Engine SDK and the ECS/React presentation split,
  which establishes the engine reporting its own state outward rather than
  callers inferring it.
