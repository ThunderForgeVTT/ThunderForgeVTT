# ADR-113: The Rules of a Fight Are One Crate

**Date:** 2026-10-06
**Status:** **ACCEPTED** 2026-10-06. Decisions 1 to 5 are proven by spec 079 User Stories 2 and 3: the server calls `crates/thunderforge-combat`, its 111 combat tests pass unedited, and a scripted fight comes out the same through the server and through the crate alone (`combat/parity_tests.rs`). Decision 6 is built and measured; the demo does not call it yet (spec 079 Phase 4).
**Participants:** ThunderForgeVTT Team
**Related:** spec 079 (FR-001 to FR-009), spec 046 (the fight), spec 074 (the demo), [ADR-111](./20261004-111-four_homes_and_why_packs_is_its_own.md) (four homes), [ADR-062](./20260902-062-packs_extend_the_engine_with_data_not_code.md) (packs extend the engine with data, not code)

---

## Problem Statement

The demo runs the real web app against a small stand-in for the server, in
the browser. The owner wants a visitor to fight the Grassy Path Ambush there.
Spec 046's fight lives in `crates/thunderforge-server/src/combat/`, and nearly
every rule in it is written between a database read and a database write.

The demo could have a fight of its own, written in TypeScript. Then there
would be two sets of combat rules, and the demo would advertise a game the
product does not play the day one of them changes. The owner chose the other
way: "move it into a shared code state like thunderforge-combat and have it
be shared logic essentially via wasm."

## Decision

1. **The rules of a fight are one crate, `crates/thunderforge-combat`.** It
   holds what an attack decides, the hit point arithmetic, turn order, whose
   turn holds a token back, the turn budget, reach and line of sight, sizes,
   and the pack's `combat` and `turnStructure` shapes. It depends on no
   database, network, engine or async runtime: `thunderforge_dice`,
   `thunderforge_canvas_core`, `serde` and `rand_core`, as those two crates
   already do.

2. **The server loads, calls the crate, and saves.** What stays in the server
   is a database question first: transactions, locks, roll records, offers,
   events, redaction by viewer, legendary and lair bookkeeping. A rule moves
   when it can be stated without a connection; a rule that is two lines inside
   a query stays until something other than the server needs it.

3. **The server's tests do not move.** A server module re-exports what moved
   and keeps a wrapper where a signature changed, so every test reaches what
   it reached before by the path it used before. An extraction that has to
   edit the tests that prove it changed nothing has not proved it.

4. **A type the schema exposes is defined once, in the crate.** `ActionCost`,
   `HitPointChangeKind`, `BudgetLine` and `TurnBudget` derive `async_graphql`
   behind a `graphql` feature the server turns on; the manifest shapes derive
   `JsonSchema` behind `schema` for `pack_system_spec`. Mirrored types in the
   server would have to be kept in step by hand, and a drift would change the
   public schema without anyone deciding to.

5. **The crate never owns entropy.** A roll takes an injected RNG, as
   `thunderforge_dice` does. The server passes its own; the crate offers
   `SeededDice` for play and `ScriptedDice` for a test that has to say "the
   goblin rolls a 14". The order the dice are drawn in is part of the rule —
   to-hit, then damage only on a hit — so a fight under fixed dice is one
   fight wherever it runs.

6. **The browser reaches it through a `wasm` feature, JSON in and JSON out,**
   as `thunderforge-pdf` does. The feature is off by default, so the server
   does not carry `wasm-bindgen`. The demo loads the module on its first combat
   operation, not on page load. Measured 2026-10-06: 412,578 bytes raw,
   118,073 brotli.

7. **A system's own rules stay in its pack (ADR-111).** The crate names no
   system's fields and reads what it needs from the pack's manifest. 5e's
   arithmetic stays in `packs/systems/dnd5e`.

## Consequences

- A change to how a fight resolves is made once, and the demo and the server
  agree by construction. The parity test is the check that they still do.
- The crate is the place a combat rule is tested without a database; the
  server's tests remain the proof that the server's behaviour did not change.
- Combatant shapes differ between the server (a diesel row) and the demo (a
  JSON object), so turn order and the turn check take a trait (`Seat`,
  `Party`) rather than a type. A rule that needs more of a combatant adds a
  method to the trait.
- Redaction, legendary and lair rules are still server-only. If the demo needs
  them, they move by decision 2's test, not wholesale.
