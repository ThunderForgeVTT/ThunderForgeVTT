# Phase 0 Research: DMCA Notice-and-Takedown Process

## R1: Where does "disable one compendium entry" hook into the existing content model?

**Decision**: A single new polymorphic table, `content_moderation_actions`, referencing content by `(entity_type, entity_id)` rather than a foreign key per content table. Read paths for `world_actors`, `world_items`, and lore-wiki entries (spec `012-lore-wiki`) each gain a join/lookup against this table to exclude or flag disabled entries.

**Rationale**: The platform already has three independent per-world content domains (actors, items, lore) with more likely to come (spec `011-world-compendium` explicitly frames Compendium as tabbed sections that will grow). A moderation concern that lived as a column on each content table (`disabled_at`, `disabled_reason`) would need to be added, tested, and kept in sync across every current and future content table. A single cross-cutting table keeps the moderation domain (notices, counter-notices, resolutions, repeat-infringer history) in one place, independently queryable by compliance staff without joining across three-plus content tables, and extensible to new content types with zero migration.

**Alternatives considered**:
- *Per-table `disabled_at`/`disabled_reason` columns*: simpler single-table joins at read time, but duplicates moderation state and history across tables, and provides no natural home for the notice/counter-notice/repeat-infringer records themselves — those would need a separate table anyway, so this alternative doesn't actually avoid the polymorphic table, it just adds redundant columns on top of it.
- *Soft-delete the entry itself (reuse existing delete flag if any)*: rejected — takedown is not deletion; the GM must be able to see *why* it's disabled and pursue a counter-notice, and the record must survive even if the GM later hard-deletes the entry themselves (FR-013). Deletion semantics and moderation semantics are different concerns and must not share a flag.

## R2: How does the visibility check get enforced without becoming a per-query afterthought?

**Decision**: Enforce at the GraphQL resolver / repository read boundary — the same layer that already enforces ownership/permission checks per Constitution Principle III and ADR-009/013/023/028. A disabled entry is excluded from list queries and returns a moderation-placeholder (not the underlying content) from single-entry queries, for all callers including the owning GM — the frontend then renders `ModeratedContentBanner` instead of the real content.

**Rationale**: Consistent with the existing, already-enforced pattern (client is never trusted to hide content on its own). Reusing the same boundary means moderation checks compose with existing permission checks (a world member without read access to an actor already gets nothing; moderation adds one more reason to get nothing) rather than requiring a second, parallel authorization system.

**Alternatives considered**: Client-side filtering based on a `moderationStatus` field returned to the client — rejected outright; it would leak the disabled content's data to any client that ignores the flag, defeating the point of a takedown.

## R3: What are the exact statutory elements and timelines to encode in validation logic?

**Decision**: Encode 17 U.S.C. § 512(c)(3) notice elements and § 512(g)(2)-(3) counter-notice elements/timeline as documented, human-readable validation rules (required-field presence checks), not as free-text legal advice generation. Use the commonly-adopted 10-14 business day counter-notice waiting period as the default, configurable by compliance/legal without a code deploy (a config value, not a hardcoded constant).

**Rationale**: The spec's Assumptions section already flags exact SLA numbers as a policy decision for legal/compliance, not an engineering constraint — encoding the waiting period as configuration (not a literal in code) respects that split of responsibility and lets the number be tuned without a release.

**Alternatives considered**: Hardcoding "14 days" directly in application logic — rejected because it silently becomes an engineering decision on a legal question; a config value keeps the number owned by whoever legal/compliance says should own it.

## R4: Does this need a new ADR?

**Decision**: Yes — one short ADR (`docs/adrs/<next-number>-content-moderation-and-dmca-safe-harbor.md`) documenting the polymorphic moderation-table decision (R1) and the enforcement-at-the-boundary decision (R2), since both introduce a new cross-cutting authorization concept (content can be "moderation-disabled" independent of ownership/permission state) that future content-table authors need to know about.

**Rationale**: Constitution Principle IV requires an ADR for decisions that change an ownership/authorization boundary. This does — every future content-read path must now also consider moderation state, not just ownership/permission.

**Alternatives considered**: Folding the decision into this spec/plan alone without an ADR — rejected per Principle IV; specs describe WHAT/WHY for stakeholders, ADRs record the durable technical decision so a future content-table author (human or agent) discovers the requirement without re-reading this spec.

## R5: Does the repeat-infringer threshold or agent-registration process require new external integrations?

**Decision**: No. Repeat-infringer tracking is a query over the platform's own `content_moderation_actions` table (count of upheld, non-restored notices per account within the policy's lookback window). DMCA agent registration with the U.S. Copyright Office's Designated Agent Directory (per 17 U.S.C. § 512(c)(2)) is an out-of-band administrative/legal action (a form submission to copyright.gov), not a software integration — this plan only requires that the resulting agent contact information be published on the legal/compliance page (FR-001) and kept in sync with what's actually filed.

**Rationale**: Keeps scope honest — no code needs to talk to the Copyright Office; that registration is a one-time (renewable) administrative task for whoever owns legal/compliance, tracked as an operational checklist item, not a system integration.

**Alternatives considered**: None — there is no plausible integration alternative for a government registry filing.
