# Quickstart: proving interface packs work

The mechanical half belongs to the suites listed at the bottom. This document is
the part a person has to look at, in the order it makes sense to look at it.

```bash
node scripts/dev.mjs        # frontend :5173, backend :30000
```

The backend is on **30000**, not 3000.

Prerequisites: a world you are the Game Master of, a second signed-in account
that is a player in that world, and both interface packs present in
`packs/interface/`.

---

## 1. The picker, and what it changes

**Where**: the world's settings, alongside the system settings card.

1. Open the world's appearance settings. The active pack is named — **Forge**
   for a world that has never chosen one, not "Not yet assigned" and not an
   empty select (FR-023, US1 scenario 3).
2. The list shows every pack in `packs/interface/`. Forge sits among them in
   title order with no badge, no pinned position, and no "(default)" suffix
   (FR-007, US1 scenario 6).
3. Preview the second pack without committing. The preview changes what you see;
   leaving without saving leaves the world on Forge.
4. Commit it. **Do not reload.** Every surface you open afterwards is drawn in
   the new pack — including the ones you had open (SC-001).

**What to actually check while it is applied**: open the world, a scene, a
character sheet, the admin sidebar, and a dialog. A pack that themes the
dashboard and forgets the dialog is the failure this step exists to catch, and
no assertion will catch it for you.

## 2. It is a look and nothing else

With the second pack active, walk the same surfaces you know under Forge and
confirm, specifically:

- Every button that was there is still there, and still enabled or disabled the
  same way.
- Every number reads the same. Token bars, sheet stats, currency, initiative.
- Nothing has moved out of reach — no control clipped, off-screen, behind
  another element, or unclickable (FR-011, FR-012).

This is SC-002, and it is a *pass of the product*, not a spot check. The e2e
covers the load-bearing screens; a person covers the rest.

## 3. The table sees one look

1. As the Game Master, change the world's pack.
2. On the second account, in the same world, **without reloading**: the look
   changes there too (SC-001, US1 scenario 1).
3. Both accounts open the same scene and the same character. Identical content,
   identical available actions, identical values (US1 scenario 2).
4. On the second account, toggle light/dark. It works, and it does not change
   what the Game Master sees (research.md §5).

Step 4 is the one worth being deliberate about. It is the accessibility escape
hatch that survived the decision to make the look table-wide, and it is the
thing most likely to be broken by an implementation that treats "the world's
appearance" as one indivisible thing.

## 4. As a player, you cannot set it

On the second account, find the appearance setting. It is either absent or
read-only, and any attempt to change it is refused with a message naming the
authority required — not a silent no-op and not a change that appears to work
and reverts (FR-010).

## 5. A pack that is not there

1. Move the second pack's directory out of `packs/interface/` while a world is
   bound to it.
2. Reload. The world opens, drawn in Forge, with **one** notice naming the
   missing pack. Not one per navigation (FR-018).
3. Nothing is blocked. Move around, edit something, roll something.
4. Put the directory back and reload. The world returns to that pack with no
   re-binding step.

## 6. A pack that should be refused

Edit a pack's `interface.json` to fail, one at a time, and confirm each is
refused *and says why*:

| Change | Expected refusal |
|---|---|
| Add any key the contract does not name | Names the unknown key |
| `"type": "system"` | Names the exclusivity rule (FR-002) |
| Set `foreground` and `background` to near-identical colours in `light` only | Names the pair, the ratio, the requirement, **and that it was the light mode** (FR-012a, SC-003a) |
| Break a colour value | Names the value, does not fall back |
| Remove `legal` | Names the missing legal metadata |

The mode in the third row is the detail worth verifying by hand. A pack that
reads fine in dark and fails in light is the common failure, and a message that
says only "contrast too low" sends an author to the wrong half of their file.

---

## What the suites cover, so you do not repeat it

| Check | Where |
|---|---|
| Manifest structure, unknown keys, id/directory match, type exclusivity | `cargo test -p pack_system_spec` |
| Contrast ratios, per mode, per pair | `cargo test -p pack_system_spec` |
| Mutation authorization, missing-pack rejection, world event recorded | `cargo test -p thunderforge` |
| Token overlay, fallback to Forge, light/dark selection | `pnpm --filter web test` |
| GM sets a pack and a second browser context sees it without reloading | `apps/web/e2e/world-appearance.spec.ts` |
| A player is refused | same |
| The two labels say the same true thing | same |

Run them with `pnpm verify` for the lint gates and the suites named above for
behaviour. Neither answers §1's "did you look at a dialog", which is why this
document exists.
