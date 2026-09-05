# Phase 0 Research: Content Collections

**Feature**: `026-content-collections` · **Date**: 2026-09-04

Every finding below was read out of the codebase rather than assumed. Where a
finding contradicts the spec, the spec was corrected and the correction is
noted here — a research document that quietly agrees with a wrong premise is
worse than none.

---

## 1. What the existing share machinery actually provides

**Decision**: Generalise the shipped share shape rather than invent a second
one — but generalise it honestly, because it is a smaller inheritance than the
spec first assumed.

**What exists**, in three near-identical modules
(`mutations_ability_shares.rs`, `mutations_item_shares.rs`,
`mutations_actor_shares.rs`, 1,808 lines between them):

| Piece | Where | Reusable as-is? |
|---|---|---|
| Unguessable code generation | `graphql/share_codes.rs` — `generate_link_code()`, v4-derived, 20 uppercase hex (~80 bits) | **Yes, directly.** FR-008 is satisfied by calling it. |
| Revocation as a soft flag | `revoked BOOLEAN` on each `world_*_shares` table | **Yes, as a pattern.** The comment explains why a delete cannot serve: a deleted row cannot distinguish "revoked" from "never existed", which is exactly FR-010's requirement. |
| Moderation gate on the read path | `crate::moderation::effective_status(state, "<entity_type>", id)` | **Yes, directly.** |
| Transactional deep copy | `copy_shared_*_to_world_impl`, each wrapping `conn.transaction::<_, CopyError, _>` | **As a pattern.** FR-013 is what this shape already delivers. |
| The `CopyError` orphan-rule workaround | a local newtype over `String` with `From<diesel::result::Error>` | **Yes.** Needed again for the same reason. |
| No-enumeration guarantee | structural — no index or query shape exists to list shares by world or user | **Yes, as a discipline.** FR-020 is met by *not* adding one. |

**Rationale**: Three implementations of one idea already exist and diverge in
small ways (the ability copy re-validates effect formulas and preserves
`gm_only`; the item copy does neither). A fourth written from scratch would
make four. Collections should reuse the code generator and copy the transaction
shape, and should not attempt to refactor the existing three — that is spec
027's kind of work, not this one's.

**Alternatives considered**: A generic `share<T>` abstraction covering all four
units. Rejected for the reason ADR-050 already records about these tables:
Diesel gives every table its own generated type, and the trait bounds needed to
write one function over "any content table" exceed the bodies they replace. The
macro approach in `auth/permissioned_entities.rs` is the local precedent for
when that is worth doing — and it was worth doing there because a *missing*
fourth block caused a live privilege leak. There is no equivalent forcing
function here.

---

## 2. Shares are authenticated today — the spec said otherwise

**Finding**: FR-009a was clarified on the stated grounds that anonymous viewing
"matches spec 025's existing share behaviour". **It does not.** All three
share queries call `authenticated_user(ctx)?`:

```rust
// mutations_ability_shares.rs
async fn shared_ability(&self, ctx: &Context<'_>, share_code: String) -> ... {
    let state = app_state(ctx)?;
    // Authenticated, but deliberately no membership check.
    let _ = authenticated_user(ctx)?;
```

The comment is precise about what it skips: **membership**, not
authentication. A share link today reaches any signed-in user; it does not
reach the public.

**Decision**: Keep the decision, correct the reasoning, and budget the work.
Anonymous viewing stands on its own merits — sharing with someone who has not
joined is most of the point, and unguessability plus revocability is what
protects the content. But it is **net-new work**, not a reuse, and the plan
budgets an unauthenticated GraphQL read path accordingly. The spec now records
this correction in FR-009a rather than carrying a false premise forward.

**Consequence**: `FR-009e` was added declaring alignment of the three existing
singleton shares **out of scope**. The argument for anonymity applies to them
too, so the product will be briefly inconsistent — but relaxing authentication
on three shipped share paths is a security change to features this spec does
not otherwise touch, and it should be decided deliberately rather than inherited.

---

## 3. The existing rate limiter cannot serve FR-009c

**Finding**: `auth_middleware::rate_limit_auth_requests` keys on the request
path and returns early unless it contains `/authentication/`:

```rust
let path = request.uri().path().to_string();
if !path.contains("/authentication/") { return next.run(request).await; }
```

Every GraphQL operation in this product arrives at one path. A per-path limiter
therefore cannot distinguish "someone is walking collection codes" from any
other query, and configuring it to cover `/graphql` would rate-limit the entire
application.

**Decision**: A **separate limiter keyed on the anonymous collection read**,
applied inside the resolver rather than in path middleware — same
`OnceLock<Mutex<HashMap<..>>>` sliding-window shape as the existing one, keyed
on `client_ip`, with a window tuned for a human opening a link rather than for
password attempts.

**Rationale**: Reusing the shape keeps one idea of what rate limiting looks
like in this codebase; reusing the *middleware* would mean either limiting all
of GraphQL or inspecting operation names in middleware, which puts GraphQL
knowledge in a layer that has none.

**Note on the debug bypass**: the existing limiter is compiled out entirely in
release when bypassed, with a long comment explaining that credential stuffing
is what it guards. The collection limiter guards code-walking rather than
credentials, and **must not** honour `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` —
that variable is set by the e2e harness on every run, and a limiter that
switches off during the tests written to prove it is a limiter nobody tests.
The e2e for FR-009c must therefore assert against a limiter that is actually on.

**Alternatives considered**: Rely on unguessability alone (rejected — FR-009c is
explicit, and ~80 bits is unguessable only while guessing is bounded); a
token-bucket crate (rejected — one more dependency for a shape already written
twenty lines away).

---

## 4. Scene copying does not exist anywhere in this product

**Finding**: `grep -rn "duplicate_scene\|clone_scene\|copy_scene" src/server/src`
returns nothing. Scenes have never been duplicated, within a world or across
worlds.

A scene is not one row. `scenes` carries `background_image_path`,
`background_asset_id` and `preview_asset_id`, and these tables reference it:
`walls`, `light_sources`, `shapes`, `fog_masks`, `interactives`, `tokens`.

**Decision**: Copy the scene row and **`walls`, `light_sources` and `shapes`**;
carry the background by creating a *new asset row pointing at the same
`storage_path`*; **do not** copy `tokens`, `fog_masks` or `interactives`.

**Rationale**:
- SC-008a names exactly background, walls and lighting as what must render in
  the destination. Shapes are drawn scenery by the same argument and cost
  nothing extra.
- **Tokens are placed actors mid-play**, not scenery. Copying them would drag
  actor rows in as a side effect the collection's owner never chose, which
  FR-014/FR-015 explicitly reject in favour of a declared loss.
- **Fog is per-session play state** — what a table has explored. It is not part
  of the place.
- `interactives` reference behaviour wired to a specific world's content.

Anything not copied is a **fidelity note** (FR-015), not a silent omission.

**On the background asset**: `storage/dedupe.rs` is explicit that each asset
keeps its own row with its own `asset_id`, `world_id`, `scene_id` and owner,
and only `storage_path` is shared; `canvas_assets_serve` authorises against the
row it looked up. So a new row in the destination world pointing at the same
object is the *designed* use of that module, satisfies FR-018 (the copy does
not depend on the source world existing — it depends on the object, which is
instance-wide), and satisfies FR-019 and SC-008 by construction: zero
additional stored bytes.

**On the deletion dependency**: `dedupe.rs` states plainly that nothing in this
product deletes stored objects, and that this is what makes a shared path safe.
Collections must not be the feature that breaks that. **Nothing in this
delivery deletes a stored object** — revoking a collection flips a flag,
deleting a collection removes rows, and neither touches storage. Reference
counting remains a prerequisite of any future deletion, exactly as the
Assumptions section says, and is not required to ship this.

---

## 5. Restricted visibility is two mechanisms, not one

**Finding**: FR-001a says an artifact "restricted to a subset of its world's
members" cannot be added. That is expressed two different ways in this codebase,
and a check that knows only one is a gate with a hole in it:

1. **Explicit grant rows** — `world_actor_permissions`,
   `world_item_permissions`, `world_lore_permissions`,
   `world_ability_permissions`, declared together in
   `auth/permissioned_entities.rs`. A row grants one member a level above the
   `Viewer` default.
2. **Visibility flags** — `world_abilities.gm_only`, `world_items.gm_only`,
   `scenes.hidden`. `permissioned_entities.rs` warns at length that visibility
   is a *separate axis* from the permission ladder and must never be folded into
   it, because `Viewer` is both the ladder's floor and its default and so cannot
   express "hidden".

**Decision**: The FR-001a check consults **both axes**, per member type, in one
function — `collection_membership::restriction_reason(...) -> Option<String>`
returning the sentence shown to the user.

**Rationale**: A single function is the thing a test can exhaust across all five
member types, which is what SC-003a demands ("verified across every artifact
type rather than sampled"). Two scattered checks are what SC-003a exists to
catch.

**Alternatives considered**: Extending the `permissioned_entities!` macro with a
visibility parameter. **Explicitly rejected by the module's own documentation**,
which says the macro "must never gain a visibility parameter 'for symmetry'".

---

## 6. Restoration after a reversed takedown is already lazy

**Finding**: `moderation::effective_status` performs lazy auto-restoration — a
forwarded counter-notice whose waiting period has elapsed, with no newer event,
restores at read time.

**Decision**: FR-025 needs no work of its own. Calling `effective_status` on
every member at read and at copy time gives restoration for free, because the
collection stores membership and asks about status rather than caching it.

**Consequence for the data model**: a collection member row **must not** carry a
`disabled` column. A cached status is a status that can be stale, and staleness
here means either serving a taken-down artifact or withholding a restored one.

---

## 7. Ownership of copies

**Finding**: `copy_shared_ability_to_world_impl` sets `created_by: user_id` and
`updated_by: user_id` on the copy, and its comment records that "the copy's
ownership block starts empty — the destination DM has implicit full control".

**Decision**: FR-017a is satisfied by the shipped convention — stamp the copier
on `created_by`/`updated_by`, create no grant rows. This is also Principle III's
provenance requirement.

**Rationale**: No new ownership concept is needed. FR-017b (re-sharing) then
follows with no code at all: the copies are ordinary content in the recipient's
world, and creating a collection over them is the feature already being built.

---

## 8. The 100-member limit

**Decision**: Enforce at **add time** in the mutation, and re-assert inside the
copy transaction.

**Rationale**: FR-005a says refused on adding, never accepted-then-failed at
copy. The re-assert inside the transaction is not redundant: it is what makes a
concurrent add unable to push a collection past the limit between check and
copy.

---

## Open, and not resolvable by research

- **FR-027's DMCA determination.** A signature from an accountable owner, before
  implementation begins. ADR-067 is the worked example (spec 034). No planning
  artifact can produce it.
