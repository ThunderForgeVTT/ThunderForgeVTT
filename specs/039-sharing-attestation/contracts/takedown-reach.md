# Contract: The reach of a takedown

**Gated on ADR-094.** This contract describes behaviour that requires reversing
spec 026 FR-012's "no referential link back to the source". Until the
accountable owner accepts that determination, none of it is buildable and
FR-022 through FR-023d should be withdrawn rather than left unimplemented. See
research.md § R6 and plan.md's guardrail section.

There is **no new GraphQL surface here**. That is the contract's most important
property: the adoption record is readable by no user, no administrator and no
API. What changes is what two existing mutations do internally.

## What `submitTakedownNotice` does now

Unchanged: validate the statutory elements, write `notice_received`, write
`content_disabled` for the named entity, fire the lore mirror-withdrawal hook.

Added, in the same transaction:

```rust
// src/server/src/moderation/reach.rs
pub fn fan_out_disable(
    conn: &mut PgConnection,
    parent_case_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<Vec<Uuid>, String>;   // the child case ids opened
```

It walks `content_adoptions` from `(entity_type, entity_id)` **transitively** —
a copy of a copy is reachable — and for each copy opens a **child case**:

| Column | Value | Why |
|---|---|---|
| `case_id` | a fresh uuid | The copy's case is its own; see below |
| `parent_case_id` | the notice's case | The link back, for restoration and for an admin reading a case |
| `action_type` | `content_disabled_as_copy` | Disabled, and distinguishable from an accusation |
| `entity_type`, `entity_id` | the **copy** | One entity, never a world and never a collection (FR-023a, FR-023c) |
| `world_id` | the adopter's world | So an admin can see where it landed |
| `account_id` | **NULL** | FR-023b: the adopter is told, not accused. `repeat_infringer_flags_impl` filters on a non-null account, so this can never become their strike |
| the claimant columns | empty, as auto-restoration rows already are | The claimant made no claim about this copy |

Then one `account_notices` row per affected adopter, kind
`adopted_copy_disabled`, and one to the person who shared, kind
`share_taken_down` (FR-024).

**Why a child case rather than more rows in the notice's case**:
`effective_status` takes the latest event per *entity* and
`repeat_infringer_flags_impl` takes the latest event per *case*. Copy rows in
the parent's case would make the second function's answer depend on which copy
was read last, corrupting the counting FR-027 requires be reused unchanged.

## What `submitCounterNotice` does now

Unchanged: validate, write `counter_notice_received`, write
`counter_notice_forwarded` carrying
`restoration_due_at = now + counter_notice_waiting_period_days()`.

Added:

```rust
pub fn fan_out_forward(
    conn: &mut PgConnection,
    parent_case_id: Uuid,
    restoration_due_at: DateTime<Utc>,
) -> Result<usize, String>;
```

which writes the same `counter_notice_forwarded` row, with the **same**
`restoration_due_at`, into every child case of the parent.

**That is the whole of FR-023d.** `effective_status` already restores lazily
when an entity's latest event is `counter_notice_forwarded` and the due date has
passed — so each copy comes back on its own next read, through the mechanism
that already exists, without an adopter asking and without new restoration code.
An `adopted_copy_restored` notice is written when the restoration materialises.

`resolveModerationCase` fans out the same way: a staff `content_restored` or
`content_remains_disabled` on a parent applies to its children, or the source
comes back while its copies stay dark and nobody notices until an adopter
complains.

## What a disabled copy looks like

Exactly what any disabled entity looks like, because the enforcement primitive
does not distinguish and must not learn to:

- omitted from list queries by `filter_visible`;
- returned as a placeholder from single-entity queries, for every caller
  including the adopter — `GraphQLItem::moderated_placeholder`,
  `queries/lore.rs`'s `"[Content removed in response to a takedown notice]"`;
- still a row in the adopter's world, still in whatever collection they put it
  in, restored intact.

**The adopter's own work is untouched** (FR-023c). One entity id is disabled.
Their edits to neighbouring records, their additions, the world it sits in and
the collection it belongs to are not moderation's business and are not touched.

## Two existing gaps this contract meets

1. **`lore_sync` filters on the wrong string.** `lore_sync/plan.rs:109` and
   `lore_sync/incoming.rs:242` call `filter_visible(state, "lore_entry", …)`
   while every moderation row carries `"world_lore_entry"`. The strings never
   match, so the Git-mirror sync paths do not filter taken-down lore at all.
   FR-022 is precisely the requirement that violates. One-line fix, carried here
   with a test that fails before it.
2. **Scenes are not a moderated entity type.**
   `collections::moderation_entity_type` returns `None` for `"scene"`,
   deliberately and with a comment. A scene in a shared collection is never
   withheld and never blocked from copying, so this contract's reach has a hole
   shaped like a scene. Recorded, not closed: it belongs to spec 015 or to spec
   038, whose FR-026c names it as the gap audio must not repeat.

## What is deliberately absent

- **No query over `content_adoptions`.** Not for a user, not for an
  administrator, not "just for debugging". No "what came from this", no "what
  did this world take". The only entry is a bounded walk from an entity id a
  notice already named. A test asserts the SDL exposes no field of this shape,
  because that assertion is the thing keeping ADR-069's determination true.
- **No notification to the adopter naming the claimant or the source world.**
  They are not a party to the notice, and telling them who to be annoyed at
  invites exactly that.
- **No deletion of an adopted copy, ever, for any reason.** FR-023 fixes the
  rule so it is never decided case by case.
