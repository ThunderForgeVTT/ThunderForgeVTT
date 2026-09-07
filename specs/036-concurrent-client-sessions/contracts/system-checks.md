# Contract: System-declared checks and `rollCheck`

Two surfaces: what a system pack declares, and how a sheet asks for it. The
sheet names a check; it never says what to roll.

## The manifest declaration

`packs/systems/<id>/system.json` may declare `checks`. A pack that does not is
valid and its sheets offer no check (FR-037).

```jsonc
"checks": [
  {
    "id": "strength",
    "label": "Strength",
    "group": "abilities",
    "formula": "1d20 + @modifier",
    "bindings": { "modifier": { "from": "ability", "key": "strength", "as": "modifier" } }
  },
  {
    "id": "athletics",
    "label": "Athletics",
    "group": "skills",
    "formula": "1d20 + @modifier",
    "bindings": { "modifier": { "from": "skill", "key": "athletics", "as": "modifier" } }
  }
]
```

- `formula` is a `thunderforge_dice` formula; placeholders use the named-
  placeholder syntax that crate already supports (ADR-044).
- `bindings` say where each placeholder's value comes from **on the actor**.
  How a raw value becomes a modifier is the *system's* business, declared here
  — `as: "modifier"` is a declaration, not a hard-coded 5e rule in the app.
- `dnd5e` gains this block from the `abilities` and `skills` it already
  declares. Six other packs keep their own `coreCheck` / `actionRoll` /
  `taskResolution` / `ladderRoll` / `skillRoll` / `manifestationRoll` keys
  untouched; those describe a core mechanic and are not this contract.

## The mutation

```graphql
extend type Mutation {
  """
  Roll a check the world's system declares, against one actor's values.
  Resolved server-side and recorded exactly as `rollDice` is.
  """
  rollCheck(worldId: UUID!, actorId: UUID!, checkId: String!): DiceRoll!
}
```

## Rules

1. The caller must hold at least the permission that any other action on that
   actor requires (`auth/actor_permissions.rs`). A sheet window is not a
   permission bypass.
2. `checkId` must be declared by the world's active system. An unknown id is
   refused; it is never interpreted as a formula.
3. The server resolves bindings against the actor's stored values and produces
   the result on the same authoritative path as `rollDice`. The client sends
   no formula, no values and no outcome.
4. The result is recorded and visible to the table on exactly the same terms
   as a roll made at the play field (FR-036) — a check rolled from the sheet is
   not a second kind of roll.
5. Callable from a companion surface **only while the server is reachable**.
   There is no offline form of this mutation, and a companion may not route it
   over a peer (FR-038, FR-039).

## What is deliberately absent

- No formula argument. A client that can name a formula is a client that can
  decide a roll.
- No target number or success/failure verdict. What a result *means* is the
  system's and the GM's; this contract produces the roll.
