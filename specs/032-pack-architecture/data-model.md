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
