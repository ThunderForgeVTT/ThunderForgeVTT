# Contract: the pack's two root GraphQL fields

**Feature**: [../spec.md](../spec.md) · **Storage decision**: [../research.md](../research.md) D1

`roll-for-shoes-server` contributes two root fields. They are the only way the
world's Extras settings are read or written, and they are the pack's entire
network surface. Nothing in `src/server/src` names them.

---

## Query — `rollForShoesWorldSettings`

```graphql
rollForShoesWorldSettings(worldId: ID!): RollForShoesWorldSettings!
```

```graphql
type RollForShoesWorldSettings {
  worldId: ID!
  difficultyMode: String!      # "free" | "rolled" | "target"
  tieSucceeds: Boolean!
  statusesEnabled: Boolean!
  skillSlotsEnabled: Boolean!
  startingSkills: [RollForShoesStartingSkill!]!
}

type RollForShoesStartingSkill {
  name: String!
  level: Int!
}
```

**A world with no row is not an error.** The field returns the defaults —
`difficultyMode: "free"`, three `false`s, an empty `startingSkills` — so a world
that predates this feature and a world whose Game Master has touched nothing are
indistinguishable, which is what FR-001 requires.

**Authorisation**: any member of the world may read. The settings describe how
the table plays and every player needs them to roll; there is nothing here to
keep from a player.

**Play/pause**: classified `reads` in the pack's `PackSurface`. A paused world's
settings are still readable — pausing stops play, not looking.

**Who calls it**: the pack's `world-settings.tsx` panel and its
`ActorSheet.tsx`, both through `postGraphQL` from `@thunderforge/host`, as
Genie's panel does. The host's `WorldRecord` does not carry these settings and
is not extended to.

---

## Mutation — `updateRollForShoesWorldSettings`

```graphql
updateRollForShoesWorldSettings(
  input: UpdateRollForShoesWorldSettingsInput!
): RollForShoesWorldSettings!

input UpdateRollForShoesWorldSettingsInput {
  worldId: ID!
  difficultyMode: String!
  tieSucceeds: Boolean!
  statusesEnabled: Boolean!
  skillSlotsEnabled: Boolean!
  startingSkills: [RollForShoesStartingSkillInput!]!
}

input RollForShoesStartingSkillInput {
  name: String!
  level: Int!
}
```

**Every field is required, and the mutation is a whole-row upsert.** The caller
reads the settings, changes one, and sends all five back. This is deliberate: an
input of optionals would make "absent" ambiguous between "leave it" and "clear
it", and FR-002's guarantee that enabling one setting never silently changes
another is easiest to hold when the caller states all five each time.

**Authorisation**: Game Master of the world only, via
`auth::world_membership::is_dm_of_world` — the same check every world setting
uses. A player receives a refusal, not a silent no-op (FR-007).

**Play/pause**: classified `gated` in the pack's `PackSurface`, with a request
document, and the resolver calls `refuse_world_if_paused`. Without both,
`play_pause_surface_tests` fails. Configuring a paused world's rules is a change
to the world, which is what pausing stops.

**Validation, refused rather than coerced**:

| Input | Refusal |
| --- | --- |
| `difficultyMode` not one of the three | "Unknown difficulty mode" |
| A starting skill's `name` empty after trimming | "A starting skill needs a name" |
| A starting skill's `level` below 1 | "A starting skill's level must be at least 1" |

**Idempotent**: sending the settings a world already has succeeds and changes
only `updated_at`. The panel does not need to diff before writing.

**What it does not do**: it writes no actor, touches no character, and triggers
no recalculation. Turning a setting off leaves every character exactly as it was
— which is what makes each Extra reversible.

---

## What is *not* contributed

- **No mutation for statuses or bought slots.** Both live in the character's
  existing JSON and are written through the host's `updateActorSystemData`, which
  the pack's sheet already uses and the pack's `validators.rs` already gates.
  Adding a pack mutation for them would duplicate an authorisation path that
  already works.
- **No mutation for rolling.** Difficulty dice go through the host's existing
  `rollDice` with a `(BAND)d6` formula binding, exactly as a skill roll goes
  through it with `(LEVEL)d6` (research D7). The dice stay server-rolled and
  auditable, and no new formula syntax is invented.
- **No subscription.** A settings change reaches other clients the next time
  they read, which for a sheet is its next mount. A Game Master changing the
  rules mid-roll is not a case worth a live channel.
