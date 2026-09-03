# Phase 1 Data Model: Interface Packs

Three entities, one of which already exists in the database and one of which
exists only in memory.

---

## Interface Pack

A directory under `packs/interface/` containing `interface.json`. Not a
database row in this increment (research.md §3).

| Field | Type | Notes |
|---|---|---|
| `id` | string | `[a-z0-9-]+`; equals the directory name |
| `type` | literal `"interface"` | FR-002's exclusivity, enforced structurally |
| `title`, `description` | string | What a Game Master reads when choosing |
| `version` | semver | |
| `compatibility` | `{minimum, verified, maximum}` | Same shape as `system.json` |
| `legal` | `SystemManifestLegal` | Reused verbatim, not re-declared |
| `light`, `dark` | token map | Partial; absent keys inherit Forge's |
| `canvas` | `AppearanceOverride?` | Absent means engine defaults |
| `targets` | `[system id]` | Required. `[]` means "any system", and is only permissible with generic-only layout |
| `layout` | `LayoutDeclaration?` | Absent inherits Forge's, which is generic |

**Identity**: `id`. Two packs claiming the same id is a startup-time conflict,
not a runtime one, because discovery is a directory listing — the filesystem
will not hold two directories of the same name, so the spec's "two packs claim
the same identity" edge case cannot arise for interface packs at all. It stays
a real problem for system packs, which can be uploaded, and it stays US2's.

**Lifecycle**: read at request time, not cached at boot. A pack added to the
directory is available on the next request; the product is not a long-running
cache of its own filesystem. If this proves hot, it caches — but a per-world
setting read on world load is not a hot path.

---

## Base Interface Pack — Forge

Not a distinct entity. Forge is `packs/interface/forge/`, discovered by the
same listing, validated by the same rules, served by the same route, and
selectable by the same mutation as any other pack (FR-007).

Two properties make the peer requirement true rather than merely stated:

1. **It is not compiled in.** If Forge were bundled into the web build and the
   others fetched, Forge would have a capability the others lack — existing
   before the network does. It is a directory like the rest.
2. **The fallback is by id, not by privilege.** When a world's pack is missing,
   the resolver falls back to the pack whose id is `forge`. That is a named
   default, and it is the one thing about Forge that is special: something has
   to be the floor. It is written in one place so that "which pack is the base"
   is a one-line answer rather than a property distributed through the code.

**The rule that keeps this honest**: if a change would give Forge a token, a
placement, or an exemption another pack cannot have, FR-007 is what it violates.
The name — chosen 2026-09-02, overriding the draft's "Mithral" — carries none
of the guarantee.

---

## Declared Value

One thing a system publishes about an actor. The unit everything downstream
carries and nothing downstream interprets.

| Field | Type | Notes |
|---|---|---|
| `id` | string | The system's own identifier — `strength`, `strengthMod`, `wishPoints` |
| `value` | integer / number / text / boolean / list | |
| `origin` | `stored` \| `derived` | Which half of the resolution produced it |

**Stored** values come from the actor's existing JSONB slots, read against the
system manifest's declarations — a path that already exists.

**Derived** values come from the system pack's own `derive`, and are **never
written to the database**. A derived value that is also stored is two values
that can disagree, and the stored one is the one that goes stale. They are
recomputed on every read, which is affordable precisely because `derive` is
pure and does no I/O.

`origin` exists so a surface can tell a player which numbers they may edit. A
5e Strength score is typed in; its modifier is not, and a sheet that offered a
text box for it would be inviting the two to disagree.

## System Contract

The single trait every system pack implements to supply its declared values
(FR-027). Lives in `thunderforge-canvas-core`, the only crate both the server
and the engine already depend on. See
[contracts/system-contract.md](./contracts/system-contract.md).

Replaces two divergent declarations that exist today and are depended upon by
nothing, one of which returns a fixed `DerivedStats` of armour class,
initiative and proficiency bonus — a shape with nowhere to put Blades in the
Dark's stress and trauma, and nothing to say to Fate Core.

## Layout Declaration

A tree of constructs, each addressing the system's declarations **generically**
(by kind and declaration order) or **specifically** (by identifier).

| | Addresses | Composes with | Validated against |
|---|---|---|---|
| Generic | a declaration set | any system, including ones that ship later | nothing to check — it names nothing |
| Specific | a named identifier | the systems in `targets` | each target's manifest, independently |

Forge is generic-only, which is what makes it simultaneously the universal
fallback (FR-006, FR-025b) and the format's conformance reference (FR-007a). A
targeted pack mixes both.

**Not part of a layout**: expressions, conditionals, thresholds, colour ramps
keyed to values, and any label the system did not declare. Each of those is a
claim about what a number *means*, which is the system's to make.

## World Appearance Binding

`worlds.interface_pack_id`, `Nullable<Varchar>`. **Already exists.** Added in
`2026-05-04-234500-0003_phase3_world_metadata`, normalized and validated on
write by `graphql/helpers.rs`, surfaced as `GraphQLWorld.interfacePackId`,
present in `apps/web/src/types/world.ts`, and read by nothing but two labels.

No migration.

| State | Meaning | What a reader sees |
|---|---|---|
| `NULL` | No choice made | Forge. Settings names it as the active choice (FR-023, US1 scenario 3) |
| `"<id>"`, pack present | Chosen | That pack |
| `"<id>"`, pack absent | Degraded | Forge, plus one notice naming the missing pack (FR-018) |

**Authority**: the world's Game Master or Owner, checked server-side by
`is_dm_of_world` (FR-010, Principle III). Not a per-user preference — decided
2026-09-02, overriding the draft's FR-009.

**Scope of what it changes**: presentation only. Switching it must not change
which actions are available, which permissions apply, or which values are
displayed (FR-011). The mechanism makes this hard to violate — a custom
property cannot reach a permission — but the e2e in quickstart.md asserts it
anyway, because "hard to violate" and "did not happen" are different claims.

---

## Resolved Appearance *(in memory, client-side)*

What the provider actually applies. Not persisted anywhere.

```
ResolvedAppearance {
  packId:   string            // the pack in force, "forge" when falling back
  missing:  string | null     // the id that was asked for and not found
  light:    TokenMap          // Forge's tokens, overlaid with the pack's
  dark:     TokenMap
  canvas:   AppearanceOverride
  layout:   LayoutDeclaration // the pack's, or Forge's generic one
}
```

Built by: take Forge's manifest as the base, overlay the chosen pack's declared
keys, keep the reader's own light/dark selection to decide which map is applied
(research.md §5). Applied by writing custom properties onto
`document.documentElement`; the canvas half goes to the engine as one
`set_display_appearance` command.

**Why the overlay rather than requiring a complete pack**: a pack that must
declare all ~30 tokens to declare one is a pack that silently pins twenty-nine
values it never chose and never updates — the same reasoning that made
`AppearanceOverride` partial, recorded in its own doc comment.

**Degraded state** is `missing !== null`. It is the whole of what User Story 3
needs on the interface side: fall back, say so once, block nothing (FR-018).
There is no "world will not open" case here — a look cannot fail in a way that
costs content, which is the asymmetry between the two pack types stated in
data form.

---

# Increment F (User Story 2) — 2026-09-03

## Installed system (read model)

Not a table. The row of record is the directory `packs/systems/<id>/`, and
`system.json` is the row. `/api/systems` projects it, mirroring
`interface_packs::list_installed`:

| Field | Source | Notes |
|---|---|---|
| `id` | `system.json` `id` | Matches the directory name; a pack test already asserts the two agree |
| `title` | `system.json` `title` | Replaces `BUNDLED_SYSTEM_LABELS` |
| `version` | `system.json` `version` | |
| `description` | `system.json` `description` | |
| `legal` | `system.json` `legal` | Already served by `/api/systems/:id/manifest.json` |

**`game_systems` (existing table)**: 0 rows, read by `/api/systems` today, and
the reason the client hardcodes a list. Its fate is ADR-028's to record — see
research F-1. Nothing in this increment writes to it.

## World-creation hook (contract, shape open)

The behaviour a pack contributes when a world on its system is created.
`genie` is the only current implementer: a `world_genie_sessions` row with
`doom_clock_max: 6`.

Shape depends on research F-2's unresolved decision, so it is recorded as a
contract rather than a signature:

- **Input**: the world that was created (`world_id`, `created_by`), inside the
  same transaction as the world and its default scene — a hook that commits
  separately can leave a world without the row its system expects.
- **Output**: success, or a failure that aborts the whole world creation. A
  half-created world is worse than a refused one.
- **Registration**: `inventory`, alongside `SystemContribution`, so discovery
  stays "what is linked" and no list names a system.

## Surface failure boundary (client state)

Per mounted pack surface, not per page:

| Field | Meaning |
|---|---|
| `packId` | Which pack this boundary wraps — the name SC-009 requires in the message |
| `surface` | Which surface failed, so the message can say *what* is unavailable |
| `error` | Captured for diagnostics, never rendered raw to a player |

The state transition is one-way within a mount: **rendering → failed**. It
resets when the boundary remounts (a different actor, a different world),
because a failure caused by one actor's data should not condemn the next.

