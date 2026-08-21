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
  - Implemented: username/password auth, OAuth providers, two-factor auth, session management, admin bootstrap (`src/server/src/auth/`, `src/server/src/users/`).

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
  - Implemented: token placement/movement on a scene, `actor_id` binding to an actor. **Missing**: no distinct token "type" field or type-specific visual representation was found (`npc`/`vehicle`-style differentiation is not implemented — every token appears to render the same way regardless of what it represents).

- [~] **Phase 5: Actor Stats and Customization** — Partial

  - Users can add stats and customizations to "Actors" (e.g., health).
  - Actors are bound to tokens.
  - This phase introduces more video game-like logic.
  - Implemented: `world_actors`/`world_actor_system_data` tables (game-system-defined stat storage), actor-token binding via `tokens.actor_id`. **Unverified**: depth of actor customization UI and whether stat changes actually feed back into token behavior (e.g. movement, combat) was not audited in this pass — needs its own follow-up check before marking Done.

- [x] **Phase 6: Walls and Lighting** — Done

  - Users can add walls and lighting to a scene.
  - These elements should restrict token vision.
  - Implemented: wall/door/light-source data model, 2D vision occlusion, hand-drawn wall/shape authoring directly on the canvas (specs 001, 002). `specs/003-dd2vtt-map-fidelity/` is closing the remaining trust gap: proving imported map data survives persistence with no silent loss.

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
