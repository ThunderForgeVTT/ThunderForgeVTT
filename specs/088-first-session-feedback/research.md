# Research: First Session Feedback

Counted against `main` at `caafdd6d`. Line numbers are from that commit
and drift as the hotfixes land. Each entry gives what is there, what was
decided, and what else was weighed.

## Base

- Worktree `../ThunderForgeVTT-088`, branch `088-first-session-feedback`,
  cut from `main` at `f4263b49` ("E2E: one run per machine, and a second
  run waits for the first").
- Every hotfix branch is on main, cherry-picked, so the branches still read
  as unmerged by ancestry. Matched by subject:
  - `hotfix-invite-transactional`: `af29a822`, `b8dc67fc`, `250dc165`, `ae8e8bfe`
  - `hotfix-invite-uses`: `a7565cb9`, `fa852b5e`
  - `hotfix-world-permissions`: `2258d605`, `c2d0b008`, `68137893`, `07793da2`
  - `hotfix-player-hero-edit`: `65675cf9`, `7d1a7d75`, `a655dd21`
  - `hotfix-player-settings-gif`: `b037d54f`, `90b70526`
  - `hotfix-map-load-sync`: `d4b54507`, `87d5d8d7`
  - `hotfix-ws-keepalive`: `88a43c07`, `487ffdf9`, `5d9a4018`
  - `fix-scene-live-launch`: `0add03cc`
- Line numbers below are from `caafdd6d` and have drifted since.

## US1 World links

### R1 What a world link is today

- `world_invites` has `max_uses INTEGER NOT NULL DEFAULT 0` with
  `CHECK (max_uses > 0)`, so the default is one the check refuses. It also
  has `revoked`, `expires_at` and `rotated_from`.
- `generateInviteCode(worldId, maxUses, expiresAt)` in
  `graphql/mutations_invites.rs` is gated by `runs_the_world` (Owner or GM).
  It refuses `maxUses <= 0`, has no upper limit, and silently drops an
  `expiresAt` it cannot parse.
- `revokeInviteCode` and `rotateInviteCode` exist and are gated the same way.
- `joinWorld` returns one message, `LINK_UNAVAILABLE_MESSAGE` (:358), for
  revoked, expired, used up and unknown alike.
- `queries/invite.rs`: `worldInvites` (gated), `worldByInviteCode` (no auth
  check at all: anyone with a code reads the world's name) and
  `alreadyMember`.
- The clients mint links with `generateInviteCode(worldId, 5)` at
  `components/campaign/CampaignSettingsPanel.tsx:116` and
  `components/world/SessionSetupInviteLink.tsx:32`: five uses, no expiry.

**Decision**: keep the table and the mutations, and fill the gaps: an
optional use limit (the owner's decision: none by default, 1 to 50 when
set, counted only when someone joins), the limits, the expiry parse, the auth on the read, and a
distinct refusal per case. **Alternative**: a new `world_links` table. It
was rejected because the existing one already has every column needed.

### R2 "Existing accounts only"

- `/join/:code` is wrapped in `RequireAuthenticated`.
  `pages/world/JoinWorldPage.tsx:65` sends a signed-out visitor to
  `/login?returnTo=/join/<code>`. The login page offers **Register**.
- OAuth's callback creates a user on first sign-in when the instance's
  access mode allows it (`auth/oauth.rs:394`).

So a link already needs an account, but the sign-in page it leads to can
make one. **Decision**: a `returnTo` under `/join/` hides registration,
and an OAuth flow started from there carries a `sign_in_only` mark in its
state, so the callback refuses to create a user (FR-009, FR-010). This
task (T028) touches `auth/oauth.rs` and `auth/instance_access.rs`, which
`hotfix-invite-uses` also changes, so it merges after that hotfix.
**Alternative**: bind each link to a named account. Not built: the
owner decided that a link is exclusive by admitting only existing
accounts and by being revocable, with an optional use limit.

### R3 The code

`generate_link_code()` (`graphql/share_codes.rs:39-47`) takes the first 20
hex characters of a UUIDv4 and upper-cases them: 80 bits, of which 74 are
random (the 4 version bits and 2 variant bits fall inside the first 20). That is
already hard to guess, but it is not a CSPRNG contract, and the UUID
crate's source of randomness is an implementation detail.

**Decision**: 16 bytes from `rand::rngs::OsRng` (`rand` 0.10 is already a
dependency), written as 26 characters of Crockford base32, with no
ambiguous letters (I, L, O, U). Lookup upper-cases and maps `O`→`0` and
`I`/`L`→`1` before comparing, so a code read aloud still works. The
encoder is about 20 lines with its own tests, not a new crate. Old codes
stay valid (Open item 6), since lookup is by exact match after
normalising, and the old alphabet is a subset.

### R4 Keeping the page out of indexes and logs

- `components/seo/SEO.tsx` writes `<meta name="robots">`; the join and
  invite pages already pass `noindex`.
- No response sets `X-Robots-Tag`. Static files are served from
  `STATIC_DIR` (`apps/thunderforge/src/main.rs:222`, :365).
- The join page shows avatars from third-party hosts, so the full URL,
  code included, goes out in `Referer`.

**Decision**: one Tower layer on the `/join/*` and `/invite/*` routes of
the SPA fallback sets `X-Robots-Tag: noindex, nofollow` and
`Referrer-Policy: no-referrer`. `robots.txt` stays open for these paths:
a crawler that is told not to fetch a page never sees its `noindex`.

## US2 Base maps

### R5 The maps and their credit

- `examples/maps/` holds seven `.dd2vtt` files:
  - `grassy-path-ambush`: 4080 × 2295 px, 0 walls, 0 lights;
  - the demo's showpiece, *The Proving Ground*: 33 walls, 12 lights;
  - `road-side-in`, `little-fish-academy`, `dwarven-forge` (1004 walls),
    `chamber-of-echoing-grief` and `azheim-meeting`.
- They are the owner's CC BY-SA 4.0 work from
  <https://github.com/mbround18/vtt-maps>. Spec 074 FR-018 settled the
  credit text, which lives in `apps/demo/credit.json` and is drawn by
  `credit.ts` and `DemoNotice.tsx`.
- `thunderforge-demo-maps <maps dir> <out dir>` already writes a WebP and a
  thumbnail per map, `maps.json` and `NOTICE.txt`, through
  `map_import/offline.rs`.

**Decision**: reuse that binary in a new Dockerfile stage, and copy its
output into the server image at `/srv/base-maps`, pointed to by
`THUNDERFORGE_BASE_MAPS_DIR`. The credit moves to
`examples/maps/credit.json`, the one source for the demo, the server and
the web app. **Alternatives**:
- importing from the `.dd2vtt` files at runtime, which puts a base64
  decode and a WebP encode on every world creation;
- seeding RustFS with one shared copy (Open item 5).

### R6 The Starting Scene

- `insert_world_sync` (`graphql/mutations_worlds.rs:50-102`) creates the
  world, its GM membership and a Starting Scene: grid 5, square,
  100 × 100, no background. The comment on `STARTER_SCENE_NAME` (:22-41)
  still says the maps cannot ship, which spec 074 overturned.
- The rescue flow calls it too (`collections/rescue.rs:169`).
- A background is set either by `uploadCanvasImage` then `updateSceneLevel`
  (`mutations_levels.rs:116`), or by the REST map import
  (`map_import/mod.rs`, `import_uvtt_impl` :115). The import records one
  `MAP_IMPORTED` (13) event.

**Decision**: `insert_world_sync` stays as it is. After its transaction
commits, `createWorld` runs the import on the Starting Scene from the base
map's prepared files: the WebP goes in through the storage adapter, and
the walls, doors and lights come from `maps.json`. A failure leaves the
world blank, with a non-fatal `STARTING_MAP_FAILED` error (FR-025). **Alternative**: one
transaction around both. It was rejected because the storage upload cannot
take part in a database transaction, and a failed upload should not
refuse the world.

### R7 Which background carries a credit

`canvas_image_assets` has no record of where an image came from.
**Decision**: a nullable `base_map_id TEXT`. `Scene.backgroundCredit`
resolves the credit through it, so a GM's own upload never shows our
credit, and a copied base map always does.

## US3 and US4 Layout

### R8 What the pages do today

- `pages/world/actor/ActorDetailPage.tsx:387`:
  `Container className="grid max-w-2xl gap-6 py-10"`. The `max-w-2xl`
  (672 px) overrides `Container`'s own 1160 px, so the sheet is a single
  672 px column at any width.
- `pages/world/WorldDashboardPage.tsx:135` uses `Container` (1160 px), with
  one `md:grid-cols-2` section (:212), and `CampaignSettingsPanel` at about
  :298.
- `WorldSectionShell`, used by the players and scenes pages, is 1800 px wide.
- Tailwind v4 defaults: sm 640, md 768, lg 1024, xl 1280, 2xl 1536.

**Decision**: each page replaces its `Container` with its own wrapper,
`mx-auto w-full max-w-[1800px] px-4 sm:px-6`, and its own grid
(contracts/layouts.md). `Container` and `components/ui/**` are not
touched, because `e2e:which` treats them as cross-cutting (FR-036).

## US5 Clear rolls

### R9 Where a roll's visibility is decided

- `rolls/visibility.rs` holds `may_roll`, `view_of(RollFacts, Viewer)` and
  `event_reaches`. CONTRIBUTING (:80-100) makes it the only place a roll's
  visibility is decided, and the demo mirrors it in `handlers/dice.ts` and
  `events.ts`.
- The paths to a roll are `worldRolls`, `worldRoll`, `worldRollRecords`, the
  live subscription (`roll_stream_tests.rs`) and the catch-up.
- Event codes run to 38 (`EVENT_CODE_AUTHORING_TOOLS_CHANGED`,
  `world_events.rs:271`). 39 is free.
- Attacks refer to roll records, so a delete would break the combat log.

**Decision**: `pub fn cleared(created_at, rolls_cleared_at: Option<_>) ->
bool`, beside `view_of`. Every path calls it first, and a cleared roll is
withheld from everyone, the GM included. The live stream and the catch-up
also drop `ROLL_MADE` (36) and `ROLL_REVEALED` (37) events at or before the
clear. The cut-off is the roll's `created_at`, not its event's time,
because a roll's event may arrive after the clear (spec.md, Edge Cases).
**Alternatives**:
- deleting rows, which breaks attacks and the world's history;
- a per-viewer "cleared" mark, which would let one player keep a feed the
  GM meant to clear.

## US6 Edge walls

### R10 What an import does with walls

- `map_import/geometry.rs`:
  - `ScenePlacement { grid_size, width, height }`;
  - `point()` maps a UVTT grid point to `(gx * scale - width / 2,
    height / 2 - gy * scale)`, centred, with y up;
  - `walls_from_line_of_sight` and `walls_from_portals` return `WallInsert {
    x1, y1, x2, y2, blocks_vision: true, blocks_movement: true, door_state }`.
- `map_import/mod.rs` inserts walls and doors in one transaction (:230-280),
  reports `walls_created` (:244), and records `MAP_IMPORTED` (:388) with it.
  A re-import adds and never deletes.
- `walls.metadata` is JSONB, free for a mark.
- `offline.rs` (`import_offline`, :75) runs the same geometry with no
  database, and its tests already expect four walls round a 10-cell room.
- The demo's `apps/demo/src/backend/mapImport.ts` mirrors the module.
- The coordinator's check on vtt-dev: `grassy-path-ambush.dd2vtt` has
  `line_of_sight: []`, `portals: []` and `lights: []`, with `map_size`
  48 × 27 at 128 px per grid. Its import recorded `walls_created: 0`.

**Decision**: a pure `perimeter_walls(placement, existing)` in a new
`map_import/perimeter.rs`. For each of the four edges it collects the
intervals that existing walls cover by collinear overlap (both end points
within 0.5 px of the edge's line), merges them, and returns the
uncovered rest as walls. A re-import finds the old perimeter by
`metadata.perimeter = true` *and* lying on the old bounds, and deletes it in
the same transaction before adding the new one. **Alternatives**:
- treating the map's edge as a clamp in movement, not a wall, which would
  not block sight and would be invisible to the GM;
- skipping an edge entirely if any file wall touches it, which leaves gaps
  on maps with partial edge walls.

### R11 A background set by hand

`updateSceneLevel` sets `backgroundAssetId` with no notion of walls.
**Decision**: a `wallEdges` input field (default true), applied only when
the background changes, using the asset's width and height and the
level's grid for the placement. The walls are recorded as `WALL_CHANGED`
(10) events, as the wall tool's own creation records them.

## US7 Mail form

### R12 How mail settings save today

- `pages/admin/components/MailPanel.tsx` draws one `SettingRow`
  (`InstanceSettingsPanel.tsx:163-339`) per key: `SERVER_KEYS` (host, port,
  security, username, password), `IDENTITY_KEYS` (from_address,
  from_name), plus `mail.enabled`. Each row has its own Save.
- A row's draft is set from its prop once (:165) and reset only on its own
  save (:179), so a reload can show a stale draft.
- The mutation is `updateInstanceSetting(key, value)`: `null` clears, a
  blank value is refused, and a key fixed by the environment is refused.
  `settings/graphql.rs:340` says there is no bulk form on purpose
  (resolver :342-359).
- `mail.password` is secret and write-only: the client gets only a
  `secretState`.
- The router is declarative `BrowserRouter` (react-router-dom 7.18), so
  `useBlocker` is not available.

**Decision**: a pure form model (`settingsForm.ts`) over the loaded
settings, a single Save that sends one existing call per dirty key, with
`mail.enabled` last so mail is never switched on before its host is saved,
and a `useUnsavedChanges` hook for `beforeunload` and in-app navigation.
**Alternatives**:
- a transactional bulk mutation (Open item 3);
- moving to a data router for `useBlocker` (Open item 4).

## US8 Hero polish

### R13 What the hotfix leaves

`hotfix-player-hero-edit` gives a player **Open sheet** and **Edit look**
on their own card (`players-hero-edit.spec.ts`). The players grid is
`sm:grid-cols-2 xl:grid-cols-3`, and the claimed character's link is at
`PlayersPage.tsx:331-337`. The builder button is at
`ActorImageryPanel.tsx:243-254`. **Decision**: only the phone layout, the
card order, `?from=players` and the empty state, built on the hotfix's
markup.

## R14 Slices and cross-cutting paths

`scripts/e2e/slices.json` names 29 slices. Of the paths 088 touches,
`schema.graphql` and `auth/**` make `e2e:which` ask for the full suite.
`components/ui/**`, `styles/**` and `routes/**` would too, and are avoided.
The owners are:

- accounts: `invite-membership.spec.ts`, `mutations_invites*.rs`;
- worlds: `mutations_worlds.rs`, the world dashboard, `WorldSectionShell`;
- actors: `pages/world/players/**`, `pages/world/actor/**`; combat and
  game-systems also cover `ActorDetailPage`;
- instance: `pages/admin/**`, `mail-delivery`;
- scenes and canvas: `examples/maps/**`, `map_import/**`, `storage/**`;
- rolls and hero-builder.

Most of `apps/demo` is covered only by its own unit tests and e2e.

## R15 Flags

CONTRIBUTING (:314-375) allows only instance-wide flags (`feature.<name>`),
for a feature merged unfinished or one an operator has a reason to switch
off. None of the stories is either, and the operator's switch for base
maps is the directory. **Decision**: no flags.
