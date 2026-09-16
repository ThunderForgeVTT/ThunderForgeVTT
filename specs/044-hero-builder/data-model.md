# Data Model: The Hero Builder

**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

Postgres through Diesel, one migration directory per change with paired
`up.sql`/`down.sql` (constitution). Phase letters are the plan's.

**Phases (a) and (b) change nothing here.** A hero built for an NPC is an SVG
handed to `uploadActorImage`, and the server stores the WebP it already stored
for any upload. The first schema change is phase (c)'s two switches; the second
is phase (d)'s stored spec.

## Changed: `worlds` (phase c)

| Column | Type | Rule |
|---|---|---|
| `allow_player_actor_art` | `BOOLEAN NOT NULL DEFAULT true` | FR-030a. Only a Game Master of the world may set it. While false, the holder's grant (below) applies to no actor in the world; a Game Master's own authority is untouched. |

**The default is the opposite of its neighbours, on purpose.** `worlds` carries
`allow_player_created_actors`, `genie_resource_carryover_enabled` and
`auto_apply_npc_damage`, all `DEFAULT false` (`schema.rs:1593-1597`). FR-030a
says this one defaults **on**: the grant is the feature, and the setting is the
way to withdraw it. The migration therefore backfills `true` for every existing
world, and the hand-kept defaults in `mutations_worlds.rs:141-145` and the
adapters must say `true` as well, or a newly created world would disagree with
every old one.

The setter follows the established shape exactly — the newest instance is
`update_world_auto_apply_npc_damage_impl` (`mutations_attacks.rs:106-130`):
`is_dm_of_world`, then `refuse_if_paused` inside the blocking closure, then one
`diesel::update` returning `GraphQLWorld`.

## Changed: `world_actors` (phase c)

| Column | Type | Rule |
|---|---|---|
| `art_locked` | `BOOLEAN NOT NULL DEFAULT false` | FR-030b. Settable by a Game Master on any actor in the world. While true, the holder's grant does not apply to **this** actor, whatever the world setting says. Locking changes no image. A player refused by a lock is told the Game Master has locked the character's look — a distinct message from the world setting's refusal. |

It sits beside `is_unique` and `visible_to_players` (`schema.rs:1086-1087`),
which are the two most recent GM-only per-actor switches and the template for
this one, including the GraphQL field on `GraphQLWorldActor`
(`types_scene.rs:246`) and the control's place on `ActorDetailPage`.

`art_locked` is **not** carried by a collection copy: a lock is this table's
Game Master telling this table's player something, and it means nothing in the
world the copy lands in. The copy arrives unlocked, and its new Game Master
locks it if they want to. This is the opposite of `is_unique`, which describes
the creature rather than the table, and it is stated here because
`collections/copy.rs:495-589` enumerates columns by name and every new one must
be a decision.

## Changed: `world_actor_images` (phase d)

| Column | Type | Rule |
|---|---|---|
| `hero_spec` | `JSONB NULL` | R4, FR-035. "This image was drawn from this spec." Written by the same upsert that stores the image, through a new optional `heroSpec` argument on `uploadActorImage`. **An upload with no argument writes `NULL`**, so replacing a built image with a file clears that role's spec in the same write (US6 scenario 2). |

Constraints and limits, all enforced in Rust before the write (FR-037), because
a JSONB column will otherwise take anything:

- it must be a JSON **object**, not an array, a string or a number;
- it must be at most **4 KB** serialised — the twelve presets are well under
  1 KB (`packages/heroes/src/presets.ts`), and the cap exists to stop the column
  becoming a document store, not to be tight;
- it must pass `HERO_SPEC_SCHEMA`, the JSON Schema phase (d) adds to
  `packages/heroes` and a package test keeps in agreement with `HERO_PARTS`,
  `HERO_COLORS` and `HERO_FLAGS` (contract B7).

The server never checks that the spec matches the image beside it, and cannot.
A mismatch needs a hand-made request from somebody who could already upload any
image, and it misleads only the builder's starting point (spec, Security table).

No table is added. The row already exists, is unique on `(actor_id, role)`
(`migrations/2026-09-01-120000-0000_create_world_actor_images/up.sql:32`), and
is deleted with its actor by `ON DELETE CASCADE`. Authorisation for writing the
spec is authorisation for writing the image, which is the whole of R4's fourth
reason.

## Who carries `hero_spec`, and who must be told to (phase d)

`world_actor_images` is read or copied in five places. Three of them enumerate
columns by name, so the new column travels only where a task puts it.

| Site | Today | Phase (d) |
|---|---|---|
| `collections/copy.rs:597-616` | selects `(role, asset_id)`, inserts those plus a new `actor_id` | **must select and insert `hero_spec`** (FR-039). Research R4's "it travels for free" is not true of this code. |
| `users/export_content.rs:304-314` | builds `ExportedImage { role, asset_id }` | **must carry `hero_spec`** (FR-040). |
| `graphql/mutations_actor_images.rs:185-204` | `actor_images_impl`, ordered by role | returns the spec; `GraphQLActorImage` (`types_actors.rs:78-98`) gains `heroSpec: JSON`. |
| `graphql/token_art.rs:41-58` | resolves a token's art from its actor | unchanged. Art is not per-token; a copy wears its actor's face. |
| `graphql/mutations_actor_shares.rs:390-420` | copies an actor with **no** imagery rows at all | unchanged, and nothing to do. A shared copy has no images, so it has no specs. |

## Not stored, and why

- **A hero spec in phases (a), (b) and (c).** The server sees an SVG and stores
  WebP. Phase (d) is the first time a spec reaches the database, and it is the
  only thing in this feature the server has to distrust in a new way.
- **The catalogue.** Parts, choices, labels, palettes and flags are code in
  `packages/heroes`, reviewed and shipped with the client. Nothing in the
  database names a part.
- **A randomiser's seed.** It is shown in the builder and travels in nothing.
  Re-entering it reproduces the hero (FR-007); storing it would be a second,
  weaker description of a spec that is already stored whole.
- **A built hero that belongs to no actor.** Out of scope (spec, Out of Scope):
  a hero travels by travelling with the actor that wears it, until the
  profile-content spec gives a hero somewhere else to live.

## Key entities, as the code sees them

| Entity | Where it lives | Trust |
|---|---|---|
| **Hero spec** | `HeroSpec` (`packages/heroes/src/spec.ts:220`) — a name plus whatever differs from the defaults | Untrusted whenever it comes from outside the code; `validateHero` is the client gate, `HERO_SPEC_SCHEMA` and the size cap the server's |
| **Resolved hero** | `ResolvedHero` (`spec.ts:178-217`), 28 fields, produced only by `validateHero`/`resolveHero` | Trusted, because only the validator makes one |
| **Catalogue** | `HERO_PARTS` (12), `HERO_COLORS` (12), `HERO_FLAGS` (2), `SIZE_CATEGORIES` (6), `SKIN_TONES` (8), `MONSTER_TONES` (12), plus phase (a)'s labels and palettes | Trusted; it is our code |
| **Rendered hero** | two 256×256 SVG strings from `createHero(spec)` (`render.ts:41-49`) | Trusted as markup; the id prefix is validated against `^[A-Za-z][\w-]{0,63}$` (`render.ts:62`) |
| **Actor image** | a `world_actor_images` row, one per `(actor_id, role)`; from phase (d) it may carry the spec that drew it | The image is WebP the server made; the spec is untrusted input stored beside it |
