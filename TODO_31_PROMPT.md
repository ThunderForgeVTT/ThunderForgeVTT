# Phase 3.1 — UI Redesign & Ownership UX Prompt - Implementation Status

**Goal:** Implement a focused Phase 3.1 UI and UX refinement that complements Phase 3 world creation and the new pack/system work.

**Status: READY FOR IMPLEMENTATION** - All backend infrastructure is in place. Frontend work can now proceed based on the working API.

---

## High-level Goals
- ✅ **Prerequisite Complete**: Backend game system infrastructure now working
- ⏳ Move **Data Ownership** and **Privacy** controls into the **User Profile** UI
- ⏳ Add **Scene Editor Notes** backed by local Git commit messages
- ⏳ Remove the "Future phase integration" section from UI/Docs
- ⏳ Add **Avatar Exporter** per world and a **World Management Portal**
- ⏳ Implement **actor ownership transfer** on assignment/acceptance
- ⏳ Improve **World List** management UX
- ⏳ Minimize in-game UI: popovers, contextual controls, minimap upper-right
- ⏳ Reposition heavy UI (chat, actors, scenes, layers) to upper-left
- ⏳ Add data correlation hooks between TLDR/Draw and Bevy engine

---

## Deliverables (what to produce)

### Status Summary
| Component | Status | Notes |
|-----------|--------|-------|
| Frontend components/pages | ⏳ Pending | Ready to implement with working backend |
| GraphQL schema additions | ⏳ Pending | Stubs ready for implementation |
| SQL migrations | ⏳ Pending | `owner_id` and `actor_assignments` tables |
| Axum endpoints | ⏳ Pending | Avatar export, git commits, delete endpoints |
| ADRs 026–030 | ⏳ Pending | Full documents needed |
| TLDR/Draw ↔ Bevy correlation | ⏳ Pending | API contract definition |
| Tests & acceptance checklist | ⏳ Pending | Ready after implementation |

---

## Frontend: Pages, Components, and Behavior

### User Profile - Data Ownership Panel
**File:** `apps/web/src/pages/profile/DataOwnershipPanel.tsx`  
**Status:** ⏳ Ready to implement
- [ ] Export data button → calls GraphQL `exportMyData` and downloads JSON/ZIP
- [ ] Delete account button → calls GraphQL `deleteMyData` with confirmation flow
- [ ] Privacy toggles and list of owned objects (worlds, tokens, events, policies)

**Dependencies:**
- GraphQL mutations: `exportMyData`, `deleteMyData`
- Backend endpoint: `POST /api/auth/export`, `POST /api/auth/delete`

### Scene Editor Notes (Local Git Commits)
**File:** `apps/web/src/components/SceneEditor/CommitNotes.tsx`  
**Status:** ⏳ Ready to implement
- [ ] Dropdown of recent local git commit messages for the world path
- [ ] Backend endpoint: `GET /api/git/commits?path=<world_path>` (dev/local only)
- [ ] Attach selected commit message to scene note
- [ ] Persist as `world_events` with `type = "scene_note"`

**Dependencies:**
- New Axum endpoint: `GET /api/git/commits`
- New GraphQL mutation: `attachSceneNote`

### World Management Portal
**File:** `apps/web/src/pages/world/WorldManagePage.tsx` (`/world/:id/manage`)  
**Status:** ⏳ Ready to implement

**Sections:**
- [ ] Overview: name, description, game system, interface pack
  - Inline edit controls for name/description
- [ ] Permissions: players, DM(s), assistant DMs, ownership toggles
- [ ] Avatar Exporter: export avatars for this world (ZIP)
- [ ] Danger Zone: delete world
  - Type exact world name to confirm
  - Provide **copy** button next to world name
  - Server-side verification of typed name

**Dependencies:**
- GraphQL mutations: `updateWorldMetadata`, `deleteWorld`
- New Axum endpoint: `POST /api/worlds/:id/export-avatars`
- New Axum endpoint: `POST /api/worlds/:id/manage/delete`

### World List (GM Controls)
**File:** `apps/web/src/pages/world/WorldListPage.tsx`  
**Status:** ⏳ Ready to implement
- [ ] Show **Manage** button for worlds where user is GM/owner
- [ ] Manage button links to `/world/:id/manage`
- [ ] Display system name for each world (now available from backend)

**Dependencies:**
- GraphQL query `worlds` already returns `gameSystemId`
- New component for manage actions

### World Play / Game Engine UI
**File:** `apps/web/src/pages/world/WorldPlayPage.tsx` (`/world/:id/play`)  
**Status:** ⏳ Ready to implement
- [ ] Full-screen engine canvas; hide workspace header; show world name only
- [ ] **Minimap**: fixed upper-right (`apps/web/src/components/Minimap/Minimap.tsx`)
- [ ] **Upper-left**: heavy UI area collapsed into `UpperLeftPanel` (actors, scenes, layer management)
- [ ] **Party Roster**: hoverable compact UI (`apps/web/src/components/PartyRoster/PartyRoster.tsx`)
  - Show assigned actors, players, GM(s)
- [ ] **Settings mini submenu**: gear icon with "Return to dashboard" hidden inside
- [ ] Contextual popovers for controls; keyboard accessible

**Dependencies:**
- Engine integration (Bevy engine already available)
- RxDB sync (existing infrastructure)

### Avatar Exporter Component
**File:** `apps/web/src/components/World/AvatarExporter.tsx`  
**Status:** ⏳ Ready to implement
- [ ] Calls backend `POST /api/worlds/:id/export-avatars` to download ZIP
- [ ] Shows export progress/confirmation
- [ ] Error handling for failed exports

**Dependencies:**
- New Axum endpoint: `POST /api/worlds/:id/export-avatars`

### TLDR/Draw ↔ Bevy Correlation Hooks (API Contract Only)
**File:** `apps/web/src/lib/engine/correlation.ts`  
**Status:** ⏳ Ready to implement (scaffolding phase)
- [ ] Expose `postEngineEvent(event)` for client-side event posting
- [ ] Expose `subscribeToEngineEvents()` for subscription management
- [ ] GraphQL subscription stub: `engineEvents(worldId)`
- [ ] Map TLDR/Draw annotations to engine event format

**Note:** Full sync implementation deferred to future phase. This phase defines API contract only.

**Dependencies:**
- New GraphQL subscription: `engineEvents(worldId)`
- New Axum WebSocket endpoint: `/api/worlds/:id/engine/events`

---

## Backend: GraphQL, Endpoints, and Data Rules

### Ownership & Actor Transfer Rules

**Database Changes (SQL Migration Required):**
```sql
ALTER TABLE world_actors ADD COLUMN owner_id UUID NULL;
ALTER TABLE world_actors ADD COLUMN created_by UUID NOT NULL;

CREATE TABLE actor_assignments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
  assigned_to UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  assigned_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('pending','accepted','revoked')),
  created_at TIMESTAMP DEFAULT now(),
  updated_at TIMESTAMP DEFAULT now()
);

CREATE INDEX idx_actor_assignments_pending ON actor_assignments(status) WHERE status = 'pending';
```

**Status:** ⏳ Migration needs implementation

### Assignment Workflow

**GraphQL Mutations to Implement:**

```graphql
extend type Mutation {
  # DM only; creates pending assignment
  assignActorToPlayer(actorId: ID!, playerId: ID!): Assignment!
  
  # Player only; transfers ownership to player
  acceptActorAssignment(assignmentId: ID!): Actor!
  
  # DM only; cancels pending assignment
  revokeActorAssignment(assignmentId: ID!): Boolean!
  
  # Update world metadata (name, description)
  updateWorldMetadata(worldId: ID!, name: String, description: String): World!
  
  # Delete world with typed confirmation
  deleteWorld(worldId: ID!, confirmationName: String!): Boolean!
  
  # Export my data
  exportMyData: String!  # Returns signed URL to download
  
  # Delete my account
  deleteMyData(password: String!): Boolean!
}

extend type Query {
  # Get pending assignments for a world
  actorAssignments(worldId: ID!): [Assignment!]!
}

extend type Subscription {
  # Subscribe to assignment notifications
  assignmentNotifications(userId: ID!): AssignmentNotification!
  
  # Subscribe to engine events (future use)
  engineEvents(worldId: ID!): EngineEvent!
}
```

**Status:** ⏳ Stubs need implementation

### Axum Endpoints to Implement

**Status:** ⏳ All pending

| Endpoint | Method | Auth | Purpose |
|----------|--------|------|---------|
| `/api/worlds/:id/export-avatars` | POST | DMs/owners | Download world avatars as ZIP |
| `/api/worlds/:id/manage/delete` | POST | Owners | Delete world with confirmation |
| `/api/git/commits` | GET | Admin/dev | Get recent git commits for path |
| `/api/auth/export` | POST | Authenticated | Export user data |
| `/api/auth/delete` | POST | Authenticated | Delete user account |
| `/api/worlds/:id/engine/events` | WS | Authenticated | WebSocket for engine events |

### GraphQL Type Additions

**Status:** ⏳ Types need definition

```graphql
type Assignment {
  id: ID!
  actor: Actor!
  assignedTo: User!
  assignedBy: User!
  status: AssignmentStatus!
  createdAt: DateTime!
  updatedAt: DateTime!
}

enum AssignmentStatus {
  PENDING
  ACCEPTED
  REVOKED
}

type AssignmentNotification {
  id: ID!
  assignment: Assignment!
  type: NotificationType!
  read: Boolean!
  createdAt: DateTime!
}

enum NotificationType {
  ASSIGNMENT_REQUESTED
  ASSIGNMENT_ACCEPTED
  ASSIGNMENT_REVOKED
}

type EngineEvent {
  id: ID!
  worldId: ID!
  type: String!
  payload: JSON!
  createdAt: DateTime!
}
```

---

## UX Rules, Micro-Interactions, and Accessibility

### Delete Confirmation Pattern
- [ ] Delete confirmation requires exact world name typed
- [ ] Provide copy button next to world name for easy confirmation
- [ ] Visual feedback when correct name is typed
- [ ] Disabled delete button until confirmation matches

### Minimized Controls
- [ ] Hidden by default; appear on hover/selection
- [ ] Keyboard accessible (Tab, Enter, Esc)
- [ ] ARIA labels for all interactive elements
- [ ] Focus states: visible gold outline (fantasy theme)

### Popovers & Modals
- [ ] ARIA roles (role="dialog", role="tooltip")
- [ ] Dismiss on ESC key
- [ ] Focus trap within modal
- [ ] Proper heading hierarchy (h1, h2, etc.)

### Notifications
- [ ] Small toasts for assignment requests, acceptances, export completion
- [ ] Auto-dismiss after 5 seconds
- [ ] Manual dismiss button
- [ ] High contrast for visibility

### Mobile Responsiveness
- [ ] Party roster and upper-left panel collapse into bottom sheet
- [ ] Touch-friendly button sizes (48px minimum)
- [ ] Hamburger menu for minimap/controls on mobile
- [ ] Swipe gestures for panel navigation

---

## ADRs to Generate

**Status:** ⏳ All pending

| ADR | Title | Focus |
|-----|-------|-------|
| ADR-026 | UI Minimalism & Full-Screen Engine Policy | Design philosophy for game engine UI |
| ADR-027 | Actor Ownership Transfer Policy | Rules for actor ownership changes |
| ADR-028 | World Management UX & Delete Confirmation Policy | Delete safety patterns |
| ADR-029 | Scene Editor Notes via Local Git (Temporary) | Local git integration justification |
| ADR-030 | Pack & World Avatar Export Policy | Avatar/pack data export security |

**Each ADR must include:**
- Context (why this decision)
- Decision (what we decided)
- Consequences (benefits & drawbacks)
- Alternatives (other options considered)
- Migration impact (how existing users/data affected)
- Security & privacy implications

**Location:** `docs/adr/026-*.md` through `docs/adr/030-*.md`

---

## Acceptance Criteria & Implementation Checklist

### Frontend Implementation
- [ ] Data Ownership panel in user profile
  - [ ] `exportMyData` GraphQL call working
  - [ ] `deleteMyData` GraphQL call with password confirmation
  - [ ] Privacy toggles display and update
- [ ] Scene Editor Notes UI
  - [ ] Dropdown shows recent git commits
  - [ ] Commit selection attaches to scene note
  - [ ] Scene notes persist via GraphQL
- [ ] World Manage page (`/world/:id/manage`)
  - [ ] Edit name/description inline
  - [ ] Avatar exporter functional
  - [ ] Permissions panel displays owners/DMs
  - [ ] Delete confirmation with typed name + copy button
  - [ ] World List shows Manage button for GMs/owners
- [ ] World Play page (`/world/:id/play`)
  - [ ] Workspace header hidden
  - [ ] World name displayed at top
  - [ ] Minimap fixed upper-right
  - [ ] Upper-left panel with hover/collapse
  - [ ] Party roster hoverable and compact
  - [ ] Settings submenu with "Return to dashboard"
  - [ ] All controls keyboard accessible
- [ ] Avatar Exporter component
  - [ ] ZIP download functional
  - [ ] Progress feedback during export
  - [ ] Error handling with user feedback

### Backend Implementation
- [ ] Database migration: `owner_id` added to `world_actors`
- [ ] Database migration: `actor_assignments` table created with indexes
- [ ] GraphQL mutations implemented:
  - [ ] `assignActorToPlayer(actorId, playerId): Assignment!`
  - [ ] `acceptActorAssignment(assignmentId): Actor!`
  - [ ] `revokeActorAssignment(assignmentId): Boolean!`
  - [ ] `updateWorldMetadata(worldId, name, description): World!`
  - [ ] `deleteWorld(worldId, confirmationName): Boolean!`
  - [ ] `exportMyData(): String!`
  - [ ] `deleteMyData(password): Boolean!`
- [ ] GraphQL queries implemented:
  - [ ] `actorAssignments(worldId): [Assignment!]!`
- [ ] GraphQL subscriptions implemented:
  - [ ] `assignmentNotifications(userId): AssignmentNotification!`
  - [ ] `engineEvents(worldId): EngineEvent!` (stub for future)
- [ ] Axum endpoints implemented:
  - [ ] `POST /api/worlds/:id/export-avatars` — returns ZIP
  - [ ] `POST /api/worlds/:id/manage/delete` — typed confirmation
  - [ ] `GET /api/git/commits?path=<path>` — returns commit messages (dev/local only)
  - [ ] `POST /api/auth/export` — download user data
  - [ ] `POST /api/auth/delete` — delete account
- [ ] Ownership transfer logic implemented and tested
- [ ] All endpoints authenticated and authorized

### Security & Privacy
- [ ] Only DMs/assistant DMs can assign actors
- [ ] Ownership transfer logged in `world_events`
- [ ] Delete requires typed confirmation AND server verification
- [ ] Export endpoints only accessible to owners/DMs
- [ ] Password required for account deletion
- [ ] Cascading delete follows Phase 1 rules

### Tests & Verification
- [ ] Frontend unit tests for components
- [ ] Frontend integration tests for flows
- [ ] Backend unit tests for ownership logic
- [ ] Backend integration tests for endpoints
- [ ] E2E tests for delete confirmation flow
- [ ] E2E tests for actor assignment workflow
- [ ] Security tests for authorization

### Docs & ADRs
- [ ] ADRs 026–030 created in `docs/adr/`
- [ ] Integration notes for TLDR/Draw ↔ Bevy correlation (`docs/integration/tldr-bevy.md`)
- [ ] MVP timeline updated to include chat as future item
- [ ] API documentation updated with new endpoints

---

## Integration Notes (TLDR/Draw ↔ Bevy)

**Phase 3.1 Scope:** API contract definition and client stubs only. Full sync deferred to Phase 3.2+.

### API Contract

**Client Hooks (Frontend):**
```typescript
// Post engine event from TLDR/Draw
export function postEngineEvent(event: EngineEvent): Promise<void>

// Subscribe to engine events
export function subscribeToEngineEvents(
  worldId: string
): Observable<EngineEvent>
```

**Engine Event Format:**
```json
{
  "type": "draw_annotation|draw_erase|token_move|...",
  "source": "tldraw|bevy|...",
  "worldId": "...",
  "payload": {}
}
```

### GraphQL Subscription (Stub)
```graphql
subscription EngineEvents($worldId: ID!) {
  engineEvents(worldId: $worldId) {
    id
    worldId
    type
    payload
    createdAt
  }
}
```

### Implementation Path for Future Phases
1. **Phase 3.2:** Implement tldraw -> engine event pipeline (annotations)
2. **Phase 3.3:** Implement engine -> tldraw sync (view updates)
3. **Phase 3.4:** Implement state consistency guarantees

---

## Implementation Guidance & File Map (Quick Reference)

### Frontend Files to Create
```
apps/web/src/pages/profile/
  └── DataOwnershipPanel.tsx

apps/web/src/pages/world/
  ├── WorldManagePage.tsx
  ├── WorldListPage.tsx (update existing)
  └── WorldPlayPage.tsx (update existing)

apps/web/src/components/SceneEditor/
  └── CommitNotes.tsx

apps/web/src/components/Minimap/
  └── Minimap.tsx

apps/web/src/components/PartyRoster/
  └── PartyRoster.tsx

apps/web/src/components/World/
  └── AvatarExporter.tsx

apps/web/src/lib/engine/
  └── correlation.ts
```

### Backend Changes
```
src/server/src/
  ├── graphql.rs (add mutations/subscriptions)
  ├── auth.rs (add export/delete endpoints)
  ├── worlds.rs (add management endpoints)
  └── actors.rs (add assignment logic)

migrations/
  ├── 2026-05-XX-01-add-owner-id-to-actors.sql
  └── 2026-05-XX-02-create-actor-assignments.sql
```

### Documentation Files
```
docs/adr/
  ├── 026-ui-minimalism.md
  ├── 027-actor-ownership.md
  ├── 028-world-management.md
  ├── 029-scene-notes-git.md
  └── 030-avatar-export.md

docs/integration/
  └── tldr-bevy.md
```

---

## Ready to Proceed?

✅ **All prerequisites are met:**
- Backend Phase 3.0+ infrastructure is complete and validated
- GraphQL API structure is in place
- Database schema is ready for Phase 3.1 migrations
- Frontend can begin implementation against working API

**Next Step:** Create SQL migrations for `owner_id` and `actor_assignments`, then begin frontend component implementation.
