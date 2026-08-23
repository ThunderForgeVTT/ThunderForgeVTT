# ADR-027: Game System Packaging and Manifest Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

**Amended:** 2026-08-23 (spec `016-system-pack-legal-compliance` — added the
`legal` object, documented below; see that section's own note)

## Context

ThunderForgeVTT supports pluggable game systems ("system packs") — 5E
System Core, and eventually others such as Pathfinder 2e, Cypher System,
Fate Core, Blades in the Dark, and Year Zero Engine. Each system pack
ships server-side data validators (`packs/systems/<id>/server`),
engine-side logic where relevant (`packs/systems/<id>/engine`), and a web
bundle (`packs/systems/<id>/web`) alongside a manifest file,
`packs/systems/<id>/system.json`, that describes the pack: identity,
version, compatibility range, entry points, and (for systems with
character-sheet mechanics) the skill/ability/spell-slot/data-type
definitions the server's `GameSystemRegistry`
(`src/server/src/systems.rs`) uses to validate actor data.

Two independent code paths read a system pack's manifest today:

1. **Bundled packs, served as static JSON** (`get_system_manifest` in
   `src/server/src/systems.rs`) — reads `packs/systems/<slug>/system.json`
   straight off disk and returns it as untyped JSON. This is how `5E
   System Core` (`dnd5e`) is actually delivered to the frontend today.
2. **Admin-installed packs** (`install_game_system`, same file) — accepts
   a ZIP upload, extracts it, and validates its `system.json` against a
   typed, schema-checked Rust struct: `pack_system_spec::SystemManifest`
   (`crates/pack_system_spec`), using `schemars` to derive a JSON Schema
   and `jsonschema` to validate an uploaded manifest against it before
   the pack is accepted. This is the path a future third-party/community
   pack install would go through.

These two paths currently accept different manifest shapes — the bundled
`dnd5e` pack's `system.json` (with `author`, `license`, `skills`,
`abilities`, `spellSlots`, `data_types`, etc.) does not conform to
`pack_system_spec::SystemManifest`'s schema (which expects `authors: []`,
`packs: []`, and has no `license`/`skills`/`abilities` fields at all) —
they were designed for different eras of this feature and have not yet
been reconciled into one contract. This ADR does not resolve that
reconciliation; it records the manifest requirements each path already
enforces or must enforce, so a pack author (internal today, potentially
third-party later) has one place to learn what's expected.

## Decision

**A system pack's `system.json` MUST describe, at minimum:** a stable
`id`, a human-readable `title`, a `version`, and (per the amendment
below) a structured `legal` object. Systems with character-sheet
mechanics additionally declare `skills`/`abilities`/`data_types` (and any
system-specific blocks like `spellSlots`) that the server's
`GameSystemRegistry` validators consume; a system with no character-sheet
mechanics may omit these.

### Amendment (2026-08-23, spec 016): the `legal` object

Per the platform's own open-license compliance review
(`research/vtt_open_license_game_systems.md`), every system pack built
from a third-party open license (CC-BY, ORC, Cypher System Open License,
Free League's FTL, etc.) is legally required to display specific
attribution/notice text to the end user as a condition of continuing to
use that licensed content — not just record it internally. Before this
amendment, `system.json` had only a loose free-text `license` string,
with no structured place for attribution text, a license-specific
required notice/badge (e.g. Pathfinder 2e's ORC Notice, the Cypher
System's "Compatible with the Cypher System" badge), a non-affiliation
disclaimer, trademark restrictions, or required UI placement guidance —
so every pack either omitted this information or invented an ad hoc
shape.

**Every system pack's manifest now MUST include a `legal` object**,
sibling to `license` (which is kept, unchanged, for short/backward-compat
display — `legal` is its structured, render-ready expansion, not a
replacement):

| Field | Type | Required | Purpose |
|---|---|---|---|
| `licenseName` | string | Yes | e.g. `"CC-BY-4.0"`, `"Open RPG Creative License (ORC)"` |
| `attributionText` | string | Yes | The full attribution string the license requires (TASL for CC-BY, ORC Notice text, etc.) |
| `requiredNotice` | string, nullable | No | A license-specific mandatory phrase/badge distinct from general attribution — e.g. `"Compatible with the Cypher System"` |
| `disclaimer` | string, nullable | No | Non-affiliation disclaimer, where the license requires one |
| `trademarkRestrictions` | string[] | No (default `[]`) | Human-readable restriction statements (e.g. "do not use X as a product name") |
| `requiredUiPlacement` | string, nullable | No | Free-text placement requirement beyond the default settings-view display (e.g. "must appear on the system-selection screen itself") |
| `sourceUrl` | string, nullable | No | Link to the license's canonical text |

A system with no third-party license at all (fully original, in-house
content) still supplies a `legal` object — stating `"original content, no
third-party license"` in `attributionText` rather than omitting the
object entirely. **Omission is itself the non-compliance failure mode**;
an absent `legal` object is indistinguishable from "nobody thought about
licensing for this pack," which is exactly the gap this amendment closes.

**Enforcement**: both manifest-reading paths described in Context now
reject a manifest missing `legal` or with an empty
`licenseName`/`attributionText` — `pack_system_spec::validate_system_manifest`
(the admin-upload path) via its derived JSON Schema plus an explicit
non-empty check (schemars guarantees presence/type, not string content),
and `get_system_manifest` (the bundled-pack serve path) via the same
`validate_legal_content` check called directly, since bundled packs never
go through the schema-validated struct at all. A pack failing this check
is never served to a GM (`UNPROCESSABLE_ENTITY`, fail closed) — see
`crates/pack_system_spec/src/lib.rs` and
`src/server/src/systems.rs::get_system_manifest`.

**Rendering**: the application renders a system's `legal` object at two
points — when a GM assigns or changes a world's active system, and from
that world's persistent System Settings view
(`apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx`) at any
later time — never buried only in a general Terms of Service document.
See spec `016-system-pack-legal-compliance` for the full requirements and
`contracts/manifest-legal-schema.md` for the field-level contract this
table summarizes.

## Consequences

- Every future system pack (Pathfinder 2e, Cypher System, Fate Core,
  Blades in the Dark, Year Zero Engine, and any third-party pack, should
  that ever be supported) MUST supply a compliant `legal` object before
  it can be loaded — this is enforced by both manifest-reading code
  paths, not left to reviewer diligence alone.
- A pack author has one documented shape to fill in (this ADR, plus
  `contracts/manifest-legal-schema.md`'s field-level detail) rather than
  needing to invent one per license type.
- The `pack_system_spec::SystemManifest` schema (admin-upload path) and
  the bundled-pack `system.json` shape (`dnd5e`'s actual fields) remain
  two distinct, unreconciled contracts — this amendment adds `legal` to
  both independently rather than resolving that pre-existing split, which
  remains a known gap for a future ADR/spec to address if/when
  third-party pack installation becomes a real product surface.

## Alternatives Considered

- **A single free-text `legal` string** (minimal change): rejected —
  this is exactly the status quo gap the compliance review found; the
  existing `license` string already does this and was insufficient.
- **A per-license-type discriminated union** (`type: "cc-by" | "orc" |
  "csol" | ...` with type-specific fields): rejected as over-engineering
  for the known license families reviewed — a flat optional-everything
  object satisfies all of them (CC-BY, ORC, Cypher System Open License,
  Free League FTL) without a variant type, and a new license type can add
  fields later without a breaking schema change.
- **A separate `GET /systems/:id/legal` endpoint**, fetched independently
  of the rest of the manifest: rejected — adds a redundant round-trip and
  cache-invalidation surface for data with the same lifecycle as the rest
  of the manifest (loaded once, cached, effectively static).
