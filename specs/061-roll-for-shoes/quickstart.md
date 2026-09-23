# Quickstart: Roll for Shoes

**Feature**: `specs/061-roll-for-shoes` | **Date**: 2026-09-22

How to run this pack and see the game work end to end, and how it is proven.

---

## Prerequisites

The pack's web module is built like every other pack's — `tsc && vite build`
into `web/dist/` — and its server crate is linked into the app binary, so
**the stack must be rebuilt after the pack is added**, not just restarted.

`scripts/dev.mjs` links `packs/systems/` into the server's data directory on
every start, so a pack added to the repository is offered the next time the
stack comes up. There is no registration step and no database row.

```bash
pnpm install
pnpm --filter @thunderforge/roll-for-shoes build   # web/dist
pnpm dev                                            # stack, with the pack linked
```

---

## Validation scenario — the whole game in one sitting

The game is six rules and this walks all six.

1. **Make a world on the system.** Create a world, open
   `/world/<id>/settings/system`, pick **Roll for Shoes** from the picker, and
   confirm the legal notice it shows. The notice names CC0 1.0 and credits Ben
   Wray and rollforshoes.com, and nothing else.
   *Proves*: the pack is discovered, its manifest is served, and its legal
   metadata passes the enforced check (FR-001 – FR-006).

2. **Make a character.** Create an actor in that world and open its sheet at
   `/world/<id>/actor/<actorId>/edit`.
   *Expect*: a name, an empty description, **XP 0**, and exactly one skill —
   **Do Anything**, at level 1, with a roll button (FR-007 – FR-011).

3. **Roll it, unjudged.** Leave the opposition empty and roll Do Anything.
   *Expect*: one die, a face between 1 and 6, the total, and the word
   **unjudged**. XP stays at 0 — an unjudged roll is not a success and pays
   nothing (FR-023, FR-026).

4. **Roll it against something, and fail.** Enter an opposition of **6** and
   roll. A single d6 cannot beat 6.
   *Expect*: **failure**, the 6 shown as what it was judged against, and **XP
   1** (FR-021, FR-022, FR-025). Roll again for **XP 2**.

5. **Buy the advancement.** With the single die showing anything but a six,
   spend 1 XP to turn it into a six.
   *Expect*: the die reads as a six for advancement, the **verdict is still
   failure**, the total is unchanged, and the XP that failure awarded is
   untouched (FR-027, FR-028, FR-029). Then try to spend again with a balance
   of 0: refused, and the refusal says the balance is too low (FR-030).

6. **Name the skill.** The advancement prompt appears. Confirm it refuses an
   empty name, then decline it — nothing is created and the verdict, dice and
   XP are unchanged (FR-037, FR-038). Reach the prompt again and this time name
   something more specific, such as *Kick A Door Down*.
   *Expect*: a second skill at **level 2**, shown beneath **Do Anything**,
   which is still there (FR-033, FR-035, FR-039, FR-043).

7. **Roll the new skill.** It rolls **two** dice (FR-019, FR-024).

8. **Reload the page.** Everything persists — XP, both skills, the lineage
   (FR-032).

9. **Open the play dock** and look at the same character. The sheet renders
   compacted and read-only, and does not crash without edit permission.

---

## What to check while you are in there

- The system shows **no round counter** and **no initiative** — it declares no
  rounds.
- The character sheet shows **no attributes, no hit points, no inventory**.
  The game has none, and the pack declares none.
- A token for the character carries an **XP counter**, because XP is declared
  as one.

---

## Proof

```bash
pnpm e2e:game-systems
```

The pack's directory is in that slice's `paths`, and its end-to-end
specification is named so the slice's `system-` prefix owns it. The slice also
runs `system-settings`, `system-panel-slots` and `system-change-guard`, and its
neighbour `status-systems` — the seams a new pack crosses.

**A roll cannot be forced.** There is no seed and no deterministic-roll hook.
The specification asserts what is true of any roll — one die per level, six
sides each, every face between one and six, the total equal to their sum, the
verdict following from the total and the opposition — and drives the branches
that need a particular face through the data path rather than by wishing for
dice. Step 4 above is the one outcome that *is* deterministic: a single d6
cannot exceed 6, so that failure and its XP are assertable every run.

## Also run

```bash
pnpm verify      # registry, packdocs, filelength, lint and format checks
cargo test -p roll_for_shoes_server
```

`check-system-registry.mjs` is the one that matters most here: it fails the
build if this system's id appears anywhere in shared server code beyond the
single linkage line, which is what SC-008 claims.
