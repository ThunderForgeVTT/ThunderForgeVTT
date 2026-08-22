# Feature Specification: Seamless Sign-Up-to-Canvas Onboarding Flow

**Feature Branch**: `008-seamless-onboarding-flow`

**Created**: 2026-08-21

**Status**: Draft

**Input**: User description: "plan on a spec to overhaul the ui ux from sign up until they land on our canvas? making it more seamless?" — grounded in a code-level audit of the current flow: register → `/welcome` (identical copy for new and returning users) → `/worlds` → `/worlds/create` (two non-functional placeholder dropdowns) → `/world/:id` dashboard (mostly placeholder panels) → `/world/:id/play` (silent, feedback-free WASM engine load) → a mandatory "New scene" modal (every fresh world starts with zero scenes) before anything renders. Five navigations, two forms, one modal, two dead dropdowns, and one broken CTA (a "Join via Invite Code" button on `/welcome` that links to the unrelated demo page instead of the already-working `/join/:code` invite-redemption flow) stand between finishing sign-up and seeing the actual product.

## Clarifications

### Session 2026-08-21

- Q: Should a brand-new world get a default first scene automatically, so there's never a mandatory "create a scene" step before a new user sees the canvas? → A: Yes — world creation auto-generates one starter scene, so the canvas already has something on it the instant a user enters. Explicit scene creation still exists for adding further scenes later; it's just never a forced first step.
- Q: Does the World Dashboard screen stay in the primary new-world path, or does world creation go straight to the canvas? → A: Skipped for new worlds — creating a world takes the user straight to its canvas. The dashboard still exists and remains reachable later (e.g. from the worlds list) for things like renaming or reviewing a world; it's just no longer a forced stop on first entry.
- Q: What happens to the non-functional game-system and interface-pack dropdowns on the create-world form? → A: Removed from this flow — world creation only asks for what's actually usable today (name, description). Game-system/interface-pack selection becomes a later, separate configuration step once that functionality is real, not part of this onboarding path.
- Q: Does a brand-new user (zero worlds) skip the landing/hub screen entirely and go straight into creating their first world, or do they still see a landing screen with "Create your first world" as one option to click? → A: Skip straight to world creation — a user with zero worlds never sees a hub screen at all; their first authenticated view is the create-world form directly.
- Q: For a returning user who has exactly one existing world, does their landing screen auto-take them straight into that world, or always show a hub screen with a one-click shortcut to it? → A: Always show a hub with a one-click shortcut, regardless of world count — consistent behavior, and keeps a natural place to also surface invite-code entry (US2 AC3).
- Q: What's the concrete target funnel size for SC-001's "meaningfully fewer steps" claim? → A: 1 form (register) + 1 form (create world, for a zero-world user with no hub stop) + canvas with visible loading feedback + rendered scene. Total: 2 forms, 0 modals, 0 dead-end dashboard stop — down from today's 2 forms + 1 modal + 5 navigations.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A new user reaches their world's canvas quickly, with honest feedback the whole way (Priority: P1)

Someone who just created an account wants to get from "I signed up" to "I can see and use the game canvas" with the fewest possible detours, and wants to know something is happening at every point that takes more than an instant — never staring at a screen wondering if it's broken.

**Why this priority**: This is the core value of the feature. Every other improvement in this spec matters less if the primary path is still long and silent.

**Independent Test**: Time and count the screens/clicks/forms a fresh account must pass through between finishing registration and seeing their world's canvas rendered with content on it; confirm no point in that path shows a blank or static screen for more than a couple of seconds without a visible indication that something is loading.

**Acceptance Scenarios**:

1. **Given** a user has just finished creating an account and has zero existing worlds, **When** they land on their first post-registration screen, **Then** that screen IS the create-world form directly — no landing/hub screen in between — and from there they go straight to a rendered canvas, with no intermediate dashboard stop and no separate scene-creation step. Total: 2 forms (register, create-world), 0 modals, 0 dead-end dashboard — down from today's 2 forms + 1 modal + 5 navigations.
2. **Given** a user has created a world and chosen to enter it, **When** the game engine is downloading and starting up, **Then** they see a visible loading indicator with status information the entire time, never a static, unexplained background.
3. **Given** a user enters a freshly created world for the first time, **When** the canvas becomes ready, **Then** there is already something meaningful rendered on it — the world's auto-created default scene — without the user being forced through a separate, unexplained "create a scene" step first.
4. **Given** the engine fails to load (a real error, e.g. a network failure), **When** that happens, **Then** the user sees a clear error state with a way to retry — never a silent blank screen indistinguishable from "still loading."

---

### User Story 2 - Nothing in the flow looks configurable or actionable when it isn't (Priority: P1)

A user moving through account creation and world setup wants every control, dropdown, and button they see to actually do something. Placeholder UI that looks real but silently does nothing (or links somewhere unrelated) undermines trust in the whole product before they've even started using it.

**Why this priority**: Independently valuable even on its own — a user who hits a dead control right after signing up forms an immediate impression that the product is unfinished or broken, regardless of how fast the rest of the flow is.

**Independent Test**: Walk through account creation and world setup as a new user, interacting with every visible control; confirm every one of them either does something real or is not shown at all — none present a false impression of functionality.

**Acceptance Scenarios**:

1. **Given** a user is creating a new world, **When** they view the creation form, **Then** every field shown (name, description) has a real effect on the world being created — the non-functional game-system/interface-pack selectors are not present.
2. **Given** a user opens their world's dashboard (now reached only after a world already exists, not as a forced creation-time stop), **When** they view it, **Then** every panel shown reflects real, current information about that world — no panel exists purely as an unfilled placeholder for a future feature.
3. **Given** a user wants to join a world via an invite code, **When** they look for that option from their landing screen, **Then** they can enter a code and be taken into that world — not redirected to an unrelated demo page.

---

### User Story 3 - Returning users get a landing experience distinct from first-time users (Priority: P2)

A user who has already been using the product and logs back in wants to land somewhere useful for continuing their existing work — not be shown first-time "getting started" framing, and not be greeted with copy that assumes a prior visit ("Welcome back") on what is actually their very first one.

**Why this priority**: A real but smaller polish item — the flow already functions for both audiences today, this is about each seeing the right framing rather than one being misled either direction.

**Independent Test**: Create a brand-new account and confirm its first landing screen never claims a return visit; separately, log in as an existing user with prior worlds and confirm their landing screen surfaces their existing worlds rather than first-time getting-started framing.

**Acceptance Scenarios**:

1. **Given** a user has just created their account for the first time and has zero worlds, **When** they land on their first authenticated screen, **Then** that screen is the create-world form itself (per US1 AC1) — there is no landing/hub screen for this case, so no "returning user" copy can appear on it at all.
2. **Given** a user has logged in before and owns or belongs to at least one existing world, **When** they land on their post-login screen, **Then** they always see a hub screen listing their world(s) as one-click shortcuts into each — consistently shown regardless of how many worlds they have (including exactly one), never an automatic direct entry that skips the hub.

---

### Edge Cases

- What happens when a user's very first world-creation attempt fails (e.g. a name collision or a transient server error)? They must land back on a form with their input preserved and a clear error, not lose their progress and start over blind.
- What happens when a user abandons the flow partway (e.g. closes the tab while the engine is loading) and returns later? Re-entering the same world should resume at a sensible point (the canvas, or the loading state again), not force them back through world creation.
- What happens when a user has an invite code for a world but no account yet? The path from "I have a code" to "I'm in the world" should not require them to separately discover registration on their own with no link back to redeeming the code afterward.
- What happens when a returning user has zero existing worlds (e.g. every world they had access to was deleted, or they were only ever invited and removed)? Per FR-010, they take the same zero-worlds path as a brand-new user — straight to the create-world form, no hub screen, no empty "your worlds" section to explain.
- What happens on a very slow connection where the engine load genuinely takes much longer than usual? The loading state must not imply a fixed, short duration in a way that reads as broken/stuck once real time exceeds it.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST take a user with zero existing worlds directly from registration to the create-world form, with no landing/hub screen in between, reducing the funnel to 2 forms (register, create-world), 0 modals, and 0 dead-end dashboard stop between finishing registration and having a rendered, populated canvas — compared to today's baseline (register → welcome → worlds list → create-world form → world dashboard → play → new-scene modal).
- **FR-001a**: System MUST always show a landing/hub screen (listing the user's world(s) as one-click shortcuts) to any authenticated user who has at least one existing world, regardless of how many worlds they have — including exactly one — rather than auto-entering a world on their behalf.
- **FR-002**: System MUST show a visible, honest loading indicator with status information for the entire duration of the game engine's startup, replacing the current static, unindicated background.
- **FR-003**: System MUST show a clear, actionable error state (not a blank or static screen) if the game engine fails to load.
- **FR-004**: System MUST auto-create one default scene as part of world creation, so a user entering a freshly created world always finds meaningful content already rendered on the canvas — "create a scene" MUST NOT appear as a mandatory, separately-surfaced step interrupting first entry. Explicit scene creation remains available afterward for adding further scenes.
- **FR-005**: System MUST NOT present the current non-functional game-system and interface-pack selectors on the world-creation form — they are removed from this flow entirely (world creation asks only for a name and description, the two fields that already have real effect); game-system/interface-pack selection becomes a later, separate configuration step once that functionality is real.
- **FR-006**: System MUST take a user straight from successful world creation to that world's canvas — the World Dashboard screen MUST NOT be a forced stop in the new-world path. The dashboard remains reachable afterward (e.g. from the worlds list) for reviewing or managing an existing world, and every panel it shows MUST continue to reflect real, current data — no panel may exist purely as an unfilled placeholder.
- **FR-007**: System MUST provide a working invite-code redemption path reachable from wherever a user currently lands: the hub screen for anyone with at least one existing world (FR-001a), and a secondary "have an invite code instead?" option on the create-world form for a zero-worlds user (FR-001) who reaches that form without having followed a direct invite link. Entering a valid code MUST take the user into that world, not to an unrelated screen.
- **FR-008**: System MUST NOT display "returning user" framing (e.g. "Welcome back") anywhere in the zero-worlds create-world flow, since that flow (per FR-001) contains no landing/hub screen for such framing to appear on in the first place.
- **FR-009**: System MUST show any user with at least one existing world (new account that has since created one, or a long-time returning user — no distinction needed) a hub screen with a direct, one-click path into each of their world(s), per FR-001a.
- **FR-010**: System MUST route a returning user who currently has zero accessible worlds (e.g. all were deleted, or they were removed from every world they'd been invited to) through the same zero-worlds path as a brand-new user (FR-001) — straight to the create-world form, never an empty "your worlds" hub with no explanation.
- **FR-011**: System MUST preserve a user's in-progress input (e.g. a partially filled world-creation form) and show a clear, specific error message when a step in the flow fails, rather than discarding their progress.
- **FR-012**: System MUST allow a user with a valid invite code but no existing account to reach account registration and be returned to redeeming that code afterward, without losing the code along the way.
- **FR-013**: System MUST continue to enforce all existing world/scene ownership and authorization rules (unchanged from specs 001-006) for every screen and action introduced or modified by this flow — this feature changes navigation and presentation only, not who is allowed to do what.

### Key Entities *(include if feature involves data)*

- **World**: Existing entity (specs 001-006). This feature does not change its shape — only that world creation now also creates one default Scene in the same action, and that the dashboard is no longer the screen a user lands on immediately after creating one.
- **Scene**: Existing entity. This feature causes one Scene to be created automatically as part of world creation (FR-004), using the existing scene data model unchanged — no new fields.
- **User Session / Landing State**: Not a new persisted entity — the distinction between "first-time" and "returning" landing framing (US3) is derived from existing data (whether the user has any worlds), not a new stored flag.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A new (zero-world) user goes from finishing registration to a populated, interactive canvas in exactly 2 forms (register, create-world), 0 modals, and 0 dead-end dashboard stops — down from today's baseline of 2 forms + 1 modal + 5 navigations, measured end to end.
- **SC-002**: 100% of the engine's load duration is covered by a visible loading indicator — zero seconds of a static, unexplained screen during a successful load.
- **SC-003**: 100% of controls presented to a user during account creation and world setup produce a real effect when used — zero non-functional/placeholder controls remain visible in that path.
- **SC-004**: A user with a valid invite code reaches the target world in one continuous path, whether or not they already have an account.
- **SC-005**: Every account state with zero worlds (genuinely new, or returning-but-emptied) reaches the same zero-worlds create-world path with no hub screen and no "returning user" framing; every account state with one or more worlds (new-with-a-world, or long-time-returning) sees the same hub screen with one-click shortcuts — verified across all four states.
- **SC-006**: A user who hits an error at any step in the flow (world creation, engine load) can see what went wrong and retry without losing prior input, 100% of the time.

## Assumptions

- This feature changes navigation, presentation, and default content generation only — it does not introduce new backend data models or change existing ownership/authorization rules (specs 001-006 remain the authority for those).
- "Seamless" is defined here as: fewer required steps, continuous honest feedback during any wait, and no UI that misrepresents its own functionality — not as a single-page or wizard-style redesign of every screen involved; existing screens may be removed, merged, or reordered, but this spec does not mandate a specific new visual design system.
- The existing `/join/:code` invite-redemption flow (already functional per spec 005) is the target this feature wires the landing screen's invite-code entry into — this spec does not redesign invite redemption itself, only makes it reachable and correctly linked.
- Performance of the underlying WASM engine load itself (its actual download/boot time) is out of scope for this feature — this spec addresses the *feedback* shown during that wait, not the wait's duration. (Separate from the previously-noted engine-bundle-size backlog item in `MVP.md`'s Post-MVP section.)
- Mobile/responsive layout considerations for these screens are out of scope unless an existing screen already supports them — this feature does not introduce a new device-support requirement.
