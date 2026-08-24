# Feature Specification: Genie House System

**Feature Branch**: `018-genie-house-system`

**Created**: 2026-08-23

**Status**: Draft

**Input**: User description: "lets make genie real" — following on from a design discussion: an original ThunderForgeVTT-owned game system, called Genie, whose purpose is to exercise every facet of the engine (dice resolution, canvas/token topology, size/footprint scaling, conditions, world compendium/items, lore wiki, GM staging) so that playing or testing Genie on the `*/play` screens functions as a living, deterministic regression fixture for the engine itself — distinct from the six externally-licensed system digests, which exist to cover real-world player demand rather than engine coverage.

## Clarifications

### Session 2026-08-23

- Q: When players get 3 wishes each session, is that one shared pool of 3 for the whole table, or 3 each per player? → A: Shared pool — 3 wishes total for the whole party each session, spent by group agreement.
- Q: When a wish gets burned from the pool, is it a big narrative reality-bend separate from normal abilities, or extra fuel for the Manifestation roll/Wish Points? → A: Separate narrative resource — a Wish is a GM-adjudicated fiction-rewriting effect (undo a failed roll, reveal a clue, remove an obstacle), not a dice-mechanic power-up.
- Q: For the escape-room/clock pressure, one big shared countdown clock, or several smaller per-puzzle clocks raced in parallel? → A: Both — one overarching session Doom Clock plus smaller per-puzzle Progress Clocks, mirroring the Blades in the Dark digest's Progress Clock pattern.
- Q: What exactly makes the party win a session — clearing all Puzzle Clocks before the Doom Clock fills, or a distinct final "escape" step after? → A: Win = resolve all active Puzzle Clocks before the Doom Clock fills; no separate final step.
- Q: What are the actual tradeable/manageable resources that give this its Catan feel? → A: A small set of narrative resource types (e.g. Insight, Favor, Essence), gathered during scenes and tradeable between players, spent to advance specific Puzzle Clocks.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A roll exercises the full breadth of the dice engine in one formula (Priority: P1)

A player takes an action that requires a Manifestation roll: they roll a pool of d6s equal to their relevant skill rating, keep the top N dice, any 6 explodes into an additional die, and the total number of dice showing 4 or higher is the count of successes. This single roll type requires the dice engine (spec `014-dice-rolling-engine`) to correctly compose keep/drop, exploding, and success-counting in one formula — the three hardest dice-notation features to get right — rather than needing three separate licensed systems' rolls to exercise all three.

**Why this priority**: This is the single mechanic most directly responsible for Genie's reason to exist. Without a native system whose core roll requires all three dice-notation features at once, engine maintainers have to stitch together test cases across multiple licensed systems' digests to get the same coverage, and none of those digests were designed for this purpose — they describe someone else's game, not our test fixture.

**Independent Test**: As any player using the Genie system, trigger a Manifestation roll; confirm the resolved result reflects correct keep/drop selection, at least one exploded die when a 6 is rolled, and an accurate success count — verifiable via the same authoritative roll record spec 014 already defines, with no dependency on any other system pack being loaded.

**Acceptance Scenarios**:

1. **Given** a Genie character with a skill rating of 4, **When** they make a Manifestation roll keeping the top 3 dice, **Then** the roll record shows exactly 4 dice rolled, exactly 3 kept (marked `kept: true`, per spec 014's roll-record contract) and 1 dropped.
2. **Given** a roll where one or more kept dice show a 6, **When** the roll resolves, **Then** each such die's full reroll chain (the original 6 plus every explosion) is present in the roll record, and the die's final value is its *last* roll in that chain — the dice engine's existing, consistent explode semantics (spec 014): an exploded die's final value is not a sum of the chain, it's whatever the last roll landed on.
3. **Given** a fully resolved roll, **When** successes are counted, **Then** the result total equals the number of kept dice (including exploded chains) whose final value (the last roll in their chain, per Scenario 2) is 4 or higher.

---

### User Story 2 - A GM switches a scene between a measured grid and an abstract zone (Priority: P1)

A GM runs a Genie scene set in the "Material" — a normal measured grid, distance in feet — and later that session opens a scene where a character's wish has warped reality into a "Wish-Warped Zone" — an abstract, gridless space with no fixed distance unit, similar to Cypher System's range bands. The canvas/token engine must correctly render and measure both, and switching between them mid-session must not corrupt token positions or measurements from the scene that just ended.

**Why this priority**: None of the six previously-digested licensed systems require *both* a measured grid and a gridless abstraction to exist simultaneously in the same system — each one picks a single topology. Genie is deliberately the one system that needs both, which is the only way to force the canvas engine's topology-handling code to be exercised on both paths rather than one being assumed-correct because nothing ever tests the other.

**Why this priority (tie with US1)**: Equally foundational to Genie's purpose — a system that only exercised the dice engine and not the canvas/token engine would leave half the `*/play` surface uncovered.

**Independent Test**: As a GM, create a Genie world with one Material scene and one Wish-Warped Zone scene; confirm tokens measure and move correctly in each, and confirm switching a scene's topology setting does not corrupt or misplace existing token data.

**Acceptance Scenarios**:

1. **Given** a Material scene, **When** a GM places and moves a token, **Then** movement is measured in feet against a square grid, consistent with the system's declared grid topology.
2. **Given** a Wish-Warped Zone scene, **When** a GM places and moves a token, **Then** positioning is handled via the zone/abstract-band model with no grid or feet-based measurement involved.
3. **Given** a world containing both scene types, **When** a GM switches which scene is active, **Then** each scene's tokens retain their own topology-appropriate position data without cross-contamination.

---

### User Story 3 - A GM stages Genie NPCs whose size category changes their token footprint (Priority: P2)

A GM stages a Genie NPC — say, a minor sprite versus a towering elemental servant — whose size category (Diminutive through Colossal) determines how many grid squares its token occupies on a Material scene. Staging these NPCs (spec `009-gm-staging-page`, `010-world-staging-actors`) exercises the token-footprint/size-category path end-to-end, the same facet that came back only partially confirmed (`sourced: false`) for two of the six licensed digests.

**Why this priority**: Important engine coverage, but depends on User Story 2's grid topology existing first, and is scoped to the GM staging flow rather than the core play loop — a real capability, but one level down from the two P1 stories.

**Independent Test**: As a GM, stage two NPCs of different size categories in a Material scene; confirm each token's default footprint (number of occupied grid squares) matches its size category without manual adjustment.

**Acceptance Scenarios**:

1. **Given** a Diminutive-size Genie NPC, **When** staged on a Material scene, **Then** its token defaults to sharing a single grid square with room for others.
2. **Given** a Colossal-size Genie NPC, **When** staged on the same scene, **Then** its token defaults to occupying a multi-square footprint proportional to its size category.

---

### User Story 4 - A character's conditions are tracked during play (Priority: P2)

A player's Genie character becomes Bound, Exposed, or another Genie-specific condition during play, and that condition is visibly tracked on their character sheet and token, consistent with how conditions are expected to render across systems.

**Why this priority**: Conditions are a cross-cutting concern touched by combat, items, and GM tools alike — real coverage value, but it depends on a character/token already existing (Stories 1-2) and is not itself a distinct architectural facet the way topology or dice notation are.

**Independent Test**: As a player, apply a condition to a Genie character during a scene; confirm it displays on both the character sheet and the associated token, and confirms/clears correctly when removed.

**Acceptance Scenarios**:

1. **Given** an active Genie character, **When** a condition is applied, **Then** it appears on the character sheet's condition track and as a token status indicator.
2. **Given** a character with an active condition, **When** the condition is removed (by duration, action, or GM ruling), **Then** it clears from both the sheet and the token consistently.

---

### User Story 5 - A player manages wish-granted items with mechanical effects (Priority: P3)

A player acquires a wish-granted item (e.g., a Lamp of Minor Binding) that has a defined mechanical effect, adds it to their character's inventory, and the item's effect is available to reference during play — exercising the world item/inventory data model (spec `013-items-inventory`).

**Why this priority**: Real coverage of the items/inventory subsystem, but it's additive content rather than a foundational mechanic — a Genie character is fully testable without any items existing yet.

**Independent Test**: As a player, add a wish-granted item with a defined effect to a Genie character's inventory; confirm the item and its effect are visible and correctly associated with that character.

**Acceptance Scenarios**:

1. **Given** a Genie character, **When** a wish-granted item is added to their inventory, **Then** the item's name, description, and mechanical effect are all visible on the character's inventory view.

---

### User Story 6 - A character progresses via a leveled Wish Points table (Priority: P3)

As a Genie character advances in level, their Wish Points resource (structurally similar to a leveled table like the existing `spellSlots` pattern) increases according to a fixed by-level table, and any derived values depending on it recalculate correctly.

**Why this priority**: Exercises the progression/derived-data recalculation path, but is meaningful only after a character already exists and has already taken at least one action — the last piece of the coverage checklist rather than a foundational one.

**Independent Test**: As a player, level up a Genie character; confirm their Wish Points total updates to match the level-appropriate value from the system's table and any values derived from it recalculate.

**Acceptance Scenarios**:

1. **Given** a Genie character at a given level, **When** they level up, **Then** their Wish Points total updates to the new level's table value without manual entry.

---

### User Story 7 - The party plays a full co-op session against the Doom Clock (Priority: P1)

A group of players sits down for a Genie session. They start with a shared Session Wish Pool of 3 wishes, a session-wide Doom Clock, and one or more Puzzle Clocks representing the session's objectives (an escape-room-style structure). As the session unfolds, actions, failed rolls, and complications advance the Doom Clock; solving puzzles, succeeding at challenges, and managing resources advance individual Puzzle Clocks toward resolution. The party must negotiate, as a group, when a wish is worth spending to avert disaster or unlock progress — this is the primary, replayable game loop Genie is actually played for, not merely an engine-coverage exercise layered on top of a game.

**Why this priority**: Equal priority to US1 and US2 — this is the loop that makes the other mechanics (the Manifestation roll, dual topology, conditions, items, progression) matter to a player in the first place, and it's the story that makes Genie "more importantly a playable system," per the direction that prompted this clarification session. A Genie session without this loop is a set of disconnected mechanics being individually tested, not a game anyone would choose to play.

**Independent Test**: As a GM, start a Genie session with a fresh Session Wish Pool (3), a Doom Clock, and at least two Puzzle Clocks; play until either all Puzzle Clocks resolve (win) or the Doom Clock fills (loss); confirm the party had at least one genuine decision point about whether to spend a wish, and confirm the session's outcome matches FR-016's win/loss condition exactly.

**Acceptance Scenarios**:

1. **Given** a fresh session, **When** it begins, **Then** the Session Wish Pool shows 3 wishes, the Doom Clock shows zero segments filled, and all defined Puzzle Clocks show zero segments filled — all visible and live-synced to every connected player and the GM.
2. **Given** an in-progress session, **When** a failed roll or complication occurs, **Then** the Doom Clock advances by a GM-adjudicated number of segments.
3. **Given** an in-progress session, **When** the party succeeds at a challenge tied to a specific objective, **Then** that objective's Puzzle Clock advances, independently of the Doom Clock and any other Puzzle Clock.
4. **Given** a tense moment where a Puzzle Clock or the Doom Clock is close to a threshold, **When** the party discusses spending a wish, **Then** any player can propose it, but spending it requires party agreement (per FR-013) and produces a GM-adjudicated Wish Effect (per FR-014) rather than a dice roll.
5. **Given** two players holding different Session Resources, **When** they agree to trade, **Then** the trade completes and is visible live to the rest of the party and the GM (per FR-017/FR-018).
6. **Given** enough pooled Session Resources of the right type for a specific Puzzle Clock, **When** the party spends them, **Then** that Puzzle Clock advances accordingly.
7. **Given** all active Puzzle Clocks reach full resolution before the Doom Clock fills, **When** the last one resolves, **Then** the session ends in a win (per FR-016).
8. **Given** the Doom Clock fills completely before all Puzzle Clocks are resolved, **When** that happens, **Then** the session ends in a loss (per FR-016).

### Edge Cases

- What happens when a Manifestation roll's exploding chain produces an unusually long sequence of consecutive 6s? The dice engine's existing exploding-dice handling (spec 014) governs this; Genie does not impose an additional cap beyond whatever the engine itself defines.
- What happens when a scene's topology is changed while tokens are actively being moved (a live sync in progress)? The in-progress move should complete or cleanly cancel under the topology active at the time it started, per spec `005-live-canvas-sync`'s existing conflict-handling behavior — Genie does not introduce new conflict-resolution rules of its own.
- What happens when an NPC's size category would make its footprint exceed the scene's boundaries? Treated the same as any other oversized token placement in the existing canvas/token authoring specs (001-006) — Genie does not define special-case scene-boundary behavior.
- What happens when a condition is applied to a token in a Wish-Warped Zone scene, where there's no grid to render a positioned status icon against? The status indicator attaches to the token itself, not to a grid-relative position, so it renders identically regardless of active topology.
- What happens when the last active Puzzle Clock resolves in the same tick/action that would also fill the Doom Clock? Resolving the final Puzzle Clock takes precedence — per FR-016, the win condition ("all active Puzzle Clocks resolved") is checked, and satisfied, before the Doom Clock's fill state is evaluated as a loss.
- What happens when a player attempts to spend Session Resources they don't hold, or offer a trade for resources a counterpart doesn't have? Rejected client-side/server-side the same way any other insufficient-resource action is rejected elsewhere in the platform (no negative balances, no speculative trades) — Genie does not need a bespoke error-handling path beyond the existing resource-validation convention.
- What happens if the party disagrees about spending a wish (FR-013) or a pooled Session Resource contribution? Resolved the same way any other GM-adjudicated group decision is handled at the table — this spec does not impose a voting/consensus mechanism; "party agreement" is a social convention the GM enforces, not a system-enforced rule.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Genie system MUST define a core resolution mechanic (the Manifestation roll) expressible as a single dice formula requiring keep/drop selection, exploding dice, and success-counting simultaneously.
- **FR-002**: The Genie system MUST define two mutually exclusive per-scene spatial topologies — a measured square grid ("Material") and an abstract, gridless zone/range-band model ("Wish-Warped Zone") — selectable independently per scene within the same world.
- **FR-003**: The Genie system MUST define ability scores, skills linked to abilities, and resource pools structured compatibly with the existing actor system `data_types` contract used by other system packs.
- **FR-004**: The Genie system MUST define a leveled resource table (Wish Points) that scales by character level, structurally compatible with the existing leveled-table manifest pattern (e.g. `spellSlots`).
- **FR-005**: The Genie system MUST define a token size/scale category system (at minimum Diminutive through Colossal) that determines a token's default grid footprint on a Material scene.
- **FR-006**: The Genie system MUST define a condition/status-effect track applicable to both player characters and NPCs, renderable on both the character sheet and the associated token regardless of active scene topology.
- **FR-007**: The Genie system MUST define at least one equippable/usable item type with a defined mechanical effect, compatible with the existing world item/item-effects data model.
- **FR-008**: The Genie system MUST define NPC/monster stat blocks structurally distinct from player character sheets, suitable for the existing GM staging flow.
- **FR-009**: The Genie system MUST define a lore-linked concept (a Patron or genie lineage) that associates a character with an entry in the world's lore wiki.
- **FR-010**: The Genie system's manifest MUST include a complete `legal` object (per spec `016-system-pack-legal-compliance`) declaring the content as original, ThunderForgeVTT-owned material requiring no third-party attribution, license notice, or trademark restriction.
- **FR-011**: The Genie system MUST NOT depend on, reference, or require any externally-licensed SRD content to be playable — it is wholly original and self-contained.
- **FR-012**: A GM MUST be able to create a fully playable Genie character and run at least one complete combat encounter using only Genie system content, with no other system pack loaded in the same world.
- **FR-013**: The Genie system MUST define a Session Wish Pool — a single shared resource of 3 wishes available to the whole party at the start of each session, spent by group agreement rather than by an individual player unilaterally, and visible/synced live to every connected player and the GM.
- **FR-014**: Spending a wish from the Session Wish Pool MUST trigger a GM-adjudicated narrative effect (e.g. undoing a failed roll's consequence, revealing a hidden clue, removing an obstacle) distinct from — and not a substitute for — any Manifestation roll or Wish Points expenditure.
- **FR-015**: The Genie system MUST define a layered clock structure: one session-wide Doom Clock tracking overall failure pressure, plus one or more independent Puzzle Clocks scoped to a specific objective/station, both segmented and advanced the same way (structurally consistent with the Progress Clock pattern documented in the Blades in the Dark digest), visible and live-synced to the whole party.
- **FR-016**: The session's win condition MUST be exactly: every active Puzzle Clock is fully resolved before the Doom Clock fills completely, with no separate finale step required beyond that. Filling the Doom Clock completely before all Puzzle Clocks are resolved MUST trigger the session's loss condition.
- **FR-017**: The Genie system MUST define a small set of narrative Session Resource types (e.g. Insight, Favor, Essence) that party members gather during scenes, can trade with one another, and can spend (individually or pooled) to advance a specific Puzzle Clock — the resource-management/negotiation layer distinct from the Session Wish Pool.
- **FR-018**: Trading a Session Resource between players MUST be visible/synced live to every connected player and the GM, consistent with the Session Wish Pool's and clocks' live-sync requirement (FR-013, FR-015).

### Key Entities

- **Genie Character**: A player character sheet carrying abilities, skills, a Wish Points resource, conditions, inventory, and a linked Patron/lineage entry.
- **Genie NPC**: A GM-staged, non-player stat block structurally distinct from a Genie Character, carrying a size category that determines its token footprint.
- **Manifestation Roll**: The core resolution mechanic — a dice pool with keep/drop, exploding, and success-counting composed into a single formula.
- **Scene Topology**: A per-scene setting determining whether a scene uses the Material (grid) or Wish-Warped Zone (abstract) spatial model.
- **Wish Points Table**: The leveled resource table governing how a character's Wish Points scale with level.
- **Wish-Granted Item**: An inventory item with a defined mechanical effect, associated with a character's inventory.
- **Patron / Lineage**: A lore-wiki-linked entity representing the source of a character's Genie-granted capabilities.
- **Session Wish Pool**: A world/session-scoped shared resource (starting at 3 per session) spent by party agreement, distinct from a character's personal Wish Points — a new engine facet (shared, live-synced session state) beyond per-character resource data.
- **Wish Effect**: The GM-adjudicated narrative outcome triggered by spending a wish (undo a failure, reveal a clue, remove an obstacle) — not a dice roll or a Manifestation-roll bonus.
- **Doom Clock**: A single session-wide, live-synced segmented clock tracking overall failure pressure; filling it completely triggers the session's loss condition.
- **Puzzle Clock**: One of several independent, live-synced segmented clocks scoped to a specific objective/station; resolving one marks progress toward the session's win condition. Structurally the same underlying mechanic as the Doom Clock, just narrower in scope.
- **Session Resource**: One of a small set of narrative resource types (e.g. Insight, Favor, Essence) gathered by players during scenes, tradeable between them, and spent (individually or pooled) to advance a specific Puzzle Clock — the Catan-style negotiation layer, distinct from the Session Wish Pool.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every engine facet identified as in-scope for `*/play` coverage (dice resolution breadth, dual scene topology, token footprint/size scaling, conditions, item/inventory effects, leveled-resource progression, GM staging of distinct NPC stat blocks, lore-wiki linkage, shared/live-synced session state via the Session Wish Pool and clocks) has at least one Genie mechanic that exercises it end-to-end, verifiable without any other system pack loaded.
- **SC-002**: A GM can create a Genie world, stage at least one NPC, and run a complete combat encounter (including at least one Manifestation roll, one condition applied and cleared, and one scene-topology switch) using only Genie content.
- **SC-003**: Genie ships with zero third-party attribution requirements — its `legal` object contains no external license name, no required notice, and no trademark restriction.
- **SC-004**: An engine regression affecting any of the facets in SC-001 is detectable by exercising Genie's own scenarios alone, without requiring any licensed system's digest to be present.
- **SC-005**: A party can play a complete Genie session (Session Wish Pool, Doom Clock, at least two Puzzle Clocks, Session Resources) from start to a definitive win or loss outcome, per FR-016, with the Session Wish Pool, both clock types, and every Session Resource trade staying accurately synced across every connected player and the GM throughout.
- **SC-006**: At least one Session Resource trade between two different players occurs and is visible to the full party during a played session, demonstrating the negotiation layer functions, not just the underlying clock/roll mechanics.

## Assumptions

- Genie is a single, class-less/archetype-less character template for v1 — every character shares the same ability/skill/resource shape, differentiated by chosen values and a Patron/lineage link rather than by a class system. This keeps the spec focused on engine-facet coverage rather than game-design depth; a class/archetype layer can be added later as a separate feature if desired.
- Genie's playability is a first-class goal, not a secondary concern to engine coverage — the two are being designed together (per the Session Wish Pool above and further clarifications in this section), on the premise that a genuinely fun, replayable session is what actually exercises the engine repeatedly rather than a fixture nobody chooses to sit down and play.
- The "Diminutive through Colossal" size-category scale is a Genie-original naming choice (distinct from D&D 5e's "Tiny through Gargantuan" and Pathfinder 2e's matching scale) to avoid any appearance of derivation from either licensed system, consistent with FR-011's wholly-original requirement.
- Building the actual `packs/systems/genie/` implementation (server validators, web components, engine hooks) is expected to follow this spec through the same plan → tasks → implementation flow used for prior features, and is not itself scoped or scheduled by this document.
- Genie is additive — it does not replace, deprecate, or alter any of the six externally-licensed system digests or the manifest contract work already completed in specs 015/016; it is a seventh, original system pack.
- A typical Genie session assumes a small co-op party (3-5 players plus a GM) and a small, fixed set of Session Resource types (illustratively Insight, Favor, Essence) — exact counts, names, and per-Puzzle-Clock costs are game-balance/content decisions for the planning phase, not fixed by this spec.
- The Session Wish Pool, Doom Clock, Puzzle Clocks, and Session Resources are all session-scoped, shared, live-synced state — a genuinely new engine facet beyond the per-character `data_types` used by Genie Character/NPC (Key Entities), and a deliberate, additional test of the platform's existing live-sync infrastructure (spec `005-live-canvas-sync`) under multi-player shared-state conditions it wasn't originally exercised against.
