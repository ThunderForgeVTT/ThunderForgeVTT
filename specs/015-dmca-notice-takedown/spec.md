# Feature Specification: DMCA Notice-and-Takedown Process

**Feature Branch**: `015-dmca-notice-takedown`

**Created**: 2026-08-23

**Status**: Draft

**Input**: User description: "DMCA / Notice-and-Takedown Compliance Process — We ship open-license game system packs (5E System Core under CC-BY-4.0, Pathfinder 2e under ORC, Cypher System, Fate Core, Blades in the Dark, Year Zero Engine) built strictly from licensed SRD/reference-document text. But GMs and players will inevitably want to hand-enter copyrighted content from retail rulebooks (extra subclasses, spells, monsters not in the SRD) into their own worlds' compendiums. Per our own legal research (research/vtt_open_license_game_systems.md), the platform's biggest liability isn't the licensed content we ship — it's unauthorized copyrighted content users upload. To stay inside DMCA safe-harbor protection we need: a registered DMCA agent, a documented notice-and-takedown procedure, a way for rights holders to submit takedown notices, a way to locate and disable access to flagged content, a counter-notice process, and a repeat-infringer policy — and critically, we must never host a centralized public repository where users can freely share their custom copyrighted compendiums with other users/worlds (private, per-world compendium data entered by a GM for their own game is a different, lower-risk case than a public sharing/marketplace feature). Build this as a spec covering: the takedown intake/handling process (who receives notices, SLA, how content gets disabled), the technical hook points in our compendium/content system where a takedown action must be able to disable a specific piece of user-entered content, agent registration and legal-page requirements, and guardrails that must be satisfied before we ever ship any public compendium-sharing or marketplace feature."

## Clarifications

*(No clarification session held — DMCA safe-harbor requirements are governed by 17 U.S.C. § 512 and are well-established industry practice; the request specifies the scope explicitly (private per-world compendiums in scope, no public sharing feature exists yet) leaving no critical ambiguity that couldn't be resolved with the reasonable defaults recorded in Assumptions below.)*

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A rights holder submits a takedown notice and the flagged content is disabled (Priority: P1)

A copyright holder (e.g. a publisher, or an agent acting on their behalf) discovers that a specific world's compendium contains material they believe infringes their copyright — for example, a GM copied a full proprietary subclass or a named monster stat block from a retail sourcebook into their world's compendium. They locate the platform's designated DMCA contact information (published in a legal/compliance page) and submit a notice identifying the copyrighted work, the infringing material's location, and their contact information, under penalty of perjury.

**Why this priority**: This is the entire reason the feature exists — without a functioning intake and response path, the platform has no way to claim safe-harbor protection at all, regardless of every other requirement being met.

**Independent Test**: Submit a notice through the published contact channel referencing a specific piece of content in a test world's compendium; confirm the content becomes inaccessible to the world's members within the committed response window, and confirm the submitter receives acknowledgment that action was taken.

**Acceptance Scenarios**:

1. **Given** a published DMCA contact/notice channel, **When** a rights holder submits a notice containing all statutorily required elements (identification of the copyrighted work, identification and location of the allegedly infringing material, contact information, good-faith statement, accuracy statement under penalty of perjury, signature), **Then** the notice is logged and enters the takedown handling process.
2. **Given** a validly-formed notice identifying a specific compendium entry, **When** the notice is processed, **Then** that specific entry becomes inaccessible to all members of the world it lives in, without needing to delete or affect any other content in that world's compendium.
3. **Given** a notice that is missing required statutory elements, **When** it is received, **Then** it is not actioned as a takedown and the submitter is told what's missing.
4. **Given** content has been disabled in response to a notice, **When** the affected world's GM (the uploader) checks that content, **Then** they see it is disabled and can see why, with a path to dispute it.

---

### User Story 2 - A GM whose content was taken down submits a counter-notice (Priority: P2)

A GM whose compendium entry was disabled believes the takedown was made in error (e.g. their entry only contains SRD-licensed material, or falls under fair use) and wants it restored. They submit a counter-notice asserting a good-faith belief the material was removed by mistake or misidentification, consenting to jurisdiction, and providing contact information.

**Why this priority**: Required by DMCA safe-harbor law itself (17 U.S.C. § 512(g)) to keep the process balanced and defensible — without it, the platform's takedown process is one-sided and creates its own liability/fairness exposure. Second priority because the intake/disable path (User Story 1) must exist first before a counter-process has anything to counter.

**Independent Test**: From a previously-disabled entry, trigger a counter-notice submission with the required statutory statement; confirm the original claimant is notified and, absent a renewed legal action within the required waiting period, the content is restored.

**Acceptance Scenarios**:

1. **Given** disabled content and its owning GM, **When** the GM submits a counter-notice with the required statutory elements, **Then** the original notice submitter is forwarded the counter-notice and informed of the restoration timeline.
2. **Given** a submitted counter-notice and no further legal action received from the original claimant within the statutory waiting period, **When** that period elapses, **Then** the content is restored and becomes accessible again to the world's members.
3. **Given** the original claimant files suit (or the platform otherwise reasonably determines restoration is not appropriate) before the waiting period elapses, **When** that happens, **Then** the content remains disabled.

---

### User Story 3 - The platform maintains a repeat-infringer policy (Priority: P2)

Compliance staff need to track how many valid takedown notices have been upheld against a given account over time, so that an account which repeatedly and knowingly hosts infringing content can be identified and, per the platform's published policy, have its account terminated — a required element of maintaining DMCA safe-harbor eligibility.

**Why this priority**: Statutorily required for safe-harbor eligibility (17 U.S.C. § 512(i)) but only becomes operationally relevant once the notice/takedown flow (User Story 1) is already running and generating a history to evaluate.

**Independent Test**: Simulate three valid, non-disputed takedown notices upheld against the same account within the policy's lookback window; confirm the account is flagged for the repeat-infringer review/termination path defined in the published policy.

**Acceptance Scenarios**:

1. **Given** an account with a history of takedown notices, **When** compliance staff review it, **Then** they can see a chronological record of every notice, whether it was disputed/restored, and whether it counts toward the repeat-infringer threshold.
2. **Given** an account crosses the published repeat-infringer threshold, **When** that occurs, **Then** the account is flagged for the termination process defined in the platform's published policy.

---

### User Story 4 - Guardrails are enforced before any public compendium-sharing feature ships (Priority: P1)

Before product/engineering ships any feature that lets one world's compendium content become visible, copyable, or discoverable by users outside that world (a public marketplace, a shared community compendium, cross-world content browsing, etc.), the team must be able to verify — as an explicit go/no-go gate — that the notice-and-takedown process, DMCA agent registration, and repeat-infringer policy from this spec are fully operational and that the new feature does not itself become "a centralized, public repository where users can freely share... copyrighted compendiums," which the platform's own legal research identifies as the single highest-liability move available.

**Why this priority**: This is a preventative control, not a reactive one — it's cheaper and safer to block a risky feature at the design/launch-review stage than to build it and discover the liability afterward. Equal priority to User Story 1 because both are prerequisites to the platform being allowed to operate any user-generated-content surface at all.

**Independent Test**: Attempt to take a hypothetical "public compendium sharing" feature through the platform's launch/release review process without the DMCA program (Stories 1-3) fully operational; confirm the review process has a defined checkpoint that blocks it.

**Acceptance Scenarios**:

1. **Given** a proposed feature that would expose compendium content beyond its owning world, **When** it reaches design or launch review, **Then** the review explicitly checks for and requires: an operational takedown intake/response process, a registered DMCA agent, and a repeat-infringer policy already in force.
2. **Given** such a feature is proposed, **When** it is evaluated, **Then** the review documents whether the feature would constitute a "centralized public repository" for user-shared copyrighted content and, if so, requires that determination to be resolved (redesigned or explicitly risk-accepted by an accountable owner) before build work begins.

### Edge Cases

- What happens when a notice targets content that has already been deleted by the GM before the notice is processed? (The claimant should still get a response confirming the content is inaccessible/gone.)
- What happens when a notice is submitted against content that is actually 100% SRD-licensed material identical to what ships in the official system pack? (Process should support a fast-path rejection/response once compliance staff confirm the match, without waiting on a counter-notice cycle.)
- How does the system handle a notice targeting an entire world's compendium rather than one specific entry? (Statute requires specificity; a non-specific notice should be treated as incomplete per User Story 1, Scenario 3, with a response asking the claimant to identify specific material.)
- What happens if a GM's world is deleted or the account is closed while a notice or counter-notice is in process? (The record of the notice and its resolution must be retained per the platform's data-retention policy regardless of world/account deletion.)
- What happens when the same claimant sends duplicate or near-duplicate notices for the same content? (Should be recognized and not double-count toward repeat-infringer tracking or create duplicate disable actions.)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The platform MUST publish a designated DMCA/copyright agent's contact information (name/title, mailing address, and an electronic contact method) on a legal/compliance page that is reachable without requiring login, and MUST keep that designation current with the U.S. Copyright Office's designated agent directory.
- **FR-002**: The platform MUST provide a channel (form and/or email address) through which anyone can submit a takedown notice, and MUST log every submission with a timestamp and unique reference.
- **FR-003**: The platform MUST validate incoming notices for the statutory required elements (signature, identification of the copyrighted work, identification and location of the allegedly infringing material, claimant contact information, good-faith statement, accuracy statement under penalty of perjury) before treating a notice as actionable, and MUST inform the submitter when required elements are missing.
- **FR-004**: For a validly-formed notice, the platform MUST be able to disable access to the specific identified piece of user-entered compendium content (not the entire world, not unrelated content) within a committed response window from notice validation.
- **FR-005**: The platform MUST notify the content's owning GM/uploader that specific content was disabled, the reason, and how to submit a counter-notice.
- **FR-006**: The platform MUST provide a channel through which a GM can submit a counter-notice containing the statutory required elements (identification of the removed material and its location before removal, a statement under penalty of perjury of good-faith belief the removal was a mistake or misidentification, consent to jurisdiction, and contact information).
- **FR-007**: Upon receiving a valid counter-notice, the platform MUST forward it to the original claimant and MUST restore the content after the statutory waiting period unless the claimant provides notice of further legal action within that period.
- **FR-008**: The platform MUST maintain a durable, queryable record per account of takedown notices received, their validity determination, and resolution (upheld, restored via counter-notice, rejected as invalid) sufficient to evaluate repeat-infringer status.
- **FR-009**: The platform MUST publish a repeat-infringer policy defining the threshold and consequence (e.g. account termination) for accounts with multiple valid, unresolved-in-their-favor takedown notices, and MUST apply it consistently when the threshold is crossed.
- **FR-010**: The platform's takedown/disable capability MUST operate at the level of an individual compendium entry (e.g. a single NPC, item, ability, or lore entry) so that unrelated content in the same world is unaffected by a takedown action.
- **FR-011**: The platform MUST NOT host, and product/engineering MUST NOT ship, any feature that functions as a centralized public repository allowing users to freely share their custom (potentially copyrighted, non-SRD) compendium content with other users or worlds, unless and until such a feature has passed the guardrail review described in FR-012.
- **FR-012**: Any proposed feature that would make one world's compendium content visible, copyable, searchable, or otherwise accessible outside that world (public sharing, marketplace, cross-world browsing, etc.) MUST pass an explicit design/launch review checkpoint confirming FR-001 through FR-009 are operational and documenting whether the feature constitutes a "centralized public repository" under this policy, before implementation begins.
- **FR-013**: Records of takedown notices, counter-notices, and their resolutions MUST be retained independent of whether the associated world or account is later deleted, for the platform's standard legal-hold retention period.
- **FR-014**: The platform's public-facing legal/compliance page MUST distinguish, in plain language, between (a) official system packs distributed under open licenses (CC-BY, ORC, Cypher System Open License, Free League FTL, etc.) and (b) user-entered compendium content, which is the user's sole responsibility and subject to this notice-and-takedown policy.

**Scope of the platform's reach** (added 2026-09-04)

- **FR-015**: The platform's public-facing legal/compliance page MUST state, in plain language, where the platform's ability to act on a notice ends. It MUST commit to both of the actions that are within its control — disabling access to the identified content on the platform, and deactivating any synchronisation or export that would continue to publish that content to a service the platform does not operate — and MUST state that it has no authority over, and will not pursue, copies already placed on such a service.
- **FR-016**: Where a feature lets a user copy or synchronise their world's content to a service the platform does not operate, acting on a valid takedown MUST stop that content being carried outward, and the platform MUST retain the ability to deactivate the outward path entirely. Deactivating the whole path is an enforcement action, not the default response to a single notice — FR-010 requires a takedown to leave unrelated content alone, and a world's entire mirror is unrelated content. It MUST be exercised where excluding the item alone cannot stop republication, and where the repeat-infringer policy of FR-009 is applied.
- **FR-017**: Content a user exported to a service they control MUST be described as published by that user. The platform's inability to retract it MUST NOT be presented as the platform disclaiming the *user's* responsibility for it; both facts MUST appear together, since stating only the first reads as an invitation.
- **FR-018**: A rights holder MUST be told, on the same page, that a notice concerning material on a third-party service should be directed to that service's own provider. The platform MUST NOT position itself as a route to content that has already left it.

### Key Entities

- **Takedown Notice**: A rights holder's formal claim that specific user-entered content infringes their copyright. Carries the statutory elements, a reference ID, timestamp, validity status, the identified content's location, and resolution state.
- **Counter-Notice**: A GM/uploader's formal response disputing a takedown. Carries the statutory elements, a reference ID, timestamp, linked takedown notice, and the resulting restoration date if unopposed.
- **Compendium Entry**: The individual unit of user-entered world content (NPC, item, ability, lore entry, etc.) that a takedown or counter-notice targets; must be independently disable-able/restorable without affecting sibling entries in the same world.
- **Infringement Record**: The durable, per-account history entry created for each resolved notice, used to evaluate repeat-infringer status; independent of whether the underlying world/content still exists.
- **DMCA Agent Designation**: The registered contact (name, address, electronic contact) published on the legal/compliance page and filed with the U.S. Copyright Office's designated agent directory.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of validly-formed takedown notices result in the identified content being made inaccessible within the platform's published response window.
- **SC-002**: 100% of takedown actions affect only the specifically identified compendium entry, with zero unrelated content in the same world becoming inaccessible as a side effect.
- **SC-003**: 100% of counter-notices meeting the statutory requirements are forwarded to the original claimant and, absent further action from the claimant, result in restoration within the statutory waiting period.
- **SC-004**: The platform's designated DMCA agent information is discoverable by an unauthenticated visitor within two clicks from any legal/footer link, at all times.
- **SC-005**: Zero public compendium-sharing or marketplace features reach production without having passed the guardrail review defined in FR-012.
- **SC-006**: Repeat-infringer accounts crossing the published threshold are flagged for review within one business day of the qualifying notice's resolution.

## Assumptions

- The platform currently has no public compendium-sharing, marketplace, or cross-world content-browsing feature (confirmed by repository search); this spec's guardrail requirements (FR-011, FR-012, User Story 4) are preventative, to be enforced before any such feature is proposed, not a retrofit of an existing one.
- Per-world, private compendium data entered by a GM for their own game (see spec `011-world-compendium`) is the content surface this takedown process applies to; it is not itself a "centralized public repository" as long as it remains scoped to that world's own members.
- A "committed response window" and the exact statutory counter-notice waiting period will be set to standard DMCA practice (commonly 10-14 business days for the counter-notice waiting period per 17 U.S.C. § 512(g)(2)(C)) unless the business specifies otherwise; exact numeric SLAs are a policy decision for legal/compliance to finalize, not an engineering constraint.
- This spec covers process, policy, and the technical capability to target/disable an individual compendium entry — it does not prescribe the specific implementation (e.g. soft-delete flag vs. access-control rule) of that capability; that belongs in the planning phase.
- Notice submission channels (web form vs. dedicated email) and the review/moderation staffing model are operational decisions to be resolved during planning; both are compatible with the requirements above.
