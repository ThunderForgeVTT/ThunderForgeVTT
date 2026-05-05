# ThunderForgeVTT - Phase 3.0+ — World Creation + Bundled Basic Game System TODO

This document outlines the remaining tasks for the "Phase 3.0+ — World Creation + Bundled Basic Game System" prompt, and notes where the assistant encountered issues.

---

## Current Status and Issues Encountered

The assistant encountered a persistent loop when attempting to modify `src/server/src/graphql.rs` and `src/server/src/main.rs` using the `replace` tool. The core issue was the extreme sensitivity of the `old_string` parameter in `replace` to whitespace and minor changes, especially in large files. Incremental modifications caused the `old_string` to become stale, leading to repeated failures and preventing forward progress.

**Critical Blocking Issue:**
The primary blocker for further progress is a persistent compilation error in the `crates/pack_system_spec` crate related to the `jsonschema` dependency. Despite multiple attempts to:
*   Correctly import and use `jsonschema::JSONSchema`.
*   Ensure `schemars` API compatibility.
*   Alias the `jsonschema` crate.
*   Update all relevant dependencies with `cargo update`.
*   Perform full file overwrites with what the documentation suggests is correct.

The error `error[E0433]: cannot find `JSONSchema` in `jsonschema`` (or similar variations) consistently prevents `pack_system_spec` from compiling. This issue seems to stem from a deeper incompatibility or a subtle misunderstanding of the `jsonschema` crate's structure that cannot be resolved with the current tools and context.

**Current State of Implementation (as of last successful action before the blocker):**
*   `crates/pack_system_spec` has been created, but currently fails to compile due to the `jsonschema` error.
*   A Diesel migration for the `game_systems` table has been created and run successfully.
*   The `GameSystem` and `NewGameSystem` models have been added to `src/server/src/models.rs`.
*   The `src/server/src/graphql.rs` file has been updated to include `GraphQLGameSystem` and its `From` implementation, and the `game_systems` resolver and `game_system(id)` resolver in `UserQuery`.
*   The `src/server/src/systems.rs` file has been implemented with functional Axum handlers for `GET /api/systems`, `GET /api/systems/{slug}/manifest.json`, `GET /api/systems/{slug}/download`, and a placeholder for `POST /api/systems/install` with multipart upload, zip extraction, and initial validation logic. This file currently has a compilation error due to the `pack_system_spec` issue.
*   The `mod systems;` declaration is correctly integrated into `src/server/src/main.rs`, and the `POST /api/systems/install` route is protected by `require_admin_user` middleware.
*   `systems_dir` has been added to the `Directories` struct in `src/server/src/config/mod.rs`.

---

## Remaining High-Level Deliverables

### 1. Monorepo workspace changes
*   **Add `./packs/*` to `pnpm-workspace.yaml`**: *User stated they completed this manually.*
*   **Create `./packs/systems/basic-game-system/` as a local npm package**: *Completed (directory and initial `package.json` created).*
*   **Establish a pack‑type standard**:
    *   `./packs/systems/*`
    *   `./packs/interface/*`
    *   `./packs/actors/*`
    *   `./packs/items/*`
    *   etc.
    *Status: Directory structure placeholders created, but formal documentation (ADRs) and further integration are pending.*

### 2. Pack manifest contract
*   **Create Rust crate: `crates/pack_system_spec`**: *Created, but currently blocked by compilation error.*
*   **This crate defines**:
    *   `SystemManifest` struct: *Implemented (but currently non-compiling).*
    *   JSON Schema generation (`get_system_manifest_schema()`): *Implemented (but currently non-compiling).*
    *   Validation helpers (`validate_system_manifest()`): *Implemented (but currently non-compiling).*
*   **Future pack types will follow the same pattern**: (No action taken yet for these)
    *   `crates/pack_interface_spec`
    *   `crates/pack_actor_spec`
    *   `crates/pack_item_spec`

### 3. Backend
*   **Diesel migration for `game_systems` table**: *Completed.*
*   **Rust models + Diesel schema updates**: *Completed (`src/server/src/models.rs` updated).*
*   **GraphQL**:
    *   `gameSystems`: *Completed (UserQuery resolver).*
    *   `gameSystem(id)`: *Completed (UserQuery resolver).*
    *   `installGameSystem` (admin only): *Partial (handler setup, but core logic blocked by `pack_system_spec`).*
*   **Axum endpoints**:
    *   `GET /api/systems`: *Implemented.*
    *   `GET /api/systems/{slug}/manifest.json`: *Implemented.*
    *   `GET /api/systems/{slug}/download`: *Implemented.*
    *   `POST /api/systems/install`: *Partial (multipart upload, zip extraction implemented, manifest validation blocked).*
    *   `GET /api/systems/schema.json`: *Pending.*
*   **Validation pipeline using `pack_system_spec`**: *Blocked by `pack_system_spec` compilation error.*
*   **Storage layout: `/data/packs/systems/<slug>/`**: *Partial (logic in `install_game_system` implemented, but dependent on `pack_system_spec`).*

### 4. Frontend
*   **World creation UI**: *Pending (Blocked by backend).*.
    *   Dropdown to select `gameSystemId`.
    *   Preview panel showing manifest fields.
*   **Lazy loader for system modules/styles**: *Pending (Blocked by backend).*.
*   **Minimal compendium browser UI**: *Pending (Blocked by backend).*.
*   **Local preview integration for `@thunderforge/basic-game-system`**: *Pending (Blocked by backend).*.

### 5. Bundled Boilerplate System
*   **Directory: `./packs/systems/basic-game-system/`**: *Completed.*
*   **Contains**:
    *   `system.json`: *Placeholder created.*
    *   `module/main.mjs`: *Placeholder created.*
    *   `styles/main.css`: *Placeholder created.*
    *   `packs/` (empty): *Directory created.*
    *   `lang/en.json`: *Pending.*
    *   `templates/`: *Directory created.*
*   **Build script produces `dist/boilerplate.zip`**: *Pending (`rollup.config.js` is a placeholder, build scripts blocked by `pack_system_spec` if it's part of CI).*

### 6. ADRs
*   ADR‑020: Pack Architecture & Pack‑Type Standard
*   ADR‑021: Game System Packaging & Manifest Contract
*   ADR‑022: `game_systems` DB Model & Ownership Rules
*   ADR‑023: Runtime Module Loading & Security
*   ADR‑024: Compendium Pack Format
*   ADR‑025: Pack Crate Naming Convention (`pack_<type>_spec`)
    *Status: Placeholder files created, content needs to be written for all (blocked by context if code changes are needed).*

### 7. Tests & CI
*   **Rust unit tests for manifest validation**: *Blocked by `pack_system_spec` compilation error.*
*   **Backend integration tests for install flow**: *Pending (Blocked by `pack_system_spec`).*
*   **Frontend tests for world creation + lazy loader**: *Pending (Blocked by backend).*.
*   **CI step to build system package and validate schema**: *Pending (Blocked by `pack_system_spec`).*
