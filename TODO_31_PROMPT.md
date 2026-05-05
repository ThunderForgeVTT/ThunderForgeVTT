# Phase 3.1 — UI Redesign & Ownership UX Prompt

**Goal:** Implement a focused Phase 3.1 UI and UX refinement that complements Phase 3 world creation and the new pack/system work. This prompt is repo‑aware and actionable: generate frontend components, GraphQL/backend hooks, small migrations, ADRs, and integration notes. Do **not** implement chat yet — add it to the MVP timeline and scaffold only where necessary.

---

## High‑level goals
- Move **Data Ownership** and **Privacy** controls into the **User Profile** UI.
- Add **Scene Editor Notes** backed by local Git commit messages (temporary local git integration).
- Remove the “Future phase integration” section from UI/Docs.
- Add **Avatar Exporter** per world and a **World Management Portal** for GMs and players with clear ownership rules.
- Implement **actor ownership transfer** on assignment/acceptance.
- Improve **World List** management UX (manage button, edit, delete with typed confirmation + copy button).
- Minimize in‑game UI: popovers, contextual controls, minimap upper‑right, hide workspace header, settings submenu for “Return to dashboard”.
- Reposition heavy UI (chat, actors, scenes, layers) to upper‑left; party roster as hoverable compact UI.
- Add data correlation hooks between TLDR/Draw and Bevy engine (API contract only).
- Produce ADRs and acceptance criteria; deliverables and checklist.

---

## Deliverables (what to produce)
- Frontend component scaffolds and pages (TSX + SCSS) with file paths.
- GraphQL schema additions and resolver stubs.
- Small SQL migrations for `owner_id` and `actor_assignments`.
- Axum endpoints for avatar export and local git commits.
- ADRs 026–030 (full documents).
- Integration notes for TLDR/Draw ↔ Bevy correlation hooks.
- Tests stubs and acceptance checklist.

---

## Frontend: pages, components, and behavior

### User Profile
**File:** `apps/web/src/pages/profile/DataOwnershipPanel.tsx`  
- Export data button → calls GraphQL `exportMyData` and downloads JSON/ZIP.  
- Delete account button → calls GraphQL `deleteMyData` with confirmation flow.  
- Privacy toggles and list of owned objects (worlds, tokens, events, policies).

### Scene Editor Notes (local git commits)
**File:** `apps/web/src/components/SceneEditor/CommitNotes.tsx`  
- Dropdown of recent local git commit messages for the world path.  
- Backend endpoint: `GET /api/git/commits?path=<world_path>` (dev/local only).  
- Attach selected commit message to scene note; persist as `world_events` with `type = "scene_note"`.

### World Management Portal
**File:** `apps/web/src/pages/world/WorldManagePage.tsx` (`/world/:id/manage`)  
Sections:
- Overview: name, description, game system, interface pack.
- Permissions: players, DM(s), assistant DMs, ownership toggles.
- Avatar Exporter: export avatars for this world (ZIP).
- Danger Zone: delete world (type world name to confirm). Provide a **copy** button next to world name.

Inline edit controls for name/description; delete modal requires exact name typed; copy button copies world name to clipboard.

### World List (GM controls)
**File:** `apps/web/src/pages/world/WorldListPage.tsx`  
- Show **Manage** button for worlds where user is GM/owner.  
- Manage button links to `/world/:id/manage`.

### World Play / Game Engine UI
**File:** `apps/web/src/pages/world/WorldPlayPage.tsx` (`/world/:id/play`)  
- Full‑screen engine canvas; hide workspace header; show world name only.  
- **Minimap**: fixed upper‑right (`apps/web/src/components/Minimap/Minimap.tsx`).  
- **Upper‑left**: heavy UI area collapsed into `UpperLeftPanel` (actors, scenes, layer management).  
- **Party Roster**: hoverable compact UI (`apps/web/src/components/PartyRoster/PartyRoster.tsx`) showing assigned actors, players, GM(s).  
- **Settings mini submenu**: gear icon with “Return to dashboard” hidden inside.  
- Contextual popovers for controls; keyboard accessible.

### Avatar Exporter
**File:** `apps/web/src/components/World/AvatarExporter.tsx`  
- Calls backend `POST /api/worlds/:id/export-avatars` to download ZIP of avatars + metadata.

### TLDR/Draw ↔ Bevy correlation hooks (API contract only)
**File:** `apps/web/src/lib/engine/correlation.ts`  
- Expose `postEngineEvent(event)` and `subscribeToEngineEvents()` for future sync.  
- GraphQL subscription stub: `engineEvents(worldId)`.

---

## Backend: GraphQL, endpoints, and data rules

### Ownership & actor transfer rules
- Ensure `world_actors` includes `created_by`, `owner_id` (nullable), `created_at`, `updated_at`.
- New table `actor_assignments` to track pending transfers.

**SQL migration (example):**
```sql
ALTER TABLE world_actors ADD COLUMN owner_id UUID NULL;

CREATE TABLE actor_assignments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  actor_id UUID NOT NULL REFERENCES world_actors(id),
  assigned_to UUID NOT NULL REFERENCES users(id),
  assigned_by UUID NOT NULL REFERENCES users(id),
  status TEXT NOT NULL CHECK (status IN ('pending','accepted','revoked')),
  created_at TIMESTAMP DEFAULT now()
);
```

### Assignment workflow
- `assignActorToPlayer(actorId, playerId)` — DM only; creates pending assignment.
- `acceptActorAssignment(assignmentId)` — player only; transfers `owner_id` to player and logs event in `world_events`.
- `revokeActorAssignment(assignmentId)` — DM only; cancels pending assignment.

Ownership rules:
- Owners may mutate objects they own.
- DMs and assistant DMs may mutate any object in their world.
- Players cannot mutate objects they do not own.

### Endpoints
- `POST /api/worlds/:id/export-avatars` — returns ZIP of avatars (DMs and owners only).
- `POST /api/worlds/:id/manage/delete` — typed confirmation; server verifies typed name; logs deletion event; cascades deletes per Phase 1 rules.
- `GET /api/git/commits?path=<path>` — returns recent commit messages for that path (dev/local only; admin/dev mode).

### GraphQL additions (stubs)
- Mutations:
  - `assignActorToPlayer(actorId: ID!, playerId: ID!): Assignment!`
  - `acceptActorAssignment(assignmentId: ID!): Actor!`
  - `revokeActorAssignment(assignmentId: ID!): Boolean!`
- Queries:
  - `actorAssignments(worldId: ID!): [Assignment!]!`
- Subscriptions (future):
  - `assignmentNotifications(userId: ID!): AssignmentNotification!`

---

## UX rules, micro‑interactions, and accessibility
- Delete confirmation requires exact world name typed; provide copy button.
- Minimized controls: hidden by default; appear on hover/selection; keyboard accessible.
- Popovers: ARIA roles; dismiss on ESC.
- Focus states: visible gold outline (fantasy theme).
- Notifications: small toasts for assignment requests, acceptances, export completion.
- Mobile: party roster and upper‑left panel collapse into bottom sheet.

---

## ADRs to generate
- **ADR‑026** — UI Minimalism & Full‑Screen Engine Policy  
- **ADR‑027** — Actor Ownership Transfer Policy  
- **ADR‑028** — World Management UX & Delete Confirmation Policy  
- **ADR‑029** — Scene Editor Notes via Local Git (temporary)  
- **ADR‑030** — Pack & World Avatar Export Policy  

Each ADR must include: context, decision, consequences, alternatives, migration impact, and security/privacy implications. Place ADRs in `docs/adr/`.

---

## Acceptance criteria & checklist

**Frontend**
- [ ] Data Ownership panel in user profile calls `exportMyData` / `deleteMyData`.
- [ ] Scene Editor Notes UI shows recent local git commit messages (dev/local).
- [ ] World Manage page with edit, avatar exporter, permissions, and delete confirmation + copy button.
- [ ] World List shows Manage button for GMs/owners.
- [ ] World Play page hides workspace header, shows world name, minimap upper‑right, upper‑left hover panel, party roster hover, contextual popovers.
- [ ] Lazy UI controls appear on selection/hover; keyboard accessible.

**Backend**
- [ ] `owner_id` added to `world_actors`.
- [ ] `actor_assignments` table created.
- [ ] GraphQL mutations: `assignActorToPlayer`, `acceptActorAssignment`, `revokeActorAssignment`.
- [ ] Endpoints: `POST /api/worlds/:id/export-avatars`, `POST /api/worlds/:id/manage/delete`, `GET /api/git/commits`.
- [ ] Ownership transfer logic implemented and tested.

**Security & Privacy**
- [ ] Only authorized users (DMs, assistant DMs) can assign actors.
- [ ] Ownership transfer logged in `world_events`.
- [ ] Delete world requires typed confirmation and server verification.
- [ ] Export endpoints only accessible to owners/DMs.

**Docs & ADRs**
- [ ] ADRs 026–030 created in `docs/adr/`.
- [ ] Integration notes for TLDR/Draw ↔ Bevy correlation hooks.
- [ ] MVP timeline updated to include chat as future item (scaffold only).

---

## Integration notes (TLDR/Draw ↔ Bevy)
- Implement a lightweight event bus and GraphQL subscription `engineEvents(worldId)` for future sync.  
- Provide client hooks `postEngineEvent(event)` and `subscribeToEngineEvents()` that map TLDR/Draw annotations to engine events.  
- This phase only defines the API contract and client stubs; full sync is a future phase.

---

## Implementation guidance & file map (quick reference)

**Frontend**
- `apps/web/src/pages/profile/DataOwnershipPanel.tsx`  
- `apps/web/src/components/SceneEditor/CommitNotes.tsx`  
- `apps/web/src/pages/world/WorldManagePage.tsx`  
- `apps/web/src/pages/world/WorldListPage.tsx`  
- `apps/web/src/pages/world/WorldPlayPage.tsx`  
- `apps/web/src/components/Minimap/Minimap.tsx`  
- `apps/web/src/components/PartyRoster/PartyRoster.tsx`  
- `apps/web/src/components/World/AvatarExporter.tsx`  
- `apps/web/src/lib/engine/correlation.ts`

**Backend**
- Migrations: add `owner_id` and `actor_assignments` (SQL files under `migrations/`)  
- GraphQL schema/resolvers: `assignActorToPlayer`, `acceptActorAssignment`, `revokeActorAssignment`  
- Axum endpoints: `/api/worlds/:id/export-avatars`, `/api/worlds/:id/manage/delete`, `/api/git/commits`

**Docs**
- `docs/adr/026-ui-minimalism.md` … `docs/adr/030-avatar-export.md`  
- Integration notes: `docs/integration/tldr-bevy.md`

