# Quickstart: World Abilities Compendium

Runnable validation scenarios proving the feature works end to end. Each maps to
a user story and its success criteria. Details live in
[data-model.md](./data-model.md) and [contracts/](./contracts/) — this is a
run/verify guide, not an implementation guide.

## Prerequisites

```bash
# From repo root. Containers must be up — tests and the dev stack both need them.
docker compose up -d postgres rustfs

# Migrations
diesel migration run

# Environment: DATABASE_URL comes from the repo-root .env and is NOT auto-loaded
# by cargo test. Without it every DB-backed test fails with
# "DATABASE_URL must be set" — an environment error, not a code failure.
set -a && source .env && set +a
```

## Running the checks

```bash
# Server — the primary coverage for this feature
cargo test -p thunderforge ability
cargo test -p thunderforge            # full suite; must stay green

# Frontend
cd apps/web
npx tsc --noEmit --ignoreDeprecations 6.0   # baseline has pre-existing unrelated errors
npx eslint src/... --max-warnings=0
npx vite build

# e2e — runnable in this sandbox: this feature has no Bevy canvas surface,
# so it escapes the documented "headless Chromium can't render the canvas"
# limitation that blocks canvas-interaction specs.
npx playwright test e2e/abilities-compendium.spec.ts --workers=1
```

Dev stack for manual verification:

```bash
pnpm dev     # or: cargo run -p thunderforge  +  pnpm -F @thunderforge/web dev
```

---

## Scenario 1 — Author an ability and see it in the Compendium (US1, SC-001/002/003)

1. Create a world as a GM; open `/world/:id/compendium?tab=abilities`.
2. **Verify**: a real searchable table with an empty state — **no "coming soon"
   text anywhere** (SC-001). `data-testid="compendium-coming-soon"` must not
   exist on the page.
3. Create an ability: name, description, classification. Confirm it appears
   **without a page reload** (SC-002).
4. Type part of its name into the search box → the table narrows (SC-003).
5. Select the row → the preview panel shows the summary beside the table.
6. Create a second ability with a **near-identical name** → an advisory
   "did you mean?" hint appears and **does not block** the create (FR-006/007).
7. Sign in as a non-GM member of the same world → the table is browsable and
   searchable, with **no create/edit/delete affordances** (US1 scenario 5).

## Scenario 2 — Record what the ability does (US2)

1. Open an ability's edit page as a GM.
2. Add an effect (type + formula + target) → it is saved and listed.
3. Add a second effect of a different type; edit one and remove the other →
   **the remaining effect is unaffected** (FR-017).
4. Try to save an effect with a whitespace-only formula → **rejected with a
   clear message, nothing persisted** (FR-018).
5. Preview the ability from the Compendium → its effects are shown.
6. **Verify** no dice are rolled and no effect is applied anywhere — effects are
   inert authored data (FR-019).

## Scenario 3 — An actor knows abilities (US3, SC-007)

1. Open an NPC's detail page as a GM → the known-abilities panel is present.
2. Attach two abilities → both listed (SC-007: under 15 seconds).
3. Attach one of them **again** → **no duplicate row, no error** (FR-021).
4. Detach one → it disappears from the actor but **still exists in the
   Compendium** (US3 scenario 6).
5. Delete an ability that an actor knows → **the delete succeeds**; the actor's
   entry survives, marked as referencing a deleted ability (FR-023).
6. As a user with only **Viewer** on the actor: the list is visible, but
   attach/detach controls are absent (US3 scenario 4).
7. **The key check** (FR-022): a user with Editor on the actor but only Viewer
   on an ability **can still attach it**. Permission follows the actor, never
   the ability.

## Scenario 4 — Lore cross-linking, both directions (US4, SC-005)

1. Author a lore entry containing `[[<ability name>]]`; save.
2. **Verify** the rendered link resolves and navigates to the ability
   (FR-028) — one interaction (SC-005).
3. Open that ability → the lore entry appears in its "Linked from (lore)" list
   (FR-029), with no manual bookkeeping.
4. In the lore editor, type `[[` and a prefix matching a lore entry, an actor,
   an item, **and** an ability → all four appear as **distinctly labelled**
   candidates (FR-030).
   - ⚠️ This is where the two pre-existing bugs surface: before the fix, items
     display as "Actor". Verify all four labels are correct
     (contracts/ability-lore-links.md).
5. Remove the link from the lore body and re-save → the ability's linked-from
   list updates.
6. Delete a linked ability → **the delete succeeds** and the link renders as
   broken/unresolved rather than blocking (FR-031).
7. **Determinism check** (FR-030a): create two abilities with the **same name**,
   link to that name from lore, and confirm it resolves to the
   earlier-created one — and keeps resolving there across repeated reloads, not
   arbitrarily flipping between them.

## Scenario 5 — Per-ability access control (US5, SC-004/004a)

Two independent mechanisms — verify both separately.

### 5a. Edit rights (ownership block)

1. As GM, set a second member to no-explicit-entry → they get **Viewer** by
   default (FR-024).
2. Set them to **Editor** → they can edit the ability but **cannot** change its
   ownership block (FR-026) or its GM-only flag (FR-024c).
3. Remove their entry entirely → they revert to implicit Viewer.
4. **Verify server-side enforcement independently of the UI** (SC-004): call
   `updateAbility` directly with an unauthorized session and confirm rejection.
   UI gating alone is not the test.
5. Confirm the GM retains full control regardless of ownership-block contents
   (US5 scenario 2).

### 5b. Visibility (GM-only flag) — SC-004a

**The leak check.** A miss on any one surface is a content leak, so walk all of
them. Mark an ability GM-only as a DM, then from a **non-DM member's** session
confirm it is absent from every one:

- [ ] the Compendium ability table
- [ ] the tab's search results (search its exact name)
- [ ] its detail route (`/world/:id/ability/:abilityId/view` → denied)
- [ ] "did you mean?" name suggestions when typing a similar name
- [ ] the catalog offered when attaching an ability to an actor
- [ ] an NPC's known-abilities list, where a DM attached it (FR-023) — and
      confirm **nothing hints** an entry was withheld: no placeholder, no count,
      no ordering gap
- [ ] `[[` link autocomplete candidates in the lore editor
- [ ] a lore entry that links to it → renders unresolved for this member, while
      the **same entry renders a working link for the DM** (FR-030b)

Then:

1. As DM, confirm the ability is visible and **clearly marked GM-only**
   (FR-024d).
2. Unmark it → it becomes visible to the member immediately, with no other data
   change (US5 scenario 3).
3. **Verify server-side** (SC-004a): call `ability` and `actorAbilities`
   directly as the non-DM and confirm the GM-only ability is denied/absent.
4. **Probe resistance**: confirm the `ability` rejection for a GM-only id is
   indistinguishable from a nonexistent id — a non-DM must not be able to test
   whether a hidden ability exists.
5. Confirm an Editor (non-DM) **cannot** clear the flag (FR-024c).

## Scenario 6 — Per-system presentation facets (FR-009..FR-013, SC-006)

The novel mechanism — verify the fallback chain carefully.

1. With a system that supplies **no** `abilityFacets` (every currently-shipped
   pack), open the Abilities tab → classifications render with **built-in
   default labels** (Spell/Feat/Power/Talent) (FR-011).
2. Add an `abilityFacets` block to one pack's `system.json` relabelling `spell`
   as "Scroll"; reload → **every** surface shows "Scroll": the table, the
   preview panel, the detail page, the create/edit picker, and the actor's
   known-abilities list (FR-012).
3. Supply a facet for only **one** classification → that one uses the facet
   label; the rest fall back to defaults (FR-011).
4. Supply a malformed entry (a bare string instead of an object, or an empty
   `label`) → falls back to the default **without throwing** and without
   breaking the page.
5. **The portability check** (FR-013, SC-006): with abilities authored, change
   the world's game system → **only the labels change**. Every ability, its
   classification, and its effects survive intact — **zero data loss**.
6. Give two classifications the **same** facet label → the authoring picker
   still presents them as distinct choices (spec Edge Cases).

## Scenario 7 — Share and copy (US6, SC-008) — gated on T001

> Requires the DMCA guardrail determination
> (`docs/adrs/20260825-049-share_link_dmca_repository_determination.md`) to be
> accepted first — task T001. Steps 7 and 8 below are not optional extras: they
> verify two of the six invariants that determination is conditional on.

1. As a member with **Owner** on an ability, generate a share link.
2. Open it from a session with **no membership** in the source world → a
   read-only preview: no edit controls, no ownership block, and **no way to
   identify the source world** (FR-033).
3. Choose "Copy to World" → the destination picker lists only worlds where the
   viewer holds DM access; a destination must be selected before anything is
   copied (FR-034).
4. Confirm → a new independent ability with cloned effects and an **empty**
   ownership block exists in the destination (FR-035).
5. **The independence check** (SC-008): edit the copy → the source is unchanged.
   Edit the source → the copy is unchanged.
6. Revoke the link → opening it shows "no longer available" rather than the
   ability's data (FR-036).
7. **Moderation check**: disable the ability via a takedown notice → the share
   link stops resolving. A share must never be a moderation bypass.
8. **Enumeration check** (FR-037): confirm there is no query returning a world's
   or a user's share links. A share must be reachable only by possessing its
   code.

---

## Definition of done

Status as of 2026-08-25 — verified where marked, with the gaps named rather
than quietly ticked.

- [x] `cargo test -p thunderforge` fully green — **332 passed, 0 failed**
      (293 before this feature; +39 new, zero regressions).
- [x] `e2e/abilities-compendium.spec.ts` passes on repeated runs — 3+
      consecutive clean runs of 4/4.
- [x] Frontend unit tests green — 20/20, including the facet fallback matrix.
      (`vitest` did not exist before this feature; see T019.)
- [x] `tsc` and `vite build` clean.
- [x] `data-testid="compendium-coming-soon"` appears nowhere — `ComingSoonTab`
      is deleted, asserted by the e2e (SC-001).
- [x] The two pre-existing lore-link bugs are fixed — `LoreLinkTargetKind`
      widened, and the autocomplete label ternary replaced with a total map, so
      item candidates stop displaying as "Actor" (research.md §3, defects 2-3).
- [x] ADR-049 Accepted 2026-08-25, accountable owner recorded, covering actor
      and item shares retroactively.
- [x] US6 shipped with all six ADR-049 invariants honoured — no-enumeration is
      structural (no listing query exists anywhere), and takedown-effectiveness
      has its own test.
- [x] Scenarios 1-6 covered by automated tests at the level each admits.

### Verified by test, not by hand

Scenarios 1-7 are covered by the automated suites rather than a manual
click-through: 39 server tests (including the full GM-only leak sweep, the
viewer-dependent link resolution, and every share invariant), 20 unit tests,
and 4 e2e tests. Two things are explicitly **not** hand-verified:

- **Scenario 5b's UI leak walk-through.** Every surface it lists is asserted
  server-side (`gm_only_ability_is_absent_from_every_non_dm_surface`,
  `gm_only_abilities_are_omitted_from_a_non_dms_known_list`,
  `gm_only_ability_is_unresolved_for_a_non_dm_reader`,
  `lore_link_targets_includes_abilities_and_hides_gm_only_from_players`), which
  is the stronger check — but no one has clicked through it as a player.
- **Scenario 6's facet rendering across every surface.** The resolver has a
  full fallback matrix in unit tests and Genie ships real facets
  (`spell`→Scroll, `talent`→Knack, with `feat`/`power` deliberately left to
  fall back), but the visual result has not been eyeballed.

### Known, not fixed

- **`eslint --max-warnings=0` does not pass** — 89 errors exist repo-wide at
  baseline, predating this feature. The new files add errors of the same
  `react-hooks/set-state-in-effect` class already present in their item
  counterparts. Deferred to a dedicated lint pass by the project owner's
  decision, rather than claimed as clean.
- **A pre-existing e2e flake** in the `/world/:id/compendium` route's lazy
  chunk (~1 run in 3 before mitigation) — verified pre-existing by stashing
  every `apps/web/src` change and reproducing the identical hang on the
  untouched baseline. `openAbilitiesTab` retries around it; the underlying
  dev-server issue deserves its own fix.
