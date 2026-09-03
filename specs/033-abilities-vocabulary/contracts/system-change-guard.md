# Contract: Changing a world's game system

**Spec**: `specs/033-abilities-vocabulary/spec.md` | **Date**: 2026-09-03

Covers User Story 2 (FR-024 to FR-033) and FR-037.

## What the operation does, and does not do

**It changes one column.** `worlds.game_system_id`, plus the interface-pack
re-pairing that already happens (`interface_packs::pack_after_system_change`,
`graphql.rs:2050`).

**It deletes nothing, rewrites nothing, re-tags nothing** (FR-024). Content
authored under the previous system stays stored exactly as authored and becomes
visible again if that system is made active again. The warning describes this
behaviour; it does not change it.

That is why the wording matters as much as the guard. A warning that says
"delete" would be false, and a false warning teaches GMs to distrust every
warning the application shows them.

## The query: what is at stake

```graphql
worldContentInventory(worldId: UUID!, targetSystemId: String): ContentInventory!
```

```text
ContentInventory {
  counts: [ContentCount!]!        # { kind, systemId, count }
  becomingUnrecognised: Int!      # abilities that lose their tab (FR-037)
  isEmpty: Boolean!               # FR-029's cheap answer
  digest: String!                 # over everything above
}
```

- `kind` covers actors, abilities and items, and any other system-tagged
  content that exists when this is built.
- `becomingUnrecognised` needs `targetSystemId` and the assembled vocabulary.
  With no target it is `0` and the field means "not asked".
- `isEmpty` answers FR-029 directly, so a fresh world takes the one-step path
  without the caller inferring it from counts.
- **DM-only.** The counts describe content a player may not be able to see.

## The mutation: acknowledged, not merely confirmed

```graphql
updateWorldGameSystem(input: {
  worldId: UUID!
  gameSystemId: String!
  acknowledgedDigest: String    # required when the world has content
}): World!
```

Server-side, in order:

1. **Authorization.** DM of the world (Owner or GM), regardless of
   acknowledgement (FR-031).
2. **No-op check.** Selecting the system already active returns unchanged, with
   no warning and no acknowledgement required (FR-030).
3. **Inventory.** Recompute. If the world has no content, apply the change —
   no digest required (FR-029).
4. **Acknowledgement.** If the world has content, `acknowledgedDigest` must
   equal the digest of what the server just counted. Absent or stale is
   **refused** (FR-028).
5. **Apply**, re-pair the interface pack, and record a world event — which this
   mutation does not do today, unlike its interface-pack sibling.

### Why a digest and not a boolean

`acknowledged: true` satisfies the letter of FR-028 and none of its intent. A
caller can pass it having never seen a count, which is precisely the bypass the
requirement exists to prevent; and it stays true if the world's content changed
while the dialog was open.

A digest over the counts means "I acknowledge **these** numbers". A world that
gained an actor between the dialog opening and the GM confirming is
re-confirmed rather than switched behind their back.

**Not a stored token.** That needs a table and an expiry policy for something
open for seconds, and the counts are already the thing being acknowledged.

## The interface: two distinct deliberate actions

1. The GM picks a target system. If the world has content, a **red** panel
   states:
   - the counts, by kind, with the system each was authored for, by display
     name — not id;
   - that affected content becomes **hidden, not destroyed**, and that
     switching back restores it;
   - what will be presented differently rather than hidden — abilities whose
     type the target system does not recognise;
   - the target system, by display name.
2. A second, distinct confirmation **naming the target system**. One is not
   enough (FR-027).

Cancelling at either step leaves the world's system and all content unchanged
(FR-032). After applying, the GM is told what became hidden and how to restore
it (FR-033).

**The existing single confirmation is not reused.** `WorldSystemSettingsPage.tsx:324-339`
exists for spec 016's legal-notice acknowledgement; making one control mean
both "I have read the licence" and "I accept this data consequence" would
weaken both.

## What "empty" means

A world with zero actors, zero abilities and zero items switches with no
warning and one action (FR-029).

**Scenes and lore do not count**, and that is load-bearing rather than an
oversight. Spec 010 guarantees every world is created with a default scene
already made, so counting scenes would mean no world is ever empty, FR-029
could never fire, and a GM would meet the red warning on a world they created a
minute earlier. This is the common case — a GM configuring a world they just
made — and it must stay cheap, or the guard becomes something people learn to
click through before it ever protects anything.
