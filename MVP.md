# MVP 1 Roadmap

This document outlines the 10 phases for reaching the Minimum Viable Product (MVP) 1 for ThunderForgeVTT.

**Status as of 2026-08-22**: This checklist had never been updated since it was written — every box was still unchecked despite several phases being substantially implemented. Statuses below were verified against the actual codebase (not assumed), and are noted `[x] Done`, `[~] Partial` (with what's missing), or `[ ] Not started`. See `specs/001-bevy-canvas-authoring/`, `specs/002-canvas-authoring-asset-storage/`, `specs/003-dd2vtt-map-fidelity/`, and `specs/010-world-staging-actors/` for the specs that delivered the completed phases.

## Core Concepts and Objects

This section provides a high-level overview of the core objects and concepts that will be implemented as part of the MVP.

- **World:** A container for all the data related to a single game. This includes scenes, actors, game systems, etc.
- **World Events:** A log of all the changes that happen within a world. This is used to keep all clients in sync.
- **Game System:** A set of rules that govern how the game is played. This includes things like character stats, skills, and dice roll formulas (e.g., "1d6+STR"). The game system should be designed to be easily extendable and shareable.
- **Scene:** A single map or location within a world. It has a background, a grid, and can contain tokens, walls, and lights.
- **Actor:** A character or creature within the world. Actors have stats, skills, and other properties defined by the game system.
- **Actor Events:** A log of all the changes that happen to an actor.
- **Token:** A visual representation of an actor on a scene. Tokens have a position, a type (NPC, player, vehicle, etc.), and are bound to an actor.
- **Token Events:** A log of all the changes that happen to a token.
- **Actor-Token Binding:** The link between an actor and a token. This allows the token to display the actor's information and for the actor's stats to affect the token's behavior.
- **Permissions and Policies:** A system for controlling who can do what within a world. This will be used to define roles like "player", "trusted player", "assistant DM", and "owner".

## MVP 1 Roadmap

- [x] **Phase 1: User Login** — Done
  - Users can log in to the application.
  - Implemented: username/password auth, OAuth providers, two-factor auth, session management, admin bootstrap (`src/server/src/auth/`, `src/server/src/users/`). Spec 007 (complete, ADR-041) added environment-variable OAuth provider configuration (`OAUTH_<PROVIDER>_CLIENT_ID`/`_CLIENT_SECRET`/etc.) as a second, deploy-time configuration source alongside the existing admin panel — env vars always win for the fields they set. Providers are generic (any OAuth2/OIDC endpoint set works), with Discord/Google/GitHub/Keycloak shipping as built-in presets, and multiple named instances of the same provider type are supported (e.g. two separate Keycloak realms). A pre-existing, unwired attempt at this same idea (`THUNDERFORGE_AUTHENTICATION` env var / `SupportedAuthentication` config type) was found dead and removed as part of this feature.

- [x] **Phase 2: World Creation** — Done
  - Users can create a "world" with a basic game system/ruleset.
  - Implemented: `createWorld` mutation with `game_system_id`/`interface_pack_id`, world listing/deletion.

- [x] **Phase 3: Scene Creation** — Done
  - Users can create a scene within a world.
  - Users can set a background for the scene.
  - Users can add a grid pattern to the scene at layer zero.
  - Implemented: scene CRUD, `grid_size`/`grid_type` fields, background image via `.dd2vtt` import or paste-to-canvas (spec 002, backed by RustFS asset storage).

- [~] **Phase 4: Token Creation** — Partial
  - Users can create tokens of different types (NPC, player, vehicle, etc.).
  - Different token types should have distinct visual representations.
  - Users can add tokens to a scene.
  - Implemented: token placement/movement on a scene, `actor_id` binding to an actor. Spec 004 unified the token backing store onto the scene-scoped `tokens` table (ADR-040), added per-player ownership/primary-token/photo/health columns, and gated player- vs. GM-initiated moves through separate mutations (`move_own_token` vs. `update_token`). Spec 006 (complete) replaced spec 004 US2's keyboard-shortcut resize/rotate stand-in with real canvas-rendered drag handles (GM-only, whole-grid-cell resize + continuous rotate, mirroring `WallPlugin`'s handle pattern) and fixed the ownership-assignment Popover-hang bug that had spec 004's player-owned-token e2e test skipped — that test is now un-skipped and passing. **Missing**: no distinct token "type" field or type-specific visual representation (`npc`/`vehicle`-style differentiation is not implemented — every token appears to render the same way regardless of what it represents). **Resolved since**: the legacy parallel `world_tokens` RxDB sync path previously noted here is gone — RxDB was hard-cut from this codebase, and neither `startWorldSync` nor any `world_tokens` reference remains anywhere in `apps/web/src` (verified 2026-08-30). `WorldPage.tsx` now drives only the per-scene sync.

- [~] **Phase 5: Actor Stats and Customization** — Partial
  - Users can add stats and customizations to "Actors" (e.g., health).
  - Actors are bound to tokens.
  - This phase introduces more video game-like logic.
  - Implemented: `world_actors`/`world_actor_system_data` tables (game-system-defined stat storage), actor-token binding via `tokens.actor_id`. Spec 010 added a dedicated `/world/:id/actor/:id/{view,edit}` UI (actor label editing, PC/NPC flag) and a DM-facing NPC catalog/creation screen at `/world/:id/staging`, plus the ability to share an actor via a link and deep-copy it (including cascaded `world_actor_system_data`) into another of the viewer's own worlds.

  **The 2026-08-22 "unverified" note has now been checked (2026-08-30), and it splits in two.** Half is closed and half is not.

  **Closed — actor stats reach the canvas.** Spec 029 made a token's resources visible without a click: the engine draws them as bars above the token (`src/engine/src/plugins/status_display.rs`), and the selected token's are shown in a viewer-chosen screen corner (`apps/web/src/components/StatusPanel/StatusPanel.tsx`). Which resources exist is declared by the game system's `system.json`, not hard-coded — four real declarations ship (Genie, D&D 5e, Pathfinder 2e, Blades in the Dark). Coarsening for viewers who are not entitled to exact figures happens server-side (`src/server/src/status_display.rs`), so a withheld value never reaches the client at all. Proven end to end by `apps/web/e2e/status-display.spec.ts`, which drives a Genie character's health and wish points through to both the token bars and the panel DOM. Documented in `docs/status-displays.md`.

  **Closed — the derived-statistics subsystem now actually executes.** `src/engine/src/derived_data.rs` has been registered in the frame loop (`src/engine/src/lib.rs`) the whole time and had **never run on a single real token**: `calculate_derived_stats` queries `(&Token, &mut DerivedStats)`, and no spawned entity carried `Token` — the only construction of that type anywhere was a unit test. Tokens now spawn with both components attached, so it computes for real entities rather than for nobody.

  **Still open — nothing acts on a computed stat.** `DerivedStats` does have a `movement_speed` field, but it is set to a hard-coded `Some(30)` for every token and **nothing anywhere reads it**: there is no movement gating on a computed value (`grep movement_speed` finds only the definition, the two writes and a test). `TokenAbilities` is also still never populated — tokens spawn with `abilities: Default::default()` because the server does not send ability scores, so `calculate_ability_stats` (which queries `Changed<TokenAbilities>`) continues to match nothing, and every ability-derived figure (AC, initiative, proficiency) is a constant. Spec 029 puts movement gating explicitly out of scope; it belongs to Phase 8's rule enforcement, not here. So the "stats feed back into token _behavior_" half of the original question is unanswered because it is unbuilt, not because it is unaudited.

- [x] **Phase 6: Walls and Lighting** — Done
  - Users can add walls and lighting to a scene.
  - These elements should restrict token vision.
  - Implemented: wall/door/light-source data model, 2D vision occlusion, hand-drawn wall/shape authoring directly on the canvas (specs 001, 002). `specs/003-dd2vtt-map-fidelity/` closed the round-trip trust gap: automated tests (`src/server/src/map_import.rs`) now re-query the DB after import (and after hand-built edits on top of an import) and assert exact field equality against the source, across the richest real fixtures — proving no wall/light/background data is silently lost or altered by a reload. It also added import-response `warnings` disclosure for previously-silent field categories (freestanding portals, non-default `ambient_light`, `objects_line_of_sight`). **Gap found during that pass, now closed (verified 2026-08-30)**: the note here said no live GraphQL subscription transport was wired client-side anywhere in the app, so a wall/light change from one session reached an already-connected second session only after a page reload. `apps/web/src/pages/world/WorldPage.tsx` now opens the `worldEventsCreated` subscription once per mounted scene and feeds `applyWallWorldEvent`, `applyTokenWorldEvent`, `applyShapeWorldEvent`, `applyLightWorldEvent` and `applyTokenStatusWorldEvent` from it — one shared subscription rather than four, since each applier filters by its own event code. The backend transport (Postgres listener → broadcast channel → GraphQL subscription → `/api/ws`) already existed in full; this was the first thing in `apps/web` to actually open it. **Residual, cosmetic**: the doc comments in `apps/web/src/engine/world/sync/{walls,lights,shapes}.ts` still say no subscription transport is wired and are now stale — they describe the code as it was, and should be corrected the next time those files are touched.

- [ ] **Phase 7: Scene Levels** — Not started
  - Users can add levels to a scene (e.g., upstairs, downstairs).
  - Each level can have its own set of walls and token assignments.
  - No "level"/multi-level concept exists anywhere in the schema or engine.

- [~] **Phase 8: Game System Integration** — Partial
  - The application should enforce the rules of the game system loaded onto the world.
  - This includes basic mechanics like movement speed, considering actor specs.
  - This is where the Bevy engine should be utilized.
  - Implemented: game system package install/manifest-serving pipeline (`src/server/src/systems.rs`), a system registry with several real systems registered (Genie, D&D 5e, Pathfinder 2e, Blades in the Dark), and derived-data plumbing in the engine (`src/engine/src/derived_data.rs`). **Missing**: full rule enforcement — movement is not gated by computed speed, and actor specs do not drive in-engine behaviour end to end.

  **Read this before starting Phase 8** (found 2026-08-30, while checking what blocks per-viewer visibility). The obvious first task looks like "populate `TokenAbilities` from actor data, so `derived_data.rs` stops matching nothing". Doing that as written would entrench a bug rather than fix one.

  `TokenAbilities` is six fixed fields — strength through charisma — and `derived_data.rs` computes on them with formulas its own comments label as D&D 5e: `compute_armor_class` is `10 + (dex - 10) / 2`, `compute_initiative` is the 5e dexterity modifier, `compute_movement_speed` ignores both its arguments and returns 30. That is one game system's rules compiled into the engine, which is the thing Principle I and spec 029's FR-001 both forbid, and it is currently harmless only because nothing populates it. Plumbing abilities through would make it load-bearing.

  It also does not survive contact with the systems already registered. Blades in the Dark has no ability scores at all — it has action ratings (Hunt, Prowl, Skirmish), twelve of them, on a 0–3 scale. There is no dexterity to read and no AC to derive. A six-field struct cannot represent that, and a system-agnostic engine should not be trying to.

  Spec 029 solved the same problem for resources and the answer should carry over: the **system manifest declares the field mapping**, the server resolves it against the actor's stored data, and the engine renders or applies what it is given while understanding none of it. `ResourceSource`/`EntrySource` in `crates/thunderforge-canvas-core/src/resource_display.rs` are the working precedent, and `packs/systems/*/system.json` already carries four systems' worth of declarations in that shape.

  So Phase 8's real first task is deciding where system rules execute — server, engine, or declared data — not wiring six D&D fields into a struct that already exists. That decision is ADR-shaped (Principle IV).

- [x] **Phase 9: Multiplayer** — Done
  - The owner of a world can invite other players via an invite code or a shareable link.
  - Invited players can join the world.
  - Players can select their "player" type actor as their character.
  - The Game Master (GM) should be able to override character selection.
  - Implemented: invite-code generation, `joinWorld` mutation, `world_members`/`world_invites` tables (`src/server/src/graphql/mutations_invites.rs`). **Unverified**: character-selection override by the GM specifically was not audited in this pass.

- [~] **Phase 10: Permissions Model** — Partial
  - A robust permissions model should be implemented.
  - The Dungeon Master (DM) can edit policies for different roles (player, trusted player, assistant DM).
  - The DM can promote other players to owner.
  - Implemented: a fixed three-tier role model (Owner/GM/Player) via `world_members`, with an `updateMemberRole` mutation that lets an Owner/GM change any member's role — including promoting someone to Owner. Spec 010 added a real, per-object permissions layer on top of this: every actor now has an "ownership block" (`world_actor_permissions`) letting the DM grant any world member Viewer/Editor/Owner on any actor (PC or NPC), with the DM implicitly retaining full control regardless of the block's contents and a default-Viewer fallback for members with no explicit grant. This is enforced server-side (`auth/actor_permissions.rs`, gating `updateActorSystemData` and `moveOwnToken`), with a DM-only editing UI at `/world/:id/actor/:id/edit`. **Missing**: this per-actor model is not yet a general "policy" system — there is still no "trusted player"/"assistant DM" world-wide role beyond the fixed three, and no equivalent ownership-block mechanism for other content types (scenes, maps, items) yet. The vestigial `policies`/`permission_grants` schema table remains dead code as before. (Same class of gap as documented in `docs/SECURITY_RBAC.md`.)

## Post-MVP

- **Metered and constrained connections:** Explicitly **not in MVP scope**, and planned.
  - Nothing in ThunderForge is currently tested against a metered, capped, or
    slow connection. The client fetches world content ahead of need
    (spec 028's cache and its background prefetch), which is the right trade
    on an unmetered link and an untested one on a metered link. There is no
    setting today to hold that back, and adding one is a real product
    decision rather than a switch — it is on the list, not built.
  - **Recommended baseline: an unmetered connection of at least 50 Mbps.**
    This figure is **UNTESTED**. It is a starting recommendation, not a
    measurement, and it is written that way deliberately: this document
    already carries one number that drifted 16% before anyone noticed, and an
    unlabelled guess is how that happens. Treat it as the number we would
    start from, and expect it to move once it has been measured.
  - What _has_ been measured is the first load, and it bounds the guess from
    below: 4.15MB brotli for the engine bundle (see the load-time entry
    below), plus a world's art. Steady-state play is small — token moves and
    dice, over one WebSocket.
  - **If you play on something slower, on a metered link, or on mobile data,
    we want to hear about it.** That evidence is worth more than our
    estimate, and there is nowhere in this project it can come from except
    people who actually did it:
    [the discussions](https://github.com/ThunderForgeVTT/ThunderForgeVTT/discussions).
- **Sharing and Federation:** Started for actors only.
  - Spec 010 added actor-level sharing: any actor can be shared via a link (`/shared/actor/:code`), previewed read-only by anyone, and deep-copied ("Copy to World") into one of the viewer's own worlds, fully independent of the source (including cascaded ability/item/lore data).
  - Not yet generalized to other content types (game systems, scenes, maps) — `specs/010-world-staging-actors/spec.md`'s Assumptions section explicitly flags this as expected future work, not built yet.
  - No cross-instance federation (talking to other ThunderForge servers) exists.
- **Marketplace:** Not started.
  - A marketplace where users can upload and share their creations.
- **Engine WASM bundle size / load-time:** Measured and largely addressed 2026-08-26. The dev-profile bundle had reached 220,099,904 bytes (~210MiB) — the earlier "~190MB (confirmed 2026-08-21)" figure had drifted ~16% unnoticed, which is itself worth knowing.
  - **What the 210MB actually was**: 71.3% of it (157MB) is the wasm `name` custom section — unmangled Rust/Bevy symbol names — not code. There are no DWARF sections at all. That is why it gzipped 10:1, and why the number was always more alarming than the program it described.
  - **Measured** (`wasm-pack build --release`, no source changes):

    | Build                    |      Raw | gzip -9 | brotli -q11 |
    | ------------------------ | -------: | ------: | ----------: |
    | dev (was shipping)       | 220.1 MB | 21.1 MB |           — |
    | release + `wasm-opt -O`  |  24.7 MB |  6.7 MB |     4.15 MB |
    | release + `wasm-opt -Oz` |  21.0 MB |  6.6 MB |    4.152 MB |

    8.9x smaller raw, 5.1x on the wire. `-Oz` is not worth it: 3.7MB less raw for **1,861 bytes** less brotli, at ~6 extra minutes of optimizer time. Stay on wasm-pack's default `-O`.

  - **Done**: `scripts/shared.mjs` now selects the profile per caller — the dev loop keeps `--dev` (a 7-minute rebuild after every engine edit is not a dev loop), everything else defaults to `--release`. `ENGINE_PROFILE=dev|release` overrides. The profile participates in the `pkg.sum` cache key, without which switching profiles silently skips the rebuild and serves whichever bundle was already on disk.
  - **Done**: the server was serving the wasm **uncompressed** — `tower-http` had no compression feature and no `CompressionLayer` existed. That was a ~6x gap on first-load bytes on top of the release win, and the number that actually governs a real player's first load. Now brotli + gzip.
  - **Done, and separate from the size of the bundle**: the load is no longer silent. Spec 028's User Story 6 shipped byte-level progress (`FR-030`/`FR-031`), a loading state inside a second, and a real explanation with a working retry when the download or the startup fails — pinned by `apps/web/e2e/engine-loading.spec.ts`. Worth stating plainly because the two are easy to conflate: **feedback is not size**. A 4.15MB brotli first load that reports itself honestly is a different problem from a 4.15MB first load, and only the first one is closed. The bundle-size items below remain open on their own terms.
  - **Still open — per-pack lazy loading**: WASM has no dynamic code-splitting analogous to JS's `import()`. A Cargo workspace split alone won't shrink the single shipped `.wasm` unless each piece compiles to its own binary, fetched and instantiated independently over a stable JS-glue boundary. The natural seam is `packs/systems/*/engine` — a world needs only its one active system's engine code. Real architectural work, and at 4.15MB brotli the case for it is now much weaker than it looked.
  - **Still open — Bevy feature trim**: `bevy_ui`/`bevy_ui_render` are now unused (the one-line debug HUD that needed them was removed) but deliberately retained; `webp` appears unreferenced in the engine crate; `bevy_gizmos` is diagnostic-only and could be feature-gated. None measured. Behind the two changes above in value.
