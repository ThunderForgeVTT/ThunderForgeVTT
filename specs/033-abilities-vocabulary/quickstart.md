# Quickstart: An Open Ability Vocabulary and a Guarded System Switch

**Spec**: `specs/033-abilities-vocabulary/spec.md` | **Date**: 2026-09-03

What a person checks by hand, and what the suites already cover so you do not
repeat them.

```bash
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 node scripts/dev.mjs
```

That environment variable is not optional. `/authentication/*` is rate-limited
40/minute per IP and the browser hits `setup/status` on every page load; a
multi-test run trips it and the app renders "ThunderForge could not load the
current instance state", which reads as a broken feature and is not. Use
`--workers=1` against a single stack.

**Prerequisites**: a world you are GM of, holding abilities of at least two
types; a second world on a different system; a signed-in player in one of them.

---

## 1. The tab set, in the system's words (US1)

1. Open a world running **genie** → Compendium.
2. **Expected**: the outer tab reads the system's umbrella word, not
   "Abilities". Inside, one tab per type — **Scrolls** and **Knacks** where
   genie relabels them, the built-in words for the rest. Each tab carries a
   count.
3. Select a tab. **Expected**: only that type is listed, and the count equals
   the rows.
4. Create from inside a tab. **Expected**: the new ability is that type and you
   were not asked to choose one.
5. Select a tab with nothing in it. **Expected**: it says it is empty and
   offers creation. It does **not** show other types' abilities.
6. Open the same world as a **player**. **Expected**: identical tabs and
   labels; only the abilities within differ.

Then a system that declares nothing — **blades_in_the_dark**:

7. **Expected**: "Abilities", and all four built-in tabs, fully labelled. No
   blank tab, no missing tab, no error. This is the case SC-013 measures and
   the one most likely to be broken by a change that only ever got tested
   against genie.

Then the rule that keeps the tab set honest (FR-011a) — a system that declares
some built-ins but not others:

8. **Expected**: a built-in the system neither uses nor the world holds any of
   gets **no tab**. Author one ability of that type. **Expected**: the tab
   appears. Delete it again. **Expected**: the tab goes. Presence follows use,
   and content is never hidden.

---

## 2. The guarded switch (US2)

**On a world with content:**

1. World Settings → System → pick a different system.
2. **Expected**: a **red** panel naming real counts by kind — "12 actors, 30
   abilities, 4 items authored for Genie" — the target system by display name,
   and plainly that content becomes **hidden, not deleted**, restorable by
   switching back. Check the numbers against the compendium. They must match
   exactly (SC-006).
3. **Expected**: the warning never says "delete", "lose" or "destroy" (FR-026).
4. Confirm once. **Expected**: not applied. A second, distinct confirmation
   naming the target system is required (FR-027).
5. Cancel at each step in turn. **Expected**: system and content unchanged.
6. Complete it. **Expected**: you are told what became hidden and how to get it
   back.
7. Count actors, abilities and items again. **Expected**: identical to before
   (SC-005).
8. Switch back. **Expected**: everything visible again, nothing renamed or
   re-typed (SC-008).

**On a fresh, empty world** — one created a minute ago and not touched:

9. **Expected**: no red warning, one action, done (FR-029). Its auto-created
   default scene does **not** make it non-empty; only actors, abilities and
   items count. If this world gets the warning, the empty check is counting
   scenes and FR-029 has become unreachable.

**Selecting the system already active:**

10. **Expected**: nothing happens, and nothing is shown (FR-030).

**Without the interface** — the half that matters most:

```bash
# Expect refusal: the world has content and no acknowledgement is offered.
curl -s -b cookies -X POST localhost:5173/api/graphql \
  -H 'Content-Type: application/json' -H "X-CSRF-Token: $CSRF" \
  -d '{"query":"mutation($i:UpdateWorldGameSystemInput!){updateWorldGameSystem(input:$i){id}}",
       "variables":{"i":{"worldId":"<world>","gameSystemId":"dnd5e"}}}'
```

11. **Expected**: refused (FR-028, SC-007). Repeat with a stale digest — take
    one, add an actor, then send it. **Expected**: also refused.

---

## 3. Where the halves meet (FR-034 to FR-038)

1. In a 5e world, author an ability of a 5e-declared type (an Enchantment).
2. Switch the world to genie, acknowledging the warning.
3. **Expected**: the Enchantment is **still listed**, in a **final tab** of the
   same tab row, clearly marked as unrecognised and offering no creation. It is
   labelled `enchantment` — the stored identity, plainly, not a label fetched
   from 5e's manifest. It opens, it edits, it deletes. It is **not** shown as a
   Spell — the behaviour today's `unwrap_or(AbilityClassification::Spell)`
   would produce, and the thing FR-034 forbids.
4. **Expected**: the warning in step 2 counted it (FR-037).
5. Switch back to 5e. **Expected**: it returns to its own tab with its own
   label, unchanged (SC-008).
6. Re-type it deliberately to a recognised type. **Expected**: allowed, and
   only because you asked (FR-038).

---

## 4. Facets (US4)

1. A type declaring `grade`. **Expected**: the grade shows on every surface
   showing the ability, in the **system's** word for it — "Level" in 5e — and
   an ungraded type shows no grade anywhere (SC-010).
2. Author a grade outside the declared range. **Expected**: refused.
3. Narrow a type's range in the manifest, restart, and reopen an ability
   already outside it. **Expected**: retained and displayed, not clamped
   (FR-023).
4. Attach an item-binding ability to an item. **Expected**: it appears on the
   item, beside that item's effects, identified as what it is, not duplicated.
5. Attach it to a character instead — through the API, bypassing the interface.
   **Expected**: refused server-side (FR-019, SC-011). A type binds to exactly
   one kind of subject, so there is no arrangement in which both succeed.

---

## What the suites cover, so you do not repeat it

| Check | Where |
|---|---|
| Vocabulary assembly: re-labelling, malformed entries, blank-label fallback, collisions | `cargo test -p thunderforge-server` (`ability_vocabulary::`) |
| Grade range refused at authoring; out-of-range retained | same |
| Binding refused at the data boundary | same |
| Acknowledgement absent or stale is refused | same (`graphql::` world system tests) |
| Content counts match seeded content | same |
| A new type for one system touches no shared file | `node scripts/check-ability-vocabulary.mjs` |
| Label resolution and fallbacks in the browser | `pnpm --filter web test` |
| Per-system tab sets, four systems | `apps/web/e2e/abilities-vocabulary.spec.ts` |
| Two confirmations, counts shown, cancel leaves unchanged | `apps/web/e2e/system-change-guard.spec.ts` |

Run `pnpm verify` for the gates. None of it answers §1 step 7 — "does a system
that declares nothing still look right" — which is why this document exists.
