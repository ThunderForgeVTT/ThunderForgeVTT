# Research: World Abilities Compendium

Phase 0 output. Resolves every open technical question before design.

---

## 1. Per-system presentation facets — where they live and how they reach a component

**The novel mechanism in this feature.** FR-010..FR-013 need each game system to
optionally re-label the fixed ability classifications in its own vocabulary.

### Decision

Ship facets as a new **optional top-level `abilityFacets` key in
`packs/systems/<id>/system.json`**, resolved on the frontend by a new
system-agnostic util (`apps/web/src/utils/abilityFacets.ts`) with built-in
default labels. **No server change, no GraphQL, no database, no migration.**

Manifest shape:

```json
"abilityFacets": {
  "spell":  { "label": "Spell",  "pluralLabel": "Spells" },
  "feat":   { "label": "Feat",   "pluralLabel": "Feats" },
  "power":  { "label": "Power",  "pluralLabel": "Powers" },
  "talent": { "label": "Talent", "pluralLabel": "Talents" }
}
```

Every key is optional at every level — an absent block, an absent
classification, or a malformed value all fall back to the built-in default for
that classification (FR-011).

### Rationale

- **The key name `abilities` is unavailable.** All 8 shipped packs already have
  a top-level `abilities` block holding ability *scores*
  (`{ label, abbreviation }` per score — Genie's Might/Cunning/Spirit, dnd5e's
  STR/DEX/…; `fate_core`'s is an empty `{}`). `abilityFacets` avoids the
  collision without renaming anything, which is exactly what the spec's
  Clarification #1 asked for.
- **`get_system_manifest` returns the manifest JSON verbatim.** It reads
  `system.json` off disk, validates only `legal` content, and returns the whole
  untyped `serde_json::Value`. A new top-level key reaches the client with
  **zero server code changes**.
- **Validation already tolerates new keys.** `pack_system_spec`'s schemars-derived
  schema does **not** set `additionalProperties: false`, and no test asserts
  rejection of extra keys — so existing manifests keep validating and new keys
  pass silently. Adding the field to the `SystemManifest` struct is optional
  (see "Open choice" below).
- **The frontend type already accommodates it.** `SystemManifest` in
  `apps/web/src/contexts/GameSystemContext.tsx` carries a
  `[key: string]: any` index signature.

### Precedent to copy exactly

Genie's `sizeCategories` is the established pattern for per-system config
reaching a component, and the new util should mirror it line for line:

| Layer | Existing (`sizeCategories`) | New (`abilityFacets`) |
|---|---|---|
| Manifest key | `packs/systems/genie/system.json` | `packs/systems/<id>/system.json` |
| Resolver util | `apps/web/src/utils/sizeCategory.ts` (`resolveSizeScale`, `DEFAULT_SIZE_SCALE`) | `apps/web/src/utils/abilityFacets.ts` (`resolveAbilityLabel`, `DEFAULT_ABILITY_LABELS`) |
| Fetch | `getGameSystemManifest(systemId)` (`apps/web/src/api/gameSystems.ts`) | same function |
| Consumption | `TokenPanel.tsx` — `useEffect` keyed on `gameSystemId`, `active` cancel flag, `catch → undefined` | same shape |

`sizeCategory.ts`'s doc comment states the design rule this feature inherits
verbatim: keep the util system-agnostic, never import a specific pack, and "any
future system pack that wants … gets it for free by shipping the same shape."

### ADR question — resolved, no amendment needed

The plan initially assumed adding a manifest field required amending ADR-027.
**It does not.** ADR-027's Decision says a manifest declares its blocks "and any
system-specific blocks like `spellSlots`" — additional optional top-level blocks
are already sanctioned. Its Alternatives Considered explicitly records that the
flat-optional-object design was chosen because "a new license type can add
fields later without a breaking schema change." The `legal` amendment was needed
only because it added a **required** field with an **enforcement** rule;
`abilityFacets` adds neither.

No ADR is required for this feature.

### Open choice, deferred to implementation

Whether to also add `ability_facets: Option<…>` to
`pack_system_spec::SystemManifest`. ADR-027 records that the bundled
`system.json` shape and the Rust `SystemManifest` (admin ZIP-upload path) are
already "two distinct, unreconciled contracts." Bundled packs need no struct
change. Adding one only benefits third-party uploaded packs. **Recommendation:
skip it** — it widens an already-acknowledged inconsistency for no present
benefit, and can be added when the upload path is reconciled.

### Alternatives rejected

- **Rename the manifest `abilities` block to `abilityScores`** — breaking change
  across all 8 packs, their tests, and ADR-027, to fix a collision that a
  different key name avoids for free. Explicitly declined by the requester.
- **Store facets in the database per world** — they are a property of the game
  system, not of a world's data; would need a migration, a sync path, and would
  drift from the pack that defines them.
- **A dedicated `GET /systems/:id/facets` endpoint** — ADR-027 already rejected
  exactly this shape for `legal`: "adds a redundant round-trip … for data with
  the same lifecycle as the rest of the manifest."

---

## 2. Duplicate the item modules rather than generalizing

### Decision

Create parallel `ability_*` modules mirroring the `item_*` ones, rather than
refactoring items and abilities into a shared "world artifact" abstraction.

### Rationale

- The two are structurally identical **today** but are expected to diverge
  exactly where it matters: abilities will gain usage/slots/preparation
  (explicitly deferred in Non-Goals), items already have quantity and will gain
  equipping. Coupling them now guarantees an unpick later.
- The codebase's own precedent is duplication: `auth/actor_permissions.rs`,
  `auth/lore_permissions.rs`, and `auth/item_permissions.rs` are three
  near-identical files, and `lore_entries_linking_to` /
  `…_to_actor` / `…_to_item` are three identical query helpers. A fourth
  follows an established, understood pattern; a generalization would be the
  novel thing.
- `ActorPermissionLevel` is already the shared level enum reused by items and
  lore — the one piece genuinely worth sharing is already shared. Abilities
  reuse it too rather than defining a fourth copy.

Recorded in plan.md's Complexity Tracking as a deliberate, non-violating choice.

### What to factor out instead

Two low-risk consolidations that reduce real duplication without coupling
domains, both optional and separable from the feature:

- `postGraphQL` is privately re-declared in `api/items.ts`, `api/itemShares.ts`,
  and `api/inventory.ts`. Abilities would make it six copies. Extracting one
  shared helper is a contained cleanup.
- `EFFECT_TYPE_LABELS` is duplicated verbatim in `ItemPreviewPanel.tsx` and
  `SharedItemPage.tsx`; abilities need the same map again.

---

## 3. Template bugs found in the item implementation — do not inherit

Mapping the precedent surfaced six real defects. Each is a decision point, not
a silent copy.

| # | Defect | Location | Decision for abilities |
|---|---|---|---|
| 1 | `updateItem` cannot clear a description — `description.or(existing.description)` means `null` is indistinguishable from "unchanged" | `mutations_items.rs` `update_item_impl` | **Fix in the ability version.** Use a nullable-explicit input so clearing works. |
| 2 | `LoreLinkTargetKind` TS type is `"LORE_ENTRY" \| "ACTOR"` — the `Item` variant was never added, though the backend returns it | `apps/web/src/types/lore.ts:53` | **Fix as a prerequisite** — must be widened for abilities anyway. |
| 3 | Autocomplete labels every non-lore candidate "Actor" via a binary ternary, so items already mislabel today | `LoreMarkdownEditor.tsx:92` | **Fix as a prerequisite** — replace the ternary with a `Record<LoreLinkTargetKind, string>` map before adding a fourth kind. |
| 4 | Effects are cloned on the share-copy path without re-running `validate_formula` | `mutations_item_shares.rs` | **Re-validate on copy** in the ability version. Cheap, and the source's validity is an assumption rather than a guarantee. |
| 5 | `ItemEffectEditor` renders for VIEWERs and in `view` mode — not gated by `canEdit` | `ItemDetailPage.tsx` | **Gate it** on the ability detail page. FR-017 restricts effect edits to Editor+. |
| 6 | `ItemCompendiumTab` declares a `refreshKey` prop the parent never passes; `iconAssetId` is selected everywhere but never rendered | `ItemCompendiumTab.tsx` | **Omit both** — don't carry dead surface into the new tab. |

Items 2 and 3 are pre-existing shipped bugs affecting items *today*. Fixing them
is a genuine prerequisite for this feature (a fourth link kind cannot be
labelled correctly otherwise), so they are in scope here rather than deferred.

---

## 4. Lore link resolution — append abilities last

### Decision

Extend `markdown/links.rs`'s fixed cascade to
**lore entry → actor → item → ability**, appending rather than inserting.

### Rationale

Resolution is first-match-wins on a case-insensitive exact title match, resolved
**once at save time** and stored. Inserting abilities anywhere but last would
silently change what an already-saved link means the next time its entry is
re-saved — a title that resolves to an item today must keep resolving to that
item. Appending guarantees no existing link changes target.

Ambiguity is meant to be resolved at authoring time by the `loreLinkTargets`
autocomplete (FR-030), not by the cascade — the cascade is only the
deterministic tie-break when a title is typed by hand.

### Note on `target_kind`

`world_lore_links.target_kind` is authoritative **only at insert time**. Because
every target FK is `ON DELETE SET NULL`, a row can keep `target_kind = 'ability'`
while `target_ability_id` has gone null; every read path treats a null FK as
unresolved regardless of the stored label. The table's original migration
deliberately keeps the "at most one target" CHECK looser than "kind must match
the non-null column," because a stricter constraint would be re-evaluated when
`ON DELETE SET NULL` fires and would block the very deletions FR-031 requires to
succeed. **Do not tighten it.**

---

## 5. GraphQL argument-shape convention

### Decision

Per-operation, mirroring the item split exactly:

- **`input: <X>Input!` object form** — `createAbility`, `updateAbility`,
  `setAbilityPermission`, `copySharedAbilityToWorld`, `attachAbilityToActor`.
- **Flat scalar args** — `deleteAbility`, `addAbilityEffect`,
  `updateAbilityEffect`, `removeAbilityEffect`, `abilityPermissions`,
  `removeAbilityPermission`, `sharedAbility`, `createAbilityShareLink`,
  `revokeAbilityShareLink`, `detachAbilityFromActor`, `worldAbilities`,
  `ability`, `suggestAbilityName`, `actorAbilities`.

### Rationale

This codebase has a documented bug class — spec 005 found five separate
frontend calls sending flat arguments where the resolver expected a single
`input` object, silently breaking the invite panel and join flow. Every current
item call was verified consistent. The mitigation is procedural: **write the
resolver signature first, then write the frontend query string to match it**,
and add a contract test rather than trusting symmetry.

---

## 6. Testing approach

### Decision

Server-side `#[cfg(test)] mod tests` mirroring the item test set, plus a
Playwright e2e for the Compendium tab. No new test infrastructure.

### Rationale

- **Server tests are the real coverage.** The item precedent has ~11 focused
  resolver tests (`only_dm_can_create_item`, `item_names_may_collide`,
  `add_item_effect_rejects_empty_formula`,
  `deleting_an_item_nulls_referencing_lore_links_instead_of_blocking`, etc.)
  using `test_support::{test_app_state, insert_test_user, insert_test_world}`.
  Abilities get direct analogues, plus new ones for classification validity,
  facet fallback, and actor-attachment de-duplication.
- **e2e is genuinely runnable here.** This feature has no Bevy canvas surface,
  so it escapes the documented sandbox limitation that blocks every
  canvas-interaction e2e test in this repo — the same property that let spec
  005's `live-sync.spec.ts` pass 4/4. `world-compendium.spec.ts` is the closest
  existing analogue to extend.
- **Note**: there are currently **zero** frontend tests for items — no
  `*.test.tsx` and no `e2e/items*.spec.ts`. Abilities should not inherit that
  gap; a Compendium-tab e2e is part of this feature, not optional polish.

Tests require `DATABASE_URL` (from the repo-root `.env`) plus running Postgres
and RustFS containers — a bare `cargo test` without `.env` loaded fails with
`DATABASE_URL must be set`, which is an environment issue, not a code failure.

---

## 7. Reuse confirmations (no research needed, verified during mapping)

- **`pg_trgm` is already enabled** by spec 013's `enable_pg_trgm` migration —
  FR-007's "did you mean?" reuses it with a `gin_trgm_ops` index and the same
  `similarity(name, $1) > 0.4 … LIMIT 5` raw-SQL query. No new extension.
- **Share codes must derive from a v4 UUID, not v7.** `generate_share_code()`
  correctly uses v4. Spec 005 fixed a real production-risk collision bug where
  codes taken from a **v7** UUID's leading hex characters collided for anything
  generated in the same millisecond (v7 front-loads a timestamp).
- **Sidebar nav needs no change** — `WorldSidebarNav.tsx` already deep-links
  `/world/:id/compendium?tab=abilities`.
- **`ComingSoonTab.tsx` loses its last caller** when the Abilities tab becomes
  real. Deleting it is a one-line cleanup; decided at implementation time.
- **Moderation integration is mandatory, not optional** — items register
  `ModerationEntityType::WorldItem`, filter list queries through
  `moderation::filter_visible`, return a moderated placeholder from the detail
  query, and block the shared-preview query entirely. Without the equivalent,
  ability share links would be a moderation bypass for exactly the content the
  DMCA guardrail concerns.
