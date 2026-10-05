# Tasks: A World To Try

Ledger for `spec.md`. A box is ticked when the thing is on `main` and the
proof named beside it has been run.

## The account page

- [x] T001 Account controls under the account; `/counter` removed (FR-001,
      FR-002, SC-007). `e47db4f5`.

## Serving it

- [x] T002 `--demo-dir` / `DEMO_DIR`, the `feature.demo` setting, and "Enter
      demo workspace" drawn only when offered (FR-015, FR-017). `17da652d`.
      Proof: `static_files/tests.rs`, `anonymous-shell.spec.ts`.
- [x] T003 The image builds the maps and the demo and carries them at
      `/srv/thunderforge/demo` (FR-016).
- [ ] T004 SC-006 end to end against a running server: setting off, `/demo`
      is not found; on, a signed-out stranger opens it from the sign-in page.

## The demo app, first cut: a board to run

- [x] T010 Map importer with no database, for the demo's scenes. `a8391eaa`.
- [x] T011 `apps/demo`: its own entry, the guard, the in-page backend over the
      server's committed schema, the seed, the standing notice (FR-003,
      FR-004, FR-006 to FR-012, FR-018).
- [x] T012 Proof on the built bundle behind a static file server,
      `pnpm -F @thunderforge/demo run e2e`: dashboard, enter world, token
      drag, wall, door, light, reload, start over, the not-in-the-demo answer,
      seven scenes with their credit, and no request or socket outside
      `/demo/` at any step (SC-001, SC-003, SC-004, SC-005, SC-009; SC-002
      but for the roll).

## Still to do

- [ ] T020 Characters: actors in the seed, a sheet that opens, a roll
      (FR-005, the rest of SC-002).
- [ ] T021 Shapes, token placement from the actors pane, chat, and switching
      scenes on the board, each proved the way T012 proves the rest. SC-009
      today opens each scene's page, not each scene's board.
- [ ] T022 A map the visitor adds stays in the browser (FR-014).
- [ ] T023 Walk every page a Game Master can reach and decide, entry by
      entry, absent or "not part of the demo" (FR-013).
- [ ] T024 One existing feature's end-to-end proof run against the demo
      (SC-008).
