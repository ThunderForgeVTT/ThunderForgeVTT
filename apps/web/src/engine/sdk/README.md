# Engine SDK — generated types

**Every `.ts` file in this directory is generated. Do not edit them.**

They are produced from the Rust definitions in
`crates/thunderforge-canvas-core/src/resource_display.rs` by `ts-rs`, and any
hand-edit is silently overwritten the next time somebody regenerates.

```bash
pnpm sdk:bindings     # regenerate (writes only the files that changed)
pnpm sdk:check        # export to a temp dir and fail if the committed output differs
```

Only `sdk:bindings` writes here. `sdk:check` — a step of `pnpm verify`, which
the pre-push hook runs — compares a temporary export against these files, and
a plain `cargo test` exports into `target/ts-rs/` (set in `.cargo/config.toml`).
A write into `apps/web/src` makes a running Vite dev server full-reload every
page, which kills a live e2e run.

## Why these are committed rather than built

So the web application builds without a Rust toolchain. The cost of that is
that a committed generated file can fall behind its source, which is why
`pnpm sdk:check` exists and runs in CI — generating types does not prevent
drift on its own, it only moves where drift can happen.

## Why the types are generated at all

The engine's command boundary used to be `apply_world_command(jsonString)`,
with every shape hand-mirrored in TypeScript. The two drifted, and a drifted
field failed **silently**: the engine deserializes what it can and ignores the
rest, so the symptom was a display that never appeared, with no error
anywhere.

Generating from one source makes that class of mistake a compile error. The
tagged enums in particular come through as discriminated unions that narrow on
their tag, which is what turns a wrong payload into a type error rather than a
no-op.

## Where the hand-written part lives

Typed wrappers over these types — the functions application code actually
calls — belong in `commands.ts` beside them, and **are** hand-written. The
rule is only that the shapes are generated.
