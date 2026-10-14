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
- [x] T004 SC-006 end to end against a running server: setting off, `/demo`
      is not found; on, a signed-out stranger opens it from the sign-in page.
      Done 2026-10-13: `apps/web/e2e/demo-offered.spec.ts` flips
      `feature.demo` on one running instance and pins both halves in turn,
      restoring what it found. The harness builds the demo once
      (`scripts/e2e/demo.mjs`) and starts the backend with `DEMO_DIR`, and
      the web client's dev server proxies `/demo` to the backend as a plain
      prefix, because vite-plugin-html's history fallback spares a path from
      its index rewrite only when it starts with a proxy key. Proof:
      `pnpm e2e:accounts`, 74 passed, 0 failed, 0 skipped.

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

- [x] T020 Characters: actors in the seed, a sheet that opens, a roll
      (FR-005, the rest of SC-002). Done 2026-10-05: five actors in
      `seed/cast.ts` (two heroes, three 5e stat blocks), Grassy Path Ambush
      opens on the encounter, the fighter's sheet shows his modifiers and
      `rollCheck` resolves Athletics as die + 5. The demo also seeds a player
      who already holds the fighter, behind "View as player" on the notice.
- [X] T021 Shapes, token placement from the actors pane, chat, and switching
      scenes on the board, each proved the way T012 proves the rest. SC-009
      today opens each scene's page, not each scene's board. Scene switching
      is answered (`launchScene`) and the door test uses it, but through the
      API, not the board's own control.
      Proof: `apps/demo/e2e/board-panels.spec.ts`, run as
      `pnpm -F @thunderforge/demo run e2e board-panels`, 5 passed, 0 failed,
      0 skipped (2026-10-13). A rectangle through the shape tool, Elowen
      placed from the actors pane with a click on the board, a chat message
      that survives a reload, and the Settings dock's own `scene-switcher`
      putting The Proving Ground's walls on the canvas. The switcher swaps
      the loaded scene locally; it does not call `launchScene`, so
      `activeSceneId` stays on the ambush. The demo's specs sit in no slice,
      so this is the T012 route, not `pnpm e2e:<slice>`.
- [X] T022 A map the visitor adds stays in the browser (FR-014). Done
      2026-10-13: `uploads.spec.ts` ("a PNG map becomes the board's
      background, and is there after a reload") now also reads the bytes
      back from this browser's IndexedDB store after the reload and asserts
      that no request other than a plain read of the demo's files, and
      nothing outside `/demo/`, was made at any point. Proof: `pnpm -F
      @thunderforge/demo exec playwright test uploads`, 2 passed, on the
      built bundle (the demo's e2e is in no `scripts/e2e` slice; `e2e:which`
      reports it uncovered).
- [X] T023 Walk every page a Game Master can reach and decide, entry by
      entry, absent or "not part of the demo" (FR-013). Done 2026-10-13:
      every route of `AppRoutes.tsx` is a row in `gm-pages.md` (24 absent,
      16 notice plus Create world and Share, 31 work). One fix: a share
      link, and the sharing terms the dialog reads first, are now refused as
      "Share links" (`refusalNames.ts`), not "Characters" or "That feature".
      Proof: `gm-pages.spec.ts`, 8 passed; `demo-pages.spec.ts` 5 and
      `demo-limits.spec.ts` 2 still pass; `refusalNames.test.ts` under
      `pnpm -F @thunderforge/demo test`, 188 passed.
- [x] T024 One existing feature's end-to-end proof run against the demo
      (SC-008). Done 2026-10-13: the web app's dice-roll proof now lives in
      `apps/web/e2e/fixtures/diceRollProof.ts`, run by
      `apps/web/e2e/dice-roll.spec.ts` against the real client and by
      `apps/demo/e2e/dice.spec.ts` ("the web app's own dice proof passes
      against the demo") against the demo, unchanged. Proof:
      `pnpm -F @thunderforge/demo run e2e`, 65 passed, 0 failed.
