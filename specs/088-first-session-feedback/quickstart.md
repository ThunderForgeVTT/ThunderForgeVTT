# Quickstart: First Session Feedback

How to see each story work, and how to prove it, on the
`088-first-session-feedback` worktree.

## 0. Setup

```bash
git worktree add ../ThunderForgeVTT-088 -b 088-first-session-feedback main
cd ../ThunderForgeVTT-088
pnpm install
make base-maps                      # new in T031: writes target/base-maps (US2)
export THUNDERFORGE_BASE_MAPS_DIR=$PWD/target/base-maps
make dev && pnpm dev               # services, migrations and seed; then server + web
```

The RustFS bucket `thunderforge-canvas-assets` must exist before
`cargo test` (the app binary creates it on start).

For every e2e run against the external stack:

```bash
export THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1
pnpm e2e:<slice> -- --workers=1
```

## 1. World links (US1)

1. As the GM, open `/world/<w>/players`. **Create link** defaults to
   1 use, 7 days.
2. Copy it. In a private window, sign in as an existing player and open it:
   they join.
3. Open it again as a second account: "This link has already been used."
4. Revoke a fresh link, then open it: "The GM has withdrawn this link."
5. Signed out, open a link: the sign-in page offers no **Register**, and
   no world name.
6. `curl -sI https://localhost:<port>/join/ABC | grep -i 'x-robots\|referrer'`
   shows both headers.

Proof: `pnpm e2e:accounts`, `cargo test -p thunderforge-server invites`.

## 2. Base maps (US2)

1. **New world**: the picker shows seven maps and **None**, with Grassy
   Path Ambush selected, and the credit beside it.
2. Create. The board opens on the map, walled at its edges, with "Map:
   MBRound18, CC BY-SA 4.0" in the corner.
3. Create another with **None**: the old blank Starting Scene.
4. Unset `THUNDERFORGE_BASE_MAPS_DIR` and restart: one `warn` line, and the
   picker shows only **None**.

Proof: `pnpm e2e:worlds`, `pnpm e2e:scenes`, `pnpm e2e:canvas`.

## 3 and 4. Actor view and world page (US3, US4)

Open each page with the browser at 375, 1280 and 2560 px. One, two and
three columns, no sideways scroll, and an 1800 px cap.

Proof: `pnpm e2e:actors` and `pnpm e2e:worlds` (`layout-widths.spec.ts`),
then `pnpm e2e:combat` and `pnpm e2e:game-systems`.

## 5. Clear rolls (US5)

1. A GM and two players in one world. Each rolls; one player rolls for the
   GM's eyes, and the GM rolls GM only.
2. The GM presses **Clear rolls** and confirms. Every feed empties, with no
   reload.
3. Reload each window: nothing comes back.
4. `SELECT count(*) FROM world_roll_records WHERE world_id = '<w>'` is
   unchanged.
5. In the demo, two tabs: the same.

Proof: `pnpm e2e:rolls` (both parts), the demo's tests,
`cargo test -p thunderforge-server rolls`.

## 6. Edge walls (US6)

1. On a blank scene, import `examples/maps/grassy-path-ambush.dd2vtt` with
   **Wall the map's edges** ticked. The answer reports
   `wallsCreated: 4`.
2. Drag a token past the edge: it stops at the wall.
3. Import it again: still four edge walls.
4. Untick the box and import onto a new scene: no walls.

Proof: `cargo test -p thunderforge-server perimeter`, the demo's
`mapImport.test.ts`, `pnpm e2e:canvas`, `pnpm e2e:scenes`.

## 7. Mail form (US7)

1. `/admin/settings/mail`: **Save** is disabled.
2. Change the host and the port: "2 unsaved changes", **Save** enabled.
3. Click another settings section: the guard asks. **Stay**.
4. Save: the network panel shows two `updateInstanceSetting` calls.
5. Change the port again and reload: the browser asks before leaving.

Proof: `pnpm e2e:instance`, `pnpm -F @thunderforge/web test settingsForm`.

## 8. Hero polish (US8)

At 375 px, as a player with a claimed hero: their card is first, **Open
sheet** and **Edit look** are full width, the builder opens full screen,
and the sheet's back control returns to the players page.

Proof: `pnpm e2e:actors`, `pnpm e2e:hero-builder`.

## Finally

```bash
make lint
cargo test -p thunderforge-server
pnpm -F @thunderforge/web test && pnpm -F @thunderforge/web typecheck
pnpm e2e:which --diff=main       # run each slice it names; never e2e-parallel
```
