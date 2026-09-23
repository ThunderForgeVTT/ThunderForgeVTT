# Contract: the contributed sheet

`packs/systems/roll_for_shoes/web/src/ActorSheet.tsx`, discovered by the
build-time glob over `packs/systems/*/web/src/ActorSheet.tsx`. Nothing
registers it; nothing outside the pack names it.

## What it is given, and nothing more

```ts
interface ActorSheetProps {
  actor: WorldActorRecord;   // carries id, worldId, gameSystemId, label
  canEdit: boolean;
}
```

That is the whole input. The world id comes from `actor.worldId`. There is no
router, no current user and no permission detail.

`canEdit` is **presentation only**: the actor page passes it on the edit route,
the play dock passes `false` always, and the server refuses an unauthorised
write regardless. Without it, every value is shown rather than edited.

## What it may import

`@thunderforge/host`, or nothing. From it this sheet uses:

| Import | For |
|---|---|
| `Card`, `Panel`, `Button`, `Input`, `StatusBadge` | every piece of chrome it draws |
| `useActorSystemData(actor.id, "roll_for_shoes")` | reading `trait_data` and `resource_data` |
| `useUpdateTraitData`, `useUpdateResourceData` | writing them |
| `postGraphQL` | issuing `rollDice` |
| `GraphQLRequestError` | telling a refusal apart from a crash |

## Reading and writing

A write **replaces the whole slot**, so every write spreads what it read, and
`refetch()` follows it. There is no subscription on actor system data.

```ts
await updateTraits({ ...traitData, skills: [...skills, granted] });
await refetch();
```

Every write and every roll is wrapped. A rejection is rendered in a
`StatusBadge`; it is never left as an unhandled rejection. Two refusals are
expected rather than exceptional — spending more XP than the character holds
(FR-030) and naming a skill with empty text (FR-037) — and both are refused by
the sheet before a mutation is sent, with the reason shown.

## Rolling

```ts
const { rollDice } = await postGraphQL<{ rollDice: RollResolution }>(
  `mutation RollSkill($input: RollDiceInput!) {
     rollDice(input: $input) {
       formula
       dice { numericSides rolls kept finalValue }
       resultKind
       resultValue
     }
   }`,
  { input: {
      worldId: actor.worldId,
      formula: "(LEVEL)d6",
      bindings: [{ name: "LEVEL", value: skill.level }],
  } },
);
```

- The placeholder must be parenthesised as a dice count. A binding that is
  absent is a refusal, not a zero.
- **The advancement check reads `dice[].finalValue`**, never `resultValue`. A
  sum cannot tell you whether every die showed a six.
- The verdict reads `resultValue` against the opposition, and only that.
- `MAX_TOTAL_DICE` is 1000. A larger pool is refused with no record written,
  and the refusal is rendered rather than clamped.

The roll is private: one row in `world_roll_records`, no world event, no chat
message. The other players see what the roller tells them, which is how the
game is played at a table.

## The loop it draws

1. **Skills** — the lineage, each beneath the skill it advanced from, each with
   its level and a roll button.
2. **Opposition** — one optional number, filled in with what the Game Master
   said. Empty means the roll will be unjudged.
3. **Result** — the dice as faces, the total, the number it was judged against,
   and one of *success*, *failure* or *unjudged*.
4. **XP** — the balance; `+1` on a judged failure; a spend control offering to
   turn one rolled die into a six, once per remaining non-six die.
5. **Advancement** — offered when every die shows six, rolled or bought. It
   asks for a name, says the name should be more specific than the skill rolled
   and relevant to what was attempted, accepts any non-empty text, and refuses
   an empty one. Declining creates nothing and changes nothing.

## Test ids

A contributed sheet has no `[data-slot="sheet-layout"]` wrapper, so it carries
its own, all prefixed `rfs-`:

| Test id | What it is |
|---|---|
| `rfs-sheet` | the sheet's root |
| `rfs-xp` | the XP balance |
| `rfs-skill-<id>` | one skill row |
| `rfs-roll-<id>` | that skill's roll button |
| `rfs-opposition` | the opposition input |
| `rfs-result` | the verdict line |
| `rfs-die-<n>` | one die of the current result |
| `rfs-total` | the current result's total |
| `rfs-spend-xp` | the buy-a-six control |
| `rfs-advancement` | the advancement prompt |
| `rfs-advancement-name` | its name input |
| `rfs-advancement-confirm`, `rfs-advancement-decline` | its two answers |
| `rfs-error` | the status badge a refusal is rendered in |

## The play dock

The same component mounts in the play dock with `canEdit: false`, compacted to
about 22rem, and there is no declarative fallback behind it. It must render
without edit permission and must not crash. Rolling is not gated on `canEdit`
— the dock is where a player sits during play.
