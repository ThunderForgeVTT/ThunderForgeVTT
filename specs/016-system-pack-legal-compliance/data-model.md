# Phase 1 Data Model: System Pack Legal & Attribution Compliance

There is no new persisted database entity for this feature — `legal` is static manifest data shipped inside each system pack's `system.json`, not per-world or per-account state. The "data model" here is the manifest field's shape and how it's threaded through existing in-memory types.

## SystemManifestLegal (new field on the existing `SystemManifest` type/struct)

| Field | Type | Required | Notes |
|---|---|---|---|
| `licenseName` | string | Yes | e.g. `"CC-BY-4.0"`, `"Open RPG Creative License (ORC)"`, `"Cypher System Open License"` — matches `legal.license_name` already used across the six `research/system_*.json` digests |
| `attributionText` | string | Yes | Full attribution string required by the license (TASL for CC-BY, ORC Notice text, etc.) |
| `requiredNotice` | string, nullable | No | A license-specific mandatory phrase/badge distinct from general attribution — e.g. Cypher System's `"Compatible with the Cypher System"` |
| `disclaimer` | string, nullable | No | Non-affiliation disclaimer text, where the license requires one (Cypher System, Year Zero Engine) |
| `trademarkRestrictions` | string[] | No (default `[]`) | Human-readable restriction statements, e.g. `"Do not use 'Pathfinder' as a product name implying endorsement"` |
| `requiredUiPlacement` | string, nullable | No | Free-text placement requirement beyond the default settings view, e.g. `"Must be prominently displayed on the system-selection/storefront screen, not just a settings tab."` — read by `systemLegalPlacement.ts` (research.md R3) as a verification signal, not parsed/branched on structurally |
| `sourceUrl` | string, nullable | No | Link to the license's canonical text, where available |

**Validation rules** (FR-007, research.md R4):
- A manifest failing to include a `legal` object at all, or including one with an empty `licenseName` or empty `attributionText`, MUST fail load-time validation.
- `trademarkRestrictions` and the nullable fields have no minimum-content requirement — a system with no special restrictions supplies `[]`/`null`, per spec Edge Cases ("original content, no third-party license" is still a valid, present `legal` object, not an omitted one).

## Relationship to existing `SystemManifest`

```
SystemManifest (existing, apps/web/src/contexts/GameSystemContext.tsx)
├── id: string                    (existing)
├── title: string                 (existing — already corrected to "5E System Core" etc.)
├── version: string                (existing)
├── legal: SystemManifestLegal     (NEW — this feature)
└── [key: string]: any             (existing catch-all — unaffected)
```

The Rust-side manifest struct in `src/server/src/systems.rs` gains the equivalent `legal` field with the same shape, using the project's existing serde/Diesel conventions for the rest of the manifest struct.

## Populated example (dnd5e, derived from research/system_dnd5e.json per FR-008)

```json
"legal": {
  "licenseName": "CC-BY-4.0",
  "attributionText": "This work includes material from the System Reference Document 5.2.1 (\"SRD 5.2.1\") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.",
  "requiredNotice": null,
  "disclaimer": "This system pack is an independent, unofficial implementation compatible with the 5E SRD 5.2.1 ruleset. It is not published, endorsed, or affiliated with Wizards of the Coast.",
  "trademarkRestrictions": [
    "Do not use 'Dungeons & Dragons' as a product/module name or in marketing that implies Wizards of the Coast endorsement",
    "May use 'compatible with fifth edition' or '5E compatible' phrasing, but not the D&D trademark or logo itself",
    "Do not reproduce proprietary campaign lore, named characters, deities, or iconic monsters excluded from the SRD (e.g. Mind Flayers, Beholders)",
    "Use the SRD's renamed terms for legacy items (e.g. 'Mysterious Deck' instead of 'Deck of Many Things')"
  ],
  "requiredUiPlacement": "Must be shown on the system selection screen or a permanent, easily accessible module settings tab — not buried in Terms of Service.",
  "sourceUrl": "https://creativecommons.org/licenses/by/4.0/"
}
```

(Field values drawn directly from the existing `legal` object in `research/system_dnd5e.json`, camelCased per R2 — no new legal research performed here.)
