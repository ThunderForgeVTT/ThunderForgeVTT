# Quickstart: proving interface packs work

The mechanical half belongs to the suites listed at the bottom. This document is
the part a person has to look at, in the order it makes sense to look at it.

```bash
node scripts/dev.mjs        # frontend :5173, backend :30000
```

The backend is on **30000**, not 3000.

Prerequisites: a world you are the Game Master of, a second signed-in account
that is a player in that world, both interface packs present in
`packs/interface/`, and at least two worlds bound to different game systems —
one of them Genie, which is the only system `IMPLEMENTED_SYSTEM_IDS` contains.

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

## 6. The sheet is the system's shape, not ours

This is the part the whole revision exists for, and no assertion covers what it
feels like.

1. Open an actor in a **Genie** world under Forge. Three attributes, Health and
   Wish Points as bars. No skills section — Genie declares none, and an empty
   heading over nothing is the failure to look for.
2. Open an actor in a world bound to a system Forge has never heard of. Same
   generic arrangement, that system's values, nothing blank or mislabelled
   (US1 scenario 8).
3. Switch that world to the targeted pack. The arrangement changes to that
   system's own shape — its grids, its trackers — and the *numbers do not*
   (FR-011).
4. Find a derived value on screen — a modifier, a passive, a save total. Confirm
   it is **not editable**, and that the stored value it comes from is. If both
   are editable, `origin` is not reaching the surface and the two will disagree.

## 7. A pack that should be refused

Edit a pack's `interface.json` to fail, one at a time, and confirm each is
refused *and says why*:

| Change | Expected refusal |
|---|---|
| Add any key the contract does not name | Names the unknown key |
| `"type": "system"` | Names the exclusivity rule (FR-002) |
| Set `foreground` and `background` to near-identical colours in `light` only | Names the pair, the ratio, the requirement, **and that it was the light mode** (FR-012a, SC-003a) |
| Break a colour value | Names the value, does not fall back |
| Remove `legal` | Names the missing legal metadata |
| Reference an identifier the target system does not declare | Names the identifier **and** the system (FR-026, SC-003b) |
| Set `targets: []` while the layout names an identifier | Names the identifier, and says an untargeted pack must be generic |
| Add a layout construct to the format without using it in Forge | Forge's conformance test fails (FR-007a) |

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
| Derived values: purity, and the same stored input giving the same output | `cargo test -p thunderforge_canvas_core`, `cargo test -p genie_server` |
| No hand-written list of system identifiers in shared server code | `node scripts/check-system-registry.mjs` |
| GM sets a pack and a second browser context sees it without reloading | `apps/web/e2e/world-appearance.spec.ts` |
| A player is refused | same |
| The two labels say the same true thing | same |

Run them with `pnpm verify` for the lint gates and the suites named above for
behaviour. Neither answers §1's "did you look at a dialog", which is why this
document exists.

---

# Increment F (User Story 2) — validation

Run against a live stack. Start it with the rate-limit bypass, or repeated page
loads trip `/authentication/*`'s 40-per-minute cap and the app renders "could
not load the current instance state", which looks exactly like a broken
feature:

```bash
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 node scripts/dev.mjs
```

## F1 — the application no longer knows which systems exist

```bash
# Every bundled pack, read from the directory rather than an empty table.
curl -s localhost:30000/api/systems | python3 -m json.tool | head -20

# And nothing in shared web code names a system.
grep -rn "BUNDLED_SYSTEM_IDS\|BUNDLED_SYSTEM_LABELS" apps/web/src/   # expect: no matches
```

**Expected**: the route lists all bundled systems with their titles; both
literals are gone; the create-world and system-settings pickers still offer
every system, now sourced from the server.

**The real test of SC-004**: add a directory under `packs/systems/`, restart,
and confirm it is offered — with no other file edited.

## F2 — a pack contributes behaviour

```bash
node scripts/check-system-registry.mjs    # expect: 0 known violations
```

**Expected**: zero. That script's `KNOWN` list is empty and the exemption for
`graphql.rs` is gone, because world creation no longer branches on `"genie"`.

By hand: create a world on Genie, then confirm its session row exists and was
created by the pack's hook rather than by shared code. Create a world on 5e and
confirm no genie row appears.

## F3 — a failing surface is contained and named

Injected, not waited for. With a surface made to throw:

1. Open an actor in a world whose system contributes that surface.
2. **Expected**: the surface is replaced by a message naming the pack and
   saying what is unavailable. The rest of the page — navigation, inventory,
   abilities, lore — still works. Nothing is blank.
3. Navigate to a different actor. **Expected**: the boundary resets and the
   next actor renders; one actor's bad data does not condemn the next.

## F4 — the contract stands on its own

The honest test of SC-010 is not mechanical, and this is the part that needs a
person:

1. Read `packs/systems/README.md` **without opening any source file**.
2. Write a minimal system pack from it alone: a manifest, one ability, one
   resource, and a turn structure.
3. Install it and bind a world to it.

**Expected**: it works, and every document the README references exists.
Anywhere step 2 required reading `sheet.rs` or `attributes.rs` to proceed is a
gap in the contract, and the gap is the finding.

