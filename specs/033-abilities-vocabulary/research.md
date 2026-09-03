# Research: An Open Ability Vocabulary and a Guarded System Switch

**Spec**: `specs/033-abilities-vocabulary/spec.md` | **Date**: 2026-09-03

Researched against the code, not reasoned about. Every claim below has a file
and a line behind it.

---

## 1. Does the vocabulary belong to the manifest or to a pack crate?

**Decision**: the **manifest**, extending the block that already exists.

**Findings**:

- `abilityFacets` already ships in `packs/systems/genie/system.json:42-51`,
  relabelling `spell → Scroll/Scrolls` and `talent → Knack/Knacks`. No other
  pack declares it. So the idea, the file and one working example already
  exist; what is missing is the umbrella term, the ordering, membership, and
  the facets.
- Six other manifest blocks are read this way already — `abilities`
  (attributes), `sheet`, `groups`, `resources`, `turnStructure`, `movement` —
  each by a `*_for_system(systems_dir, system_id)` function over a
  filesystem-free `*_from_manifest(&Value)` half. `attributes.rs:9-21` is the
  canonical argument for manifest over Rust.
- ADR-054's line, restated in this spec's assumptions, is the test: a manifest
  may not declare a *capability no code can perform*, but naming and grouping
  that varies per ruleset is exactly what it is for. An ability type declares
  *that* it binds to items and *that* it is graded; the application performs
  both generically.

**Alternatives considered**: contributing the vocabulary from the pack's Rust
crate via `SystemContribution`, now that packs can. Rejected — it would make a
purely declarative naming table require a compiled crate, which excludes
manifest-only packs from a feature that is entirely about names, and ADR-029
says an outside pack is data. A pack with no crate must be able to name its
own types.

**Shape** (backwards compatible with the existing block):

```json
"abilityVocabulary": {
  "umbrella": { "label": "Spell", "pluralLabel": "Spells" },
  "types": [
    { "id": "spell", "label": "Spell", "pluralLabel": "Spells", "order": 0,
      "binds": "character", "grade": { "label": "Level", "min": 0, "max": 9 } },
    { "id": "enchantment", "label": "Enchantment", "pluralLabel": "Enchantments",
      "order": 1, "binds": "item" }
  ]
}
```

`abilityFacets` keeps working and is read as a `types` list carrying labels
only. It is not deleted: genie ships it, and breaking a shipped pack to tidy a
key name would be this feature failing its own premise.

---

## 2. What replaces the CHECK constraint?

**Decision**: **remove it.** Validity becomes "the world's assembled vocabulary
contains this identity", enforced server-side in the ability mutations.

**Findings**:

- `src/server/migrations/2026-08-25-120000-0000_create_world_abilities/up.sql:19-20`:
  `classification VARCHAR(16) NOT NULL CHECK (classification IN ('spell', 'feat', 'power', 'talent'))`.
  It is the only migration touching the column.
- FR-012 and SC-003 require zero changes to shared files when a system adds a
  type. A constraint enumerating valid values makes every new type a migration,
  and a migration is shared by definition. The two requirements cannot both
  hold.
- A table-wide constraint also cannot express the rule that actually applies.
  FR-013 says a type declared by one system must not be offered in a world
  running another — that is a *per-world* rule, and no column constraint can
  see the world's system.
- The column stays `VARCHAR(16)`. A length bound is a storage decision, not a
  vocabulary one, and 16 characters has not been a constraint anyone has hit.

**Alternatives considered**:

- *Keep the constraint and regenerate it per installed pack.* Rejected: it
  makes the database schema a function of which packs are installed, so
  installing a pack becomes a migration and uninstalling one makes existing
  rows invalid.
- *A `world_ability_types` table seeded from manifests.* Rejected for the
  reason ADR-028 gives for `game_systems`: it makes the database a cache of the
  filesystem with the filesystem still authoritative, and adds a staleness mode
  for no gain.

**Consequence, stated plainly**: after this, a bad `classification` can reach
the database only through a bug in the mutation layer rather than being
refused by Postgres. That is the same trust already placed in every other
authoring rule in this codebase — `gm_only`, permission levels, world
membership — none of which has a constraint either.

---

## 3. What happens to `AbilityClassification`?

**Decision**: retire the GraphQL enum and the TypeScript union; the wire type
becomes a **string identifier**, described by the vocabulary the same request
can fetch.

**Findings**:

- `src/server/src/graphql/types.rs:1046-1077` defines the enum with
  `as_db_str`/`from_db_str`. `apps/web/src/types/ability.ts:14` mirrors it as
  `"SPELL" | "FEAT" | "POWER" | "TALENT"`.
- A GraphQL enum cannot carry a value a pack invented. Introspection publishes
  a closed set, and a client validating against it would reject an
  Enchantment. This is the same reason spec 032's `DeclaredValue` carries an
  identifier and a declaration rather than a fixed struct.

**And it fixes a live bug.** `types.rs:1210` and
`mutations_ability_shares.rs:114` read an unknown stored value as
`from_db_str(..).unwrap_or(AbilityClassification::Spell)`. The comment is
honest about its intent — an older reader should not break on a newer row — but
the behaviour is that an ability of an unrecognised type is **silently
presented as a Spell**, which FR-034 forbids in as many words. Nothing can
write a fifth value today, so nobody has hit it; User Story 3 makes it
writable, so it is fixed in the same increment that creates the possibility.

---

## 4. Where does the vocabulary get assembled — server or web?

**Decision**: **server**, in `ability_vocabulary.rs`, following `attributes.rs`.
The web consumes the assembled answer.

**Findings**:

- Today it is web-only and read six times: `WorldCompendiumPage.tsx:100-123`,
  `AbilityCompendiumTab.tsx:21`, `AbilityPreviewPanel.tsx:9`,
  `AbilityDetailPage.tsx:39`, `ActorAbilitiesPanel.tsx:18`,
  `SharedAbilityPage.tsx:22` — each fetching the manifest and casting
  `abilityFacets` itself. FR-006 requires all of them to agree; six independent
  readers is six chances not to.
- The server needs it regardless. FR-013 (a type not offered in the wrong
  world), FR-019 (binding refused at the data boundary) and FR-023 (grade
  outside range refused) are all refusals only the server can make.
- Assembling it twice, in two languages, risks the two disagreeing about what
  FR-006 says must be identical.

**Alternatives considered**: keeping assembly in the web and having the server
validate from its own read. Rejected — that *is* assembling it twice, with the
disagreement hidden until a pack declares something the two halves parse
differently.

---

## 5. How is the system-change guard enforced against a direct call?

**Decision**: the mutation takes an **acknowledgement token derived from the
counts it is acknowledging**, and refuses when it does not match what the
server currently counts.

**Findings**:

- `update_world_game_system_impl` (`src/server/src/graphql.rs:2009-2066`)
  guards only DM-of-world and a non-empty id. There is no content check, no
  acknowledgement, and — unlike `update_world_interface_pack_impl` at
  `:1945-2007` — **no world event is recorded**, which is its own small
  asymmetry worth closing.
- The current "confirmation" in `WorldSystemSettingsPage.tsx:324-339` exists
  for spec 016's *legal notice*, not for data risk. Reusing it would make one
  control mean two different things.
- A boolean `acknowledged: true` satisfies the letter of FR-028 and none of its
  intent: a caller can pass it without ever having seen a count, and it would
  stay `true` if the world's content changed between reading and applying.

**Shape**: the inventory query returns the counts and a digest over them; the
mutation takes that digest; the server recomputes and refuses on mismatch. So
"I acknowledge" means "I acknowledge *these* numbers", and a world that gained
content while the dialog was open is re-confirmed rather than silently
switched.

**Alternatives considered**: a second `confirmToken` minted by the server and
stored. Rejected — it needs a table and an expiry policy for a dialog that is
open for seconds, and a digest of the counts is already exactly the thing being
acknowledged.

---

## 6. How is SC-003 checked automatically?

**Decision**: `scripts/check-ability-vocabulary.mjs`, modelled on
`check-system-registry.mjs`, wired into `scripts/verify.mjs`.

**Findings**:

- `check-interaction-seam.mjs` — ADR-054's check — greps one guarded file for
  forbidden words (`scripts/check-interaction-seam.mjs:43,65`), and its header
  argues for a word grep over an import graph because the likely violation is a
  `match` on ids with no new dependency.
- `check-system-registry.mjs` is the closer model: it derives the ids it
  forbids **from `packs/systems/` itself** rather than a list in the script, so
  the check cannot go stale as packs are added.

**Mechanism**: collect every type identity declared by any pack's manifest,
subtract the built-ins, and fail if any of those identities appears as a
literal in shared server or web code. A pack inventing `enchantment` is fine; a
shared file mentioning `"enchantment"` is the violation SC-003 measures.

---

## 7. Where does the ability↔item attachment live?

**Decision**: the **server's** migrations, as a peer of `world_actor_abilities`.

**Findings**:

- `world_actor_abilities` (`schema.rs:489-497`) is the existing character
  attachment: `actor_id`, nullable `ability_id` as a tombstone,
  `ability_name_snapshot`. The item counterpart mirrors it.
- No ability↔item link of any kind exists today, and items carry *effects*
  (`world_item_effects`), which the spec is explicit are a different concept at
  a different layer and are not merged.
- **No pack has ever shipped a migration** — `find packs -type d -name migrations`
  returns nothing. Spec 032 moved genie's *declarations* into its crate while
  its migrations stayed in the server's directory, and ADR-063 says splitting
  that is a separate decision.

The attachment is application content, not pack content: every system can bind
an ability to something, and a *facet* declared by a pack does not make the
*table* the pack's. Worth writing down because the opposite reads plausibly —
"the pack declared `binds: item`, so the pack owns it" — and it would make a
generic relationship pack-private.

---

## 8. What does the content inventory count?

**Decision**: per-kind counts for the world, plus the system each was authored
under, computed in one query when the dialog opens.

**Findings**:

- Nothing like it exists. The only per-world counting in shared code is
  incidental existence checks; `admin.rs:106-110` counts globally for the
  dashboard.
- FR-025 requires counts by kind — actors, abilities, items "and any other
  system-tagged content" — named against the system they were authored for.
  FR-037 requires abilities that will *become* unrecognised to be included,
  which is a function of the target system's vocabulary and cannot be counted
  without it.
- FR-029 needs "no authored content" to be answerable cheaply, since that is
  the common case for a world a GM just made and is still configuring.

**Consequence**: the inventory depends on the vocabulary (Increment A) for
FR-037's part of the count only. The rest of Increment C does not, which is why
C can ship before D and gain that column afterwards.
