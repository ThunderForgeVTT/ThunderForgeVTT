# Contract: Interaction effects

This feature adds the first two *placeable* interactions a player meets on the
map. It extends the vocabulary established by ADR-054 and must obey that
decision's seam.

---

## The seam, restated

`InteractionPlugin` owns placement, hit-testing, trigger detection, permission
resolution, `once` bookkeeping and writing the activation event. **It owns no
effect at all.** Every effect is contributed by the subsystem that performs it,
and the authorable vocabulary is the union of what is compiled in.

`scripts/verify.mjs` greps the interaction core for subsystem words. An effect
added to the core rather than contributed would fail that check by design.

**Existing declared vocabulary**: `door.reveal`, `door.set_lock`,
`door.set_state`, `light.toggle`, `lore.open`, `nav.request_scene`.

---

## `lore.open` — already exists

**Used by**: FR-011, FR-012 (place a lore marker; activating it opens the entry).

**Contributed by**: the lore subsystem. No new effect is needed — this feature
supplies the authoring affordance and the presentation.

**Presentation (FR-012)**: rendered with a book icon; activation opens the lore
entry in a **separate browser tab**, leaving play uninterrupted. `lucide-react`
is already a dependency and supplies the icon.

**Player-visible outcome**: reading lore never disturbs the table.

---

## `item.pickup` — new

**Used by**: FR-013 through FR-017.

**Contributed by**: the **item subsystem**, not the interaction core
(research R3).

**Offered on activation (FR-014)**: inspection and pickup. Inspection opens the
item's page in a separate tab, consistent with lore.

**Effects of a successful pickup (FR-015)**: the item leaves the map for every
connected client, and enters the acting player's inventory.

**Authority**: the server decides. The engine may remove the token optimistically
— ADR-054 permits this explicitly — but a refusal restores it (FR-017).

**Concurrency (FR-016)**: two players activating pickup on the same item at the
same moment must result in exactly one inventory gaining it. The loser is told
it is gone. This is the same race spec 017 already settles for character claims;
reuse that resolution rather than inventing another.

**Permissions**: who may take a placed item into their inventory is governed by
the existing per-content permission model (spec 027 / ADR-050). A player acting
on the play screen is not automatically entitled to world content.

---

## Availability

ADR-054 requires that an interactive whose subsystem is absent be reported
**unavailable** to the GM — compared against the assembled registry before
dispatch, not discovered by noticing that dispatch did nothing. Both effects
here inherit that behaviour: a placed item in a build without the item
subsystem is a marker the GM cannot use, not a scene that fails to load.

---

## What this feature must not do

- Add either effect to the interaction plugin's own logic.
- Introduce a second dispatch path for "placed things" alongside the existing
  activation event.
- Let the engine be a second authority on whether a pickup succeeded.
