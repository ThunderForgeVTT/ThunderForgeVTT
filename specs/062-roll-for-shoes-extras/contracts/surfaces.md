# Contract: the two screens

**Feature**: [../spec.md](../spec.md) · **Sheet contract from 061**: [../../061-roll-for-shoes/contracts/sheet.md](../../061-roll-for-shoes/contracts/sheet.md)

Two surfaces change: a new settings panel, and the existing actor sheet. Both are
pack files. No file under `apps/web/src` learns that Roll for Shoes exists
(FR-006).

---

## The settings panel — `web/src/panels/world-settings.tsx`

`world-settings` is an existing panel slot. A pack fills it by creating a file of
that name; `apps/web/src/panels/systemPanels.ts` discovers it with a build-time
`import.meta.glob` and `WorldSystemSettingsPage.tsx` mounts whatever
`resolvePanel(world.gameSystemId, "world-settings")` returns. Genie fills the
same slot. Nothing needs to be registered (research D2).

**Props**, fixed by `@thunderforge/host`:

```ts
{ worldId: string; world: WorldRecord; isGm: boolean; onWorldChanged: () => void }
```

`world` does not carry these settings — they are not on `worlds` — so the panel
reads them itself with `rollForShoesWorldSettings` through `postGraphQL`.
`onWorldChanged` is still called after a write, because the page holding the
panel may show other things that moved.

**What it shows**: five controls, in the spec's order — difficulty mode, the tie
rule, statuses, skill slots, starting skills. Each states what it does in the
table's language, and each is independent: changing one sends the other four back
unchanged (`graphql.md`).

**When `isGm` is false** the panel renders the settings read-only rather than
hiding. A player who can see the rules their table plays under is better served
than one who cannot, and the server refuses the write regardless (FR-007).

**Test ids**: `rfs-settings`, `rfs-setting-difficulty`, `rfs-setting-tie`,
`rfs-setting-statuses`, `rfs-setting-slots`, `rfs-setting-starting-skills`,
`rfs-settings-save`, `rfs-settings-error`.

---

## The actor sheet — `web/src/ActorSheet.tsx`

Every test id spec 061 established keeps its meaning: `rfs-sheet`, `rfs-error`,
`rfs-xp`, `rfs-description`, `rfs-opposition`, `rfs-skill-<id>`,
`rfs-roll-<id>`, `rfs-die-<index>`, `rfs-total`, `rfs-result`, `rfs-spend-xp`,
`rfs-advancement`, `rfs-advancement-name`, `rfs-advancement-confirm`,
`rfs-advancement-decline`. **With every setting off the sheet renders exactly
what it rendered before**, which is what lets `system-roll-for-shoes.spec.ts`
pass unmodified (FR-004).

The sheet reads the world's settings once on mount and renders the additions
each one turns on:

| Setting off | Setting on |
| --- | --- |
| `rfs-opposition` is a typed number | plus `rfs-difficulty-mode` and four band buttons, `rfs-band-<band>`; the typed number stays available |
| — | `rfs-gm-dice`, `rfs-gm-die-<index>`, `rfs-gm-total` for a rolled difficulty — a **separate** roll from the character's (research D7) |
| `rfs-total` is the sum of the dice | `rfs-total` is the sum plus the status modifier, with `rfs-modifier` showing the adjustment |
| — | `rfs-statuses`, `rfs-status-<id>`, `rfs-status-add`, `rfs-status-remove-<id>` |
| advancement always offers the new skill | at the cap, `rfs-advancement-no-room` explains, and `rfs-buy-slot` offers the purchase at `slotCost(level)` |

`rfs-die-<index>` always shows the character's dice and never a Game Master die,
so a test counting sixes counts the same faces `isAdvancement` reads.

**`canEdit`** continues to gate only the description. Rolling, spending XP and
advancement stay ungated, as spec 061 deliberately left them, and statuses and
slot purchases join that group: they are things a player does to their own
character during play, and the server's own authorisation is the real gate.

**Errors stay in `rfs-error`.** A refused write shows in the badge rather than
throwing, which is why every e2e assertion that a write landed must also assert
`rfs-error` has count 0 — without it a lost write reads as a disagreement about
a number.
