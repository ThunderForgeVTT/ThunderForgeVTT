# Actor Imagery Is Rows Keyed by Role, Not Columns

- **Date**: 2026-09-01
- **Status**: Accepted
- **Spec**: `specs/031-playability/` (US8, FR-036)

## Context

An actor needs two images, and they are genuinely different things: a
**portrait**, which is the character's face in a sheet or a panel, and a
**token image**, which is what stands on the map. Spec 031 asks for both.

`world_actors` today has **no image columns at all** — id, world_id, scene_id,
actor_type, game_system_id, label, created_by, owned_by, is_public, is_npc,
timestamps, description, available_for_claim. (`world_items`, by contrast,
already carries `icon_asset_id`, an asymmetry worth settling separately.)

So this is net-new modelling, and the obvious shape is two nullable columns:
`portrait_asset_id` and `token_asset_id`. That is the decision this ADR exists
to argue against.

During the same playtest that produced spec 031, a further use was raised and
deliberately deferred: a **talking / not-talking / background** image set, so an
NPC or a player character can be presented in a VTuber style during play. It is
not being built now. But it is *n* images per actor, keyed by what each one is
for — which is the same question this decision answers.

## Decision

**Actor imagery is stored as rows in `world_actor_images`, keyed by role.**

| Column | Purpose |
|---|---|
| `actor_id` | which actor |
| `role` | what this image is *for* |
| `asset_id` | the stored, transcoded image |

Unique on (`actor_id`, `role`) — an actor has at most one image per role.

Spec 031 requires exactly two roles, `portrait` and `token`. The column is open
rather than an enumeration, for the same reason ADR-054 rejected a central enum
of interaction effects: a fixed set means every new role edits the core.

## Rationale (Y-Statement)

In the context of giving actors a portrait and a token image, facing a deferred
requirement for an open-ended set of presentation images, we decided **to model
imagery as rows keyed by role** and neglected **two scalar columns on
`world_actors`**, to achieve **a later image set that is additive rather than a
second mechanism**, accepting **a join and a slightly less obvious read path for
the two roles we need today**.

## Consequences

**The deferred feature becomes additive.** Talking, not-talking and background
are three more roles, not three more columns and a separate table alongside the
columns. This is the entire point: the cheap choice today is the one that
forecloses tomorrow's, and the difference in cost *now* is close to nothing.

**Reads need a join.** Fetching an actor with its portrait is no longer a single
row. In exchange, "give me every image this actor has" is a query rather than a
column-by-column enumeration, which is what the presentation layer will
actually want once there is more than one kind.

**Roles are open, so they must be documented.** An open column can accumulate
typos. The set in use is recorded here and in the spec; a role that no code
recognises should be ignored rather than rendered.

**Storage and conversion are unchanged.** Images continue through the existing
path — the size limit, webp conversion and object-storage write that
`mutations_lore_images.rs` already performs, with the same permission checks.
Nothing about this decision touches how bytes are handled.

**The `world_items.icon_asset_id` asymmetry survives.** Items keep their column
for now. Settling that is a separate, deliberate pass rather than a drive-by
migration inside a playability feature.

## Alternatives Considered

- **Two columns on `world_actors`.** Rejected. It is simpler today and forces
  the deferred set into a parallel mechanism tomorrow, at which point an actor's
  images live in two places and every reader has to know both.
- **A single column holding a JSON blob of images.** Rejected: it defeats
  referential integrity with the asset table and makes "which actors use this
  asset" unanswerable.
- **A closed enum of roles.** Rejected on ADR-054's reasoning — a central list
  that every new role must edit is the coupling that decision exists to avoid.
- **Reuse `world_items.icon_asset_id`'s shape for consistency.** Rejected:
  consistency with a shape we already suspect is wrong is not a reason to repeat
  it.

## Related Decisions

- **ADR-054** — the contribution seam; the argument against central enumerations
  is borrowed directly.
- **ADR-039** — RustFS scoped asset storage; where the bytes live.
- **ADR-036** — extensible system-agnostic actor data; the same instinct
  (open, keyed, additive) applied to a different part of the actor.
