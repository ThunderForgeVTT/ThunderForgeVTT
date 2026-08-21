# MVP 1 Roadmap

This document outlines the 10 phases for reaching the Minimum Viable Product (MVP) 1 for ThunderForgeVTT.

**Status as of 2026-08-21**: This checklist had never been updated since it was written — every box was still unchecked despite several phases being substantially implemented. Statuses below were verified against the actual codebase (not assumed), and are noted `[x] Done`, `[~] Partial` (with what's missing), or `[ ] Not started`. See `specs/001-bevy-canvas-authoring/`, `specs/002-canvas-authoring-asset-storage/`, and `specs/003-dd2vtt-map-fidelity/` for the specs that delivered the completed phases.

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
  - Implemented: token placement/movement on a scene, `actor_id` binding to an actor. Spec 004 unified the token backing store onto the scene-scoped `tokens` table (ADR-040), added per-player ownership/primary-token/photo/health columns, and gated player- vs. GM-initiated moves through separate mutations (`move_own_token` vs. `update_token`). Spec 006 (complete) replaced spec 004 US2's keyboard-shortcut resize/rotate stand-in with real canvas-rendered drag handles (GM-only, whole-grid-cell resize + continuous rotate, mirroring `WallPlugin`'s handle pattern) and fixed the ownership-assignment Popover-hang bug that had spec 004's player-owned-token e2e test skipped — that test is now un-skipped and passing. **Missing**: no distinct token "type" field or type-specific visual representation (`npc`/`vehicle`-style differentiation is not implemented — every token appears to render the same way regardless of what it represents); a legacy parallel `world_tokens` RxDB sync path (`engine/world/sync/index.ts#startWorldSync`) is still wired into `WorldPage.tsx` alongside the modern per-scene sync and should be investigated/retired in a follow-up (found during spec 004, not fixed).

- [~] **Phase 5: Actor Stats and Customization** — Partial

  - Users can add stats and customizations to "Actors" (e.g., health).
  - Actors are bound to tokens.
  - This phase introduces more video game-like logic.
  - Implemented: `world_actors`/`world_actor_system_data` tables (game-system-defined stat storage), actor-token binding via `tokens.actor_id`. **Unverified**: depth of actor customization UI and whether stat changes actually feed back into token behavior (e.g. movement, combat) was not audited in this pass — needs its own follow-up check before marking Done.

- [x] **Phase 6: Walls and Lighting** — Done

  - Users can add walls and lighting to a scene.
  - These elements should restrict token vision.
  - Implemented: wall/door/light-source data model, 2D vision occlusion, hand-drawn wall/shape authoring directly on the canvas (specs 001, 002). `specs/003-dd2vtt-map-fidelity/` closed the round-trip trust gap: automated tests (`src/server/src/map_import.rs`) now re-query the DB after import (and after hand-built edits on top of an import) and assert exact field equality against the source, across the richest real fixtures — proving no wall/light/background data is silently lost or altered by a reload. It also added import-response `warnings` disclosure for previously-silent field categories (freestanding portals, non-default `ambient_light`, `objects_line_of_sight`). **New gap found during this pass, not yet fixed**: `apps/web/src/engine/world/sync/{walls,lights,shapes}.ts` all self-document that no live GraphQL subscription transport is wired client-side anywhere in the app — a wall/light property change from one session does not reach an already-connected second session without a page reload (confirmed via a reproducible failing Playwright test, `apps/web/e2e/map-editor-tooling.spec.ts`). This means FR-005-style "live sync to a connected session" claims hold only for a fresh page load, not a truly persistent connection — worth a dedicated follow-up feature (a client-side GraphQL subscription client) rather than a patch within this spec's scope.

- [ ] **Phase 7: Scene Levels** — Not started

  - Users can add levels to a scene (e.g., upstairs, downstairs).
  - Each level can have its own set of walls and token assignments.
  - No "level"/multi-level concept exists anywhere in the schema or engine.

- [~] **Phase 8: Game System Integration** — Partial

  - The application should enforce the rules of the game system loaded onto the world.
  - This includes basic mechanics like movement speed, considering actor specs.
  - This is where the Bevy engine should be utilized.
  - Implemented: game system package install/manifest-serving pipeline (`src/server/src/systems.rs`), a system registry with at least one real system registered (D&D 5e), and derived-data/movement-speed plumbing in the engine (`src/engine/src/derived_data.rs`). **Missing/unverified**: full rule enforcement (e.g. movement actually gated by computed speed, actor specs driving in-engine behavior end-to-end) was not confirmed in this pass.

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
  - Implemented: a fixed three-tier role model (Owner/GM/Player) via `world_members`, with an `updateMemberRole` mutation that lets an Owner/GM change any member's role — including promoting someone to Owner. **Missing**: the fine-grained, DM-editable "policy" system this phase originally described does not exist — a `policies`/`permission_grants`-style table exists in the schema but is vestigial (its model struct is commented out, and it's read only for an admin stats counter, never for real authorization decisions). There is no "trusted player"/"assistant DM" role beyond the fixed three, and no policy-editing UI or mutation. (This is the same class of gap documented in `docs/SECURITY_RBAC.md` — a similarly-named `world_collaborators`/`RbacEngine` system was found to be entirely dead code, never compiled, and was deleted; `world_members`'s simpler three-role model is what's actually live.)

## Post-MVP

- **Sharing and Federation:** Not started.
  - Users can create and share their own game systems, tokens, maps, etc. via a share code.
  - The system should be able to talk to other systems to share content (federation).
- **Marketplace:** Not started.
  - A marketplace where users can upload and share their creations.
- **Engine WASM bundle size / load-time:** Not started. The dev-served `dist/engine/engine_bg.wasm` is ~190MB (confirmed 2026-08-21) — every full page reload in e2e (and every real player's first load) re-instantiates it from scratch, which is now the dominant cost driving several e2e test timeouts up into the 3-8 minute range. Two independent options, not mutually exclusive:
  - **Build profile**: that 190MB figure is from an unoptimized `dev` profile build (`wasm-pack`'s default in this repo's `scripts/build.mjs`, no `wasm-opt`, full debug info). Switching the served bundle to `--release` + `wasm-opt` requires no restructuring and is likely the highest-leverage first step — worth measuring before anything below.
  - **Per-pack lazy loading**: WASM has no dynamic code-splitting analogous to JS's `import()` — a Cargo workspace split alone (e.g. separating `packs/systems/dnd5e/engine` further) won't shrink the single shipped `.wasm` unless each piece is actually compiled to its own wasm binary and fetched/instantiated independently at runtime, communicating with the host/core engine over a stable JS-glue boundary. The natural seam for this is the existing pack-system boundary (`packs/systems/*/engine`) — a given world only needs its one active game system's engine code, not every installed system's — but this is a real architectural change (host interface design, lazy-instantiation lifecycle) and should follow, not precede, the release-profile measurement above.
