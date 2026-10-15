# Tasks: A Token Is Down

**Input**: [spec.md](./spec.md), [plan.md](./plan.md)

**Tests**: TDD. Engine tests run with `cargo test -p thunderforge-engine`.

## Phase 1: Engine (US1, US2)

- [ ] T001 [US1] Failing tests in `plugins/token_defeated_tests.rs`:
  - down on spawn;
  - survives a position-only update;
  - up again restores the colour and removes the mark;
  - culled builds nothing.
- [ ] T002 [US1] Add the `defeated` field to `payloads.rs`, and insert the
  component in both arms of `app.rs`.
- [ ] T003 [US1] Write the `token_defeated.rs` plugin (mark and dim), and
  register it in `startup.rs`.
- [ ] T004 [US2] Size the mark from `token_side`, and test a 2×2 token.

## Phase 2: Proof (US3)

- [ ] T005 [US3] Add the `token_defeated()` export with a mirror test.
- [ ] T006 Rebuild the wasm (`ENGINE_PROFILE=release node scripts/build.mjs --only-wasm --force`).
  The landing's 098 e2e proves it on the page.

## Follow-ups

- F001 Web client: set `defeated` from 046's out-of-the-fight.
