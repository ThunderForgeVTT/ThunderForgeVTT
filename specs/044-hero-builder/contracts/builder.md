# Contract: The Hero Builder

**Spec**: [../spec.md](../spec.md) | **Plan**: [../plan.md](../plan.md) | **Data model**: [../data-model.md](../data-model.md)

Four seams, in the order a phase meets them: what `packages/heroes` gains (§1),
what the builder library exports (§2), what the web app does with it (§3), and
what the server learns (§4). The rules a task is measured against are B1–B9 (with B5a) in
§5. Nothing here changes `storage/svg.rs`, the WebP path, the asset routes or
the engine.

## 1. `packages/heroes` — six additions in phase (a), one in phase (d)

Every addition is a pure function or a data export with a `node --test` test in
the package, and all of them are reached by `pnpm verify`'s existing `heroes`
step (`scripts/verify.mjs:144-147`). The package keeps zero runtime dependencies
and gains no framework.

```ts
// labels.ts (a) — FR-002, US2 scenario 3
export const HERO_LABELS: {
  fields: Record<string, string>;   // every key of HERO_PARTS, HERO_COLORS,
                                    // HERO_FLAGS, plus name, title, size, race
  choices: Record<string, Record<string, string>>; // field → choice → label
  races: Record<RaceKey, string>;   // "half-orc" → "Half-orc"
};

// palettes.ts (a) — FR-008
export const HERO_PALETTES: Record<(typeof HERO_COLORS)[number], readonly string[]>;
// skin → SKIN_TONES' values; hideColor → MONSTER_TONES' values; a curated
// list for the other ten. Every entry is `#rrggbb`, so HEX_COLOR accepts it.

// random.ts (a) — FR-007, FR-007a
export function randomHero(
  seed: string,
  options?: {
    locked?: Partial<Record<keyof ResolvedHero, true>>;
    race?: RaceKey;            // absent = "any"
  },
): HeroSpec;
// Built on the existing `seeded(seed)` chooser (seed.ts:56). Chooses only from
// the closed lists and HERO_PALETTES, narrowed by the race's look where one is
// given. A locked field is absent from the result, so the caller's own value
// survives a merge — a lock beats a race. The race is part of the seed's
// input, so (seed, race) reproduces the hero. No model, no service, no
// Math.random.

// races.ts (a) — FR-007a, research R6
export type RaceKey = string;            // a key of HERO_RACES
export const HERO_RACES: Record<RaceKey, {
  aliases: readonly string[];            // lower-case; unique across races
  choices?: Partial<Record<keyof typeof HERO_PARTS | "size", readonly string[]>>;
  swatches?: Partial<Record<(typeof HERO_COLORS)[number], readonly string[]>>;
  flags?: Partial<Record<(typeof HERO_FLAGS)[number], boolean>>;
}>;
export function matchRace(text: string | null | undefined): RaceKey | null;
// Trims, lower-cases, and compares against each key and alias. "High Elf" →
// "elf"; "Moonkin" → null. Labels for races live in HERO_LABELS.races.

// minimal.ts (a) — FR-009
export function minimalSpec(spec: HeroSpec): HeroSpec;
// Drops a field exactly when removing it leaves resolveHero() unchanged, so
// derived colours (trim, ring, beardColor, accent, hideColor, wingColor —
// spec.ts:319-335) keep following. `name` is always kept.

// presetSource.ts (a) — FR-014
export function presetSource(slug: string, spec: HeroSpec): string;
// A `HeroPreset` entry as TypeScript source, ready to paste into presets.ts.
// Writes `SKIN_TONES.peach` rather than "#f1c6a0" where the value matches a
// named tone, so the pasted entry reads like its neighbours.

// schema.ts (d) — FR-037
export const HERO_SPEC_SCHEMA: object;   // JSON Schema, draft 2020-12
// Derived from HERO_PARTS, HERO_COLORS, HERO_FLAGS, SIZES and the text bounds
// validateHero already applies. A package test fails if the schema and the
// lists disagree.
```

**Unchanged and depended upon**: `createHero`, `renderPortrait`, `renderToken`,
`validateHero`, `resolveHero`, `HeroSpecError`, `seeded`, `PRESET_HEROES`,
`HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS`, `SIZE_CATEGORIES`, `SKIN_TONES`,
`MONSTER_TONES`, and `RenderOptions.idPrefix` with its
`^[A-Za-z][\w-]{0,63}$` rule (`render.ts:62`). No existing export changes shape.

## 2. `@thunderforge/hero-builder` — the library's seam

One package, React 19 as a **peer** dependency, no import from `apps/web`, no
routing, no GraphQL, no network.

```tsx
export interface HeroBuilderProps {
  /** The spec the builder opens on. Run through validateHero before drawing;
   *  a failure is reported through `onInvalid`, not thrown. */
  initialSpec: HeroSpec;
  /** Unique within the host document. Every SVG the builder mounts derives its
   *  id prefix from this (FR-011). */
  idPrefix: string;
  /** Called on every accepted change, with the minimal spec. */
  onChange?(spec: HeroSpec): void;
  /** What the host offers at the bottom of the builder: the standalone app
   *  passes export and copy-as-preset; the web app passes "Save to this
   *  <thing>". The library performs no action of its own. */
  actions?: React.ReactNode;
  /** A spec that would not validate — from import, paste or storage. */
  onInvalid?(problems: readonly { field: string; message: string }[]): void;
  /** Where the race picker starts. The web app passes matchRace() of the
   *  sheet's race; null or absent starts on "any" (FR-007a). The builder
   *  reports no race back — it is a roll setting, not part of the spec. */
  initialRace?: RaceKey | null;
}

export function HeroBuilder(props: HeroBuilderProps): JSX.Element;

/** The two SVG strings for a spec, id-prefixed. The host uploads these; the
 *  library never does. */
export function renderHero(
  spec: HeroSpec,
  idPrefix: string,
): { portrait: string; token: string };

/** The previews alone, for a dialog that is not the full builder (Quick NPC). */
export function HeroPreview(props: {
  spec: HeroSpec;
  idPrefix: string;
  size?: "sm" | "md";
}): JSX.Element;
```

Rules the library holds to:

- **Controls are generated.** One group per key of `HERO_PARTS`, one per entry
  of `HERO_COLORS`, one per entry of `HERO_FLAGS`, plus name, title and size.
  The library names no part, no choice and no colour field in its own source —
  a grep for `"headgear"` or `"wizard"` in `packages/hero-builder/src` finds
  nothing (B1).
- **A choice with no label shows its key** (FR-002); the package's test is what
  fails, not the builder.
- **Set versus following.** A colour the user has not set shows the resolved
  value with a "following" affordance, and can be put back to following
  (FR-005). The builder learns which fields follow by comparing
  `resolveHero(spec)` with `resolveHero(minimalSpec(spec))`, not by a list.
- **No network, no clock, no storage.** The host supplies everything.
- **It exports specs, never drawings** (FR-026). There is no SVG import.
- **The dice sit beside the race picker** (FR-007a), both generated from
  `HERO_RACES` and `HERO_LABELS.races`, with "any" first. Rolling shows the
  seed; the race is not written into the spec, so export, save and "copy as
  preset" are unchanged by it.

## 3. `apps/web` — what the hosts do

### One dialog, one save (phase b, FR-019, FR-024, FR-025)

`HeroBuilderDialog` is the only way `apps/web` shows the builder, and it is a
**full-screen dialog** over whatever page opened it — never a route (FR-019).
It `import()`s the library at open time and nowhere else (FR-022, SC-012),
traps focus, and returns it to the opener. It takes `{ worldId, actorId,
actorLabel, existingRoles, onSaved }`, and reads the race itself:
`useActorSystemData(actorId)` plus the world's manifest `appearance.race.source`
(research R7) → `matchRace` → `initialRace`.

Every host — the panel, a compendium row, Quick NPC's "Open in builder" — uses
one save helper, `saveBuiltHero(actorId, spec)` in
`apps/web/src/pages/world/actor/saveBuiltHero.ts`, so the rules below are
written once.

**A "Build look" control** (label fixed by the clarify session) is placed in
two hosts:

| Host | Test id | Shown when | After a save |
|---|---|---|---|
| `ActorImageryPanel` (NPC editor `:224`, `ActorDetailPage` `:489-494`) | `actor-imagery-build` | `canEdit`, the condition on the file inputs (`ActorImageryPanel.tsx:182`) | the panel shows both roles |
| Each row of `NpcCompendiumTab` (FR-028a) | `npc-catalog-build-${id}` | `npc.myPermissionLevel !== "VIEWER"`, the condition on the row's portrait upload (`NpcCompendiumTab.tsx:296`) | `imagesByActor[id]` updates from the two replies, as `handlePortrait` does (`:177-200`); the GM stays on the list |

The row's existing portrait upload is unchanged. The reserved comment in
`ActorDetailPage.tsx:~496-501` is removed when the control lands, since the
panel now carries it.

Saving:

1. If either role already has an image, say so **before** the upload starts, and
   name both roles (FR-025).
2. `uploadActorImage(actorId, "portrait", file)` then
   `uploadActorImage(actorId, "token", file)`, each file a `File` built from the
   SVG string with type `image/svg+xml`. Two ordinary uploads, not a
   transaction (research R3).
3. Report per role. A role that failed is retryable alone, and no role reports
   success unless the mutation returned its row (FR-025, B4).
4. **A paused world** (`refuse_content_if_paused`,
   `mutations_actor_images.rs:122-124`) refuses both roles. The dialog shows the
   server's message, stays open with the hero intact, and offers export; nothing
   is reported saved (spec Edge Cases).

### Quick NPC (phase b, FR-027, FR-028)

`data-testid="quick-npc"` beside the compendium's "New NPC" button
(`data-testid="new-npc-link"`, `NpcCompendiumTab.tsx:397-407`), on the same
`isGm` condition. The dialog
holds, in this order: a name field, the previews of a random hero, a reroll, a
space reserved for a later template choice, "Open in builder", and Create.
Create is `createActor({ worldId, label, isNpc: true })` followed by the same two
uploads. **The NPC is never deleted to hide a failed upload** (FR-028): it stays,
listed as lacking art, and its row's "Build look" is the way to add it. Quick
NPC's reroll rolls with race "any": a new NPC has no sheet yet.

### Building while creating your own character (phase c, FR-034)

`ActorSelectionPage.tsx:101-124` calls `createAndClaimActor` first and uploads
second. A failure leaves the character with no art and says where to add it.

## 4. GraphQL

Three changes, all additive. The SDL is regenerated with
`node scripts/check-graphql-contract.mjs --schema --fix`, and `pnpm verify`'s
`graphql-schema` and `graphql-ops` steps are the gate.

```graphql
# Phase (d). The only new argument on the only image-writing mutation.
# Absent or null clears the role's spec (FR-035).
uploadActorImage(actorId: UUID!, role: String!, file: Upload!, heroSpec: JSON): GraphQLActorImage!

# Phase (d). The spec that drew this image, or null.
type GraphQLActorImage { id: UUID!, actorId: UUID!, role: String!, assetId: UUID!, url: String!, thumbnailUrl: String!, heroSpec: JSON }

# Phase (b). No new GraphQL: the race is read client-side from the sheet the
# app already fetches (useActorSystemData) and the manifest it already loads.
# The manifest gains an optional block, validated by crates/pack_system_spec:
#   "appearance": { "race": { "source": { "slot": "traitData", "field": "race" } } }
# A source naming an undeclared slot or field is refused at pack load, exactly
# as combat.sizes.source is (combat.rs:215-220).

# Phase (c). Game Master of the world only. FR-030a.
updateWorldAllowPlayerActorArt(input: UpdateWorldAllowPlayerActorArtInput!): GraphQLWorld!

# Phase (c). Game Master of the world only, any actor in it. FR-030b.
setActorArtLocked(actorId: UUID!, locked: Boolean!): GraphQLWorldActor!

# Phase (c). Read-only fields, so a client can show the right thing.
type GraphQLWorld { …, allowPlayerActorArt: Boolean! }
type GraphQLWorldActor { …, artLocked: Boolean!, myMayChangeImagery: Boolean! }
# myMayChangeImagery is B6 evaluated for the caller. The panel's canEdit today
# is `myPermissionLevel !== "VIEWER"` (ActorDetailPage.tsx:144), which a holder
# with no grant row fails, and edit mode redirects a Viewer to /view (:140-142).
# Phase (c) therefore mounts the panel in view mode too, driven by this field.
```

`removeActorImage` is unchanged: removing the row removes the spec with it.

## 5. Rules the server enforces

| # | Rule | Where | Phase |
|---|---|---|---|
| **B1** | The builder's own source names no part, choice or colour field; every control is generated from the catalogue. | `packages/hero-builder/src`, checked by a test that greps the built source for a sample of choice keys | a |
| **B2** | Every SVG in the document carries an id prefix unique to that document; no two elements share an id. | the library; asserted by the phase (a) e2e after every preset (SC-005) | a |
| **B3** | Nothing is drawn from a spec that has not passed `validateHero`. An invalid import is refused whole, by field, and changes nothing on screen. | the library (FR-004, FR-010) | a |
| **B4** | A built hero becomes stored pixels only through `uploadActorImage`. No new upload path, no browser rasterisation, no second mutation. | `apps/web`; the phase (b) e2e asserts the served bytes are WebP, as the P8 e2e does (`world-compendium.spec.ts:268`) | b |
| **B5** | Imagery writes still require Editor **or** the holder's grant, and still refuse while the world is paused. | `mutations_actor_images.rs:113-124`, extended in phase (c) | b, c |
| **B5a** | A race narrows a roll and nothing else. It is never stored, never sent to the server and never written to the sheet; the saved spec is the same whether it was rolled with a race or not. | the library and `packages/heroes` (FR-007a); a package test asserts every roll for a race satisfies its look | a |
| **B6** | The holder's grant applies when, and only when: the caller holds a live claim on the actor (`world_actor_claims`) **or** created it and still holds it; the actor's world has `allow_player_actor_art = true`; and the actor has `art_locked = false`. It covers `uploadActorImage` and `removeActorImage` and nothing else — not the label, the description, the sheet, the abilities or the inventory. A Game Master is unaffected by either switch (FR-032). | new `auth/actor_imagery.rs`, called by both image mutations | c |
| **B7** | A stored spec must be a JSON object, at most 4 KB, and must pass `HERO_SPEC_SCHEMA`. A refusal names what was wrong and stores nothing — neither the spec nor the image. | new `heroes/spec_schema.rs`, before the upsert | d |
| **B8** | An image written with no `heroSpec` has no spec. Replacing a built image with a file clears that role's spec in the same write. | `mutations_actor_images.rs` upsert (`:162-174`) | d |
| **B9** | A collection copy carries each image's spec; a personal export carries it; a shared-actor copy carries no images and so carries no specs. | `collections/copy.rs:595-616`, `users/export_content.rs:304-314` | d |

Refusal messages are distinguishable, because SC-010 and FR-030b ask a player to
be told *which* rule stopped them:

- no claim → "Only the player who holds this character may change its art."
- world setting off → "The Game Master has turned off players changing their
  character's art in this world."
- `art_locked` → "The Game Master has locked this character's look."

## 6. What does not change

- `storage/svg.rs` — the 1024 px raster, the refusal of external references, the
  absence of a text renderer, the ignored declared size. A built SVG is an SVG.
- `transcode.rs` — the 25 MB ceiling and the WebP output. A hero SVG is a few
  kilobytes.
- `assets_serve/actor.rs` — a hidden NPC's portrait stays as hidden as its name
  (`:155-158`); its token art still follows the map (`:161-165`). Built art
  inherits both.
- `token_art.rs` — a token with no `photo_url` still wears its actor's token
  image, so every copy of a built NPC gets the face for free.
- The engine. The builder never speaks to it (FR-029).
