# Contract: Abilities as a lore in-text link target

The delta to spec 012 (lore) and spec 013's `item-lore-links.md` — adding a
**fourth** target kind (FR-028..FR-031).

## Link syntax (unchanged)

`[[Title]]` or `[[Title|Display Text]]`, per `markdown/links.rs`'s
`LINK_PATTERN`. No syntax change — abilities join the existing resolution set.

## Resolution precedence — append last

```text
lore entry → actor → item → ability → unresolved
```

Abilities append as the **last** resolution target (research.md §4). Inserting
earlier would silently change what already-saved links mean on their next
re-save; appending guarantees no existing link changes target.

Each step is a case-insensitive exact match (`.ilike(&title)`) scoped by
`world_id`, first match wins. Ambiguity across *kinds* is meant to be resolved at
authoring time by the `loreLinkTargets` autocomplete (FR-030); the cascade is
only the deterministic tie-break for a hand-typed title.

Ability href: `/world/{world_id}/ability/{ability_id}/view`

### Two additional rules for the ability branch (Clarifications, Session 2026-08-25)

**1. Deterministic duplicate-name resolution (FR-030a).** FR-006 permits two
abilities to share a name, and `[[Title]]` cannot say which is meant. The ability
lookup MUST therefore order explicitly — `ORDER BY created_at ASC LIMIT 1`,
earliest wins. Without it Postgres may return either row and the same link can
resolve differently between reads. **The existing lore-entry, actor, and item
branches have this same latent bug** and should get the same fix alongside.

**2. GM-only abilities are skipped for non-DM readers (FR-030b, FR-024b).** The
lookup adds `AND (NOT gm_only OR :viewer_is_dm)`. Consequences:

- A non-DM's link resolves to the earliest-created **visible** match, skipping a
  hidden one; a DM's resolves to the earliest-created match overall. The same
  title can legitimately resolve differently for a DM and a player — that is the
  intended effect of hiding, not a bug.
- Where no visible match remains, the link renders as an unresolved span for that
  reader.
- **The link text still shows the raw title**, so a GM-only ability's *name* is
  visible if the GM wrote it into player-readable prose. Accepted: the GM typed
  it there themselves, and the ability's data stays inaccessible. Hiding an
  ability does not retroactively censor the GM's own writing.

⚠️ **Resolution becomes viewer-dependent**, which the current code is not.
`markdown/links.rs`'s resolver takes no viewer today. Lore `rendered_html` is
re-rendered on every read (not served from a stored snapshot), so this is
achievable — but the render path must thread the viewer's DM status through to
the resolver. This is the single largest implementation cost in User Story 4;
budget for it rather than discovering it mid-task.

## Data-model delta

Mirrors spec 013's `add_item_target_to_world_lore_links` migration exactly —
four operations. Full DDL in [data-model.md](../data-model.md) §6.

1. `ADD COLUMN target_ability_id UUID REFERENCES world_abilities(id) ON DELETE SET NULL`
2. `CREATE INDEX world_lore_links_target_ability_id_idx`
3. widen `world_lore_links_target_kind_check` to include `'ability'`
4. widen the anonymous `world_lore_links_check` CASE sum from 3 terms to 4

`target_kind VARCHAR(16)` accommodates `'ability'` (7 chars) with no change.

### Two invariants that must not be "tightened"

- **`ON DELETE SET NULL`, never RESTRICT/CASCADE.** FR-031 requires deleting an
  ability to succeed even when lore links to it; the row must survive and render
  unresolved.
- **The "at most one target" CHECK stays looser than "kind must match the
  non-null column."** A stricter constraint would be re-evaluated when
  `ON DELETE SET NULL` fires and would block exactly the deletions FR-031
  requires to succeed. `target_kind` is authoritative only at insert time; every
  read path treats a null FK as unresolved regardless of the stored label.

## GraphQL delta

```graphql
enum GraphQLLoreLinkTargetKind { LORE_ENTRY ACTOR ITEM ABILITY }   # + ABILITY
```

- `lore_link_targets_impl` gains a fourth `results.extend(...)` block querying
  `world_abilities` by name prefix, so abilities appear as disambiguated
  autocomplete candidates (FR-030).
- New `lore_entries_linking_to_ability(state, target_ability_id)` in
  `graphql/queries/lore.rs` — a verbatim copy of `lore_entries_linking_to_item`,
  including its `moderation::filter_visible(state, "world_lore_entry", …)` pass
  (a DMCA-disabled source entry must not leak its title through a backlink list).
- `GraphQLAbility.linkedFromLore` — a `#[graphql(complex)]` field mirroring
  `GraphQLItem::linked_from_lore`. Use the `linkedFromLore` name (the newer
  convention; actors use the older `loreLinkedFrom`).

## ⚠️ Two pre-existing bugs this feature must fix first

Both affect **items today** and block correct four-kind labelling
(research.md §3, defects 2-3):

| Bug | Location | Fix |
|---|---|---|
| `LoreLinkTargetKind` is `"LORE_ENTRY" \| "ACTOR"` — the `ITEM` variant was never added to TS, though the backend has returned it since spec 013 | `apps/web/src/types/lore.ts:53` | Widen to all four kinds |
| Autocomplete labels every non-lore candidate `"Actor"` via a binary ternary, so item candidates already display as "Actor" | `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx:92` | Replace with a `Record<LoreLinkTargetKind, string>` label map |

These are genuine prerequisites, not opportunistic cleanup — a fourth kind
cannot be labelled correctly while a binary ternary decides the label.

## What needs no change

- **The renderer.** `LoreMarkdownRenderer.tsx` renders server-produced,
  ammonia-sanitized HTML via `dangerouslySetInnerHTML`; the client never
  resolves links. Server-generated `<a class="lore-link">` already covers
  abilities. Add `.scss` only if abilities should look visually distinct.
- **No new authorization surface.** Link resolution runs at lore-save time under
  the saving user's existing lore permissions, and resolves by name within one
  world. It exposes no ability data beyond a name and an href — the same
  position spec 013's `item-lore-links.md` records. Reading the linked ability
  still goes through `ability`'s own permission check (FR-025).

## Complete change checklist

| # | File | Change |
|---|---|---|
| 1 | migration `up.sql`/`down.sql` | 4 operations above, reversed in `down` |
| 2 | `schema.rs` | `target_ability_id` in `world_lore_links!` (**appended last, after `created_at`** — ALTER order); `joinable!(world_lore_links -> world_abilities (target_ability_id))` |
| 3 | `models.rs` | field on `LoreLink` and `NewLoreLink` (**field order must match `schema.rs`**) |
| 4 | `markdown/links.rs` | `PreparedLink.target_ability_id` (+ `None` in the other constructions), schema import, new cascade level with kind `"ability"` + href, precedence test |
| 5 | `graphql/mutations_lore.rs` | pass `target_ability_id` through `replace_lore_links` → `NewLoreLink` |
| 6 | `graphql/queries/lore.rs` | `lore_entries_linking_to_ability`; `ABILITY` enum variant; fourth branch in `lore_link_targets_impl` |
| 7 | `graphql/types.rs` | `GraphQLAbility::linked_from_lore` ComplexObject field |
| 8 | `apps/web/src/types/lore.ts` | widen `LoreLinkTargetKind` (fixes the shipped bug) |
| 9 | `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx` | ternary → label map (fixes the shipped bug) |
| 10 | ability detail page | "Linked from (lore)" card mirroring `ItemDetailPage.tsx` |

## Test expectations

- `resolves_link_to_existing_ability`.
- `item_wins_over_ability_on_title_collision` — pins the append-last precedence.
  (Note: the existing suite has `lore_entry_wins_over_actor_on_title_collision`
  but **no** actor-beats-item test; adding the ability-precedence test is worth
  doing alongside the missing item one.)
- `duplicate_ability_names_resolve_to_the_oldest` — two same-named abilities;
  the link resolves to the earlier-created one, stably across repeated reads
  (FR-030a).
- `gm_only_ability_is_unresolved_for_a_non_dm_reader` — the same lore entry
  renders a working link for a DM and an unresolved span for a player (FR-030b).
- `gm_only_ability_is_absent_from_link_candidates_for_a_non_dm` (FR-024b).
- `deleting_an_ability_nulls_referencing_lore_links_instead_of_blocking` —
  mirrors the item test; delete succeeds, link row survives with a null FK, the
  source lore entry is untouched.
- An ability candidate appears in `loreLinkTargets` output with kind `ABILITY`.
