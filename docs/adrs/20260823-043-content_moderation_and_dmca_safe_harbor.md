# ADR-043: Content Moderation and DMCA Safe-Harbor Enforcement Boundary

**Status:** Accepted

**Decision Date:** 2026-08-23

## Context

The platform ships open-licensed system-pack content (5E System Core under
CC-BY-4.0, Pathfinder 2e under ORC, and others), but GMs and players
inevitably hand-enter copyrighted material from retail sourcebooks into
their own worlds' compendiums (actors, items, lore entries). Per the
platform's own legal research (`research/vtt_open_license_game_systems.md`),
unauthorized user-uploaded copyrighted content — not the licensed content
the platform ships — is the primary DMCA liability surface. To retain
safe-harbor protection under 17 U.S.C. § 512, the platform needs a working
notice-and-takedown process, and every current and future content-read path
must respect it.

Spec `015-dmca-notice-takedown` introduces this program. Two decisions in
it are architecturally significant enough to record here rather than leave
implicit in the spec, per Constitution Principle IV: they establish a new
authorization concept every future content-table author must know about.

## Decision

**1. A single polymorphic `content_moderation_actions` table, not
per-table moderation columns.** Every takedown notice, counter-notice, and
resolution is one append-only row referencing content by
`(entity_type, entity_id)` rather than a foreign key into a specific
content table. The three current content domains (`world_actors`,
`world_items`, lore entries) each gain a read-time lookup against this
table; a fourth content type (any future Compendium tab, per spec
`011-world-compendium`'s stated intent to grow) needs zero migration to
participate — it only needs to call the same lookup. The table has
deliberately no cascading foreign keys to `worlds`/`users`/content tables,
since moderation history must survive deletion of any of them (FR-013).

**2. Content can now be "moderation-disabled," a state orthogonal to
ownership/permission.** Enforcement happens at the same GraphQL
resolver/repository boundary that already enforces ownership checks
(Constitution Principle III, ADR-009/013/023/028) — never left to the
client. A moderation-disabled entity is excluded entirely from list
queries and returns a placeholder (never real field values) from
single-entity queries, for every caller including the content's own
owner/GM. This is a genuinely new authorization axis: a caller can hold
full Owner-level permission on an entity and still not see its real
content if it's moderation-disabled. **Every future content-read path
(new Compendium tabs, search indexes, share-link previews, cross-reference
lists like lore's "linked from") MUST add this check** — it does not come
for free from the existing permission system, and omitting it is a direct
safe-harbor and takedown-effectiveness failure. Two such omissions were
found and fixed during this feature's own implementation (item/actor
share-link previews, and lore's "linked from" backlink lists) — both
existing read paths that predated this ADR and had to be retrofitted,
underscoring why this rule needs to be written down rather than
rediscovered per-feature.

Auto-restoration after a counter-notice's statutory waiting period is
evaluated lazily at read time (the first read past the due date
materializes a real `content_restored` event) rather than via a new
background job/scheduler, since no such infrastructure exists in this
codebase and the read-time check is already required regardless.

## Consequences

- Every future content type added to the Compendium (or any other
  user-generated-content surface) must wire its list and single-entity
  read paths through `moderation::effective_status`
  (`src/server/src/moderation/mod.rs`) before shipping, the same way it
  must already wire through the existing ownership-permission checks.
- Spec `015`'s own FR-011/FR-012 guardrail (see the platform's
  feature/launch-review checklist) requires this program to be
  operational, and requires an explicit "is this a centralized public
  repository" determination, before any feature that exposes one world's
  compendium content to another world or the public may proceed.
- No new external integration is required — repeat-infringer tracking is
  a query over this platform's own data; DMCA agent registration with the
  U.S. Copyright Office is an out-of-band administrative filing, tracked
  as an operational checklist item, not a system integration.

## Alternatives Considered

- **Per-content-table `disabled_at`/`disabled_reason` columns**: rejected
  — duplicates moderation state and history across every content table and
  still needs a separate table for the notices/counter-notices/
  repeat-infringer records themselves, so it doesn't avoid the polymorphic
  table, it only adds redundant columns on top of it.
- **Reusing the existing soft-delete/deletion flag, if any**: rejected —
  takedown is not deletion. The owner must be able to see *why* content is
  disabled and pursue a counter-notice, and the moderation record must
  survive even if the owner later hard-deletes the entity themselves
  (FR-013). Deletion and moderation are different concerns and must not
  share a flag.
- **Client-side filtering of a `moderationStatus` field**: rejected
  outright — would return the disabled content's real data to any client
  that ignores the flag, defeating the point of a takedown.
