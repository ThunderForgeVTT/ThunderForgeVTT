# Feature Specification: The Hero Builder

**Feature Branch**: `044-hero-builder`

**Created**: 2026-09-10

**Status**: Draft. This is the specification only. It ships in four phases,
(a) to (d), and each one is shippable by itself. Phase (a) needs nothing from
the server.

**Input**: Project owner: "Spec a hero builder … the goal would be a frontend project like engine but specifically designed for us to generate and eventually for users to build heroes or GMs to build NPCs fast."

## Context

### The factory exists; nothing drives it

`packages/heroes` (`@thunderforge/heroes`) landed on 2026-09-10 for playtest
P8. It is a factory. A **hero spec** is a name plus whichever parts and colours
differ from the defaults. `createHero(spec)` returns a hero whose `.portrait()`
is a square card and whose `.token()` is a round token with a coloured rim.
Both are self-contained 256×256 SVG strings.

The package was built with a builder in mind, and it says so:

- **Every choice is exported as data.** `HERO_PARTS` holds six closed lists of
  choices: ears, mouth, hair, headgear, emblem and prop. `HERO_COLORS` lists ten
  colour fields, `HERO_FLAGS` two on/off fields, and `SKIN_TONES` eight starting
  swatches. A builder can render its controls from these instead of keeping its
  own list.
- **No choice is a dead control.** The package test asserts that every choice
  of every part draws a different token.
- **`validateHero(unknown)` is the gate for a spec that came from outside.** It
  accepts colours only as `#rrggbb`, refuses unknown fields and unknown choices
  instead of drawing the default, and bounds the name and title. The renderer
  escapes both as text.
- **Ids inside each SVG are prefixed**, so several heroes can be inlined on one
  page.
- **There are twelve presets**, and a CLI exports them to a directory.

Today the only ways to make a hero are to edit `presets.ts` by hand, or to
write code that calls the factory. `scripts/seed-demo-art.mjs` and one e2e do
the latter.

### The path from a drawing to the map already works

- **The server accepts SVG uploads and stores WebP.** An actor's portrait and
  token are uploaded through the `uploadActorImage(actorId, role, file)`
  mutation, with `role` either `portrait` or `token`.
  `src/server/src/storage/svg.rs` draws an uploaded SVG to pixels at
  1024 px on its longest edge. It follows no reference the file makes, draws no
  text, and ignores the size the file declares. The result then takes the same
  WebP path as every other upload.
- **Imagery is stored as rows keyed by role (ADR-057).** The table is
  `world_actor_images`, unique on (`actor_id`, `role`), so re-uploading a role
  replaces that role's image and nothing else. `world_actors` has no image
  columns.
- **The NPC editor is the only UI for this.** On
  `/world/:id/compendium/npc/:actorId/edit`, `ActorImageryPanel` offers a file
  input per role. Because imagery hangs off an actor id, the NPC must be saved
  before it can have art.
- **The e2e "Hero art (playtest 2026-09-10 P8)"** in `world-compendium.spec.ts`
  proves the path end to end. It uploads a preset's token as SVG, and it is
  served back as WebP.

So everything behind a builder exists. What is missing is the builder itself.

### Three absences, stated precisely

**1. There is no way to look at a hero while changing it.** Tuning a preset
means editing TypeScript, running the CLI and opening the gallery it writes.
That loop is what the owner means by "for us to generate".

**2. A GM who wants an NPC with a face has to be an illustrator, or upload
somebody else's picture.** The imagery panel takes a file. It cannot make one.
"GMs to build NPCs fast" means making the art in the product, in seconds, from
parts that are ours to redistribute.

**3. A player cannot give their own character a face.** The imagery panel is
mounted on the NPC editor only. `ActorDetailPage`, the page a player's
character lives on, has no imagery controls at all. There is a second gap
underneath the first. As read on 2026-09-10, claiming a character or creating
one's own (spec 017) writes no actor-permission row. Actor permission
resolution does not consult claims or `owned_by`, and a member with no grant
resolves to Viewer. The imagery mutation requires Editor. So a player most
likely cannot upload art for their own character today, even by hand.

### What "a frontend project like engine" means here

The engine is one artefact with two consumers. The Bevy crate compiles to a
wasm package (`@thunderforge/engine`, in `dist/engine`). `apps/engine-sandbox`
drives it with no server, no auth and no React. `apps/web` mounts the same
package through `mountEngine`/`useCanvasEngine`.

The hero builder takes that shape, adjusted for what it is. The builder is
React on the same React as the web app, not a separate runtime behind a canvas.
Mounting it the way the engine is mounted would buy nothing and cost a second
React, which is the "Invalid hook call" failure that `apps/web/vite.config.mts`
already works around for `@thunderforge/genie`. So:

- **`packages/hero-builder` (`@thunderforge/hero-builder`)** is a library of
  React components. It holds the builder's structure and behaviour, takes React
  as a peer dependency, and never imports from `apps/web`.
- **`apps/hero-builder`** is a thin standalone app for phase (a). Like the
  sandbox, it has no server, no auth and no GraphQL, and a change means a
  reload.
- **`apps/web` imports the library** for phases (b) to (d), loaded only when a
  builder is opened.
- **`packages/heroes` stays free of any framework.** The seed script, the CLI
  and the e2e import it without React.

The reasons, and the options not taken, are in [research.md](./research.md) R1.

## Phases

Each phase ships by itself, and each is proven by an e2e rather than argued.
The table is the contract. The user stories below give the detail.

| Phase | Who | Ships | Done means | The e2e that proves it |
|---|---|---|---|---|
| **(a)** | Us | `packages/hero-builder`, `apps/hero-builder` | A hero can be made, tuned, exported and copied back into `presets.ts` without touching code, and every control comes from the package's data | `apps/hero-builder/e2e/builder.spec.ts`, which runs with no backend (US1, US2) |
| **(b)** | A GM, in a world | The builder in the NPC editor, and quick NPC in the compendium | A GM saves a built hero to an NPC as portrait and token, or makes a new NPC with art in three interactions | `apps/web/e2e/hero-builder-npc.spec.ts` (US3, US4) |
| **(c)** | A player | The builder on their own character, and during create-your-own | A player gives the character they hold a portrait and token, and nobody else's | `apps/web/e2e/hero-builder-player.spec.ts` (US5) |
| **(d)** | Everyone above | Saved specs, and heroes that travel in collections | A saved hero re-opens in the builder with its choices intact, including after it has been copied through a collection | `apps/web/e2e/hero-builder-saved.spec.ts` (US6, US7) |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - We tune a hero by looking at it (Priority: P1, phase a)

A developer opens the standalone builder, starts from a preset or from a
random hero, and changes parts and colours. The portrait and token redraw with
every change. When the hero is right, they export it as SVG or as spec JSON,
or copy it as a `HeroPreset` entry to paste into `presets.ts`.

**Why this priority**: it is the owner's first stated use ("for us to
generate"). It is also where the builder's controls, layout and behaviour get
proven before any player sees them. It needs no server.

**Independent Test**: with only the standalone app running, load a preset,
change one choice of every part, and export. Import the export back and
confirm it draws exactly what was on screen.

**Acceptance Scenarios**:

1. **Given** the standalone builder, **When** it opens, **Then** it shows a
   live portrait and token and one control group for every field in
   `HERO_PARTS`, `HERO_COLORS` and `HERO_FLAGS`, plus name and title.
2. **Given** any control, **When** its value changes, **Then** both previews
   redraw before the next interaction, without a reload.
3. **Given** the preset picker, **When** a preset is chosen, **Then** every
   control takes that preset's values, and the previews match the CLI's output
   for that preset exactly.
4. **Given** randomise, **When** it is pressed, **Then** a new valid hero is
   drawn from the closed lists and a seed is shown. The same seed later
   produces the same hero, and any part the user locked is left unchanged.
5. **Given** a finished hero, **When** it is exported, **Then** the user gets
   the portrait SVG, the token SVG and the spec as JSON. The spec contains only
   fields that differ from the defaults.
6. **Given** a finished hero, **When** "copy as preset" is used, **Then** the
   clipboard holds a `HeroPreset` entry (slug and spec) that can be pasted into
   `presets.ts`. After pasting, it passes the package's preset tests and draws
   identically.
7. **Given** a spec JSON pasted or loaded from a file, **When** it is valid,
   **Then** the builder shows it. **When** it is not, **Then** every problem
   is listed by field and nothing on screen changes.

---

### User Story 2 - The catalogue grows without the builder changing (Priority: P1, phase a)

A developer adds a new headgear to `packages/heroes` by drawing it in
`parts.ts` and adding it to the list and its label. The builder offers it the
next time it loads, and nobody edits the builder.

**Why this priority**: the builder's value grows with the catalogue. If every
part also needs a builder change, the catalogue stops growing. This is the
property the package's data exports exist to give, and it has to be proven, not
assumed.

**Independent Test**: add a throwaway choice to one part on a branch, run the
builder e2e unchanged, and confirm it finds, selects and draws the new choice.

**Acceptance Scenarios**:

1. **Given** a new choice added to any list in `HERO_PARTS`, **When** the
   builder loads, **Then** the choice is offered, labelled and selectable, with
   no change to `packages/hero-builder`.
2. **Given** a new colour field added to `HERO_COLORS` with its palette,
   **When** the builder loads, **Then** it has a colour control.
3. **Given** a new choice with no label, **When** the package test runs,
   **Then** the test fails, not the builder. The builder shows the choice under
   its key rather than hiding it.
4. **Given** a new choice that draws the same as an existing one, **When** the
   package test runs, **Then** the test fails, as it already does today.

---

### User Story 3 - A GM gives an NPC a face (Priority: P1, phase b)

A GM editing an NPC opens the builder from the imagery panel. The hero's name is
already the NPC's name. They choose parts, or start from a preset or a random
hero, and press "Save to this NPC". The portrait and token appear in the panel,
and the token stands on the map with that art.

**Why this priority**: it is the owner's second stated use ("GMs to build NPCs
fast"). It is also the first time the builder's output reaches the product,
through a path already proven by the P8 e2e.

**Independent Test**: as a GM, create an NPC, build a hero for it, save, and
confirm the portrait and token are stored and served as WebP and shown in the
panel.

**Acceptance Scenarios**:

1. **Given** a GM (Editor or above) on an NPC's edit page, **When** they open
   the imagery panel, **Then** a "Build a hero" control opens the builder with
   the NPC's name as the hero's name.
2. **Given** a hero in the builder, **When** "Save to this NPC" is pressed,
   **Then** the portrait and the token are both uploaded through
   `uploadActorImage`, and the panel shows both on success.
3. **Given** an NPC that already has a portrait or token, **When** the GM saves
   a built hero, **Then** they are told before saving that both will be
   replaced.
4. **Given** one upload succeeding and the other failing, **When** the result is
   shown, **Then** the GM is told which role failed and can retry that role
   alone. Nothing claims success for an image that was not stored.
5. **Given** a Viewer on the same page, **When** it renders, **Then** no build
   or save control is offered. The server refuses the upload regardless.
6. **Given** a builder that was closed without saving, **When** the GM returns,
   **Then** the NPC's images are unchanged.

---

### User Story 4 - Quick NPC: a named NPC with art in three interactions (Priority: P1, phase b)

A GM in the compendium needs a bandit captain now. They press "Quick NPC",
type a name and press Create, and a new NPC with a random portrait and token is
in the list. If they don't like the face, a reroll gives another before they
create.

**Why this priority**: "fast" is the owner's word. A full builder is fast for
one NPC and slow for six. Quick NPC is the builder with every choice made for
the GM, and randomising from closed lists is deterministic tooling, not an AI
(see Assumptions and ADR-051).

**Independent Test**: from an empty compendium, make three NPCs with quick NPC
and confirm each has a name, a portrait and a token, and that the three faces
differ.

**Acceptance Scenarios**:

1. **Given** the compendium, **When** a GM presses "Quick NPC", **Then** a
   small dialog shows a name field and the previews of a random hero.
2. **Given** a name, **When** the GM presses Create, **Then** the NPC is
   created, its portrait and token are uploaded, and it appears in the list,
   selected. The GM stays in the compendium so they can make another.
3. **Given** the dialog, **When** the GM rerolls, **Then** a different hero is
   shown, and "Open in builder" takes the current one into the full builder.
4. **Given** the NPC was created but an upload failed, **When** the result is
   shown, **Then** the NPC exists and is labelled as missing art, with a way to
   retry. The dialog does not report a success.
5. **Given** a Player, **When** they browse the compendium, **Then** no
   "Quick NPC" is offered.

---

### User Story 5 - A player builds their own hero (Priority: P2, phase c)

A player joining a world creates their own character and builds its hero in
the same step. A player who claimed a pre-made character opens it and changes
its look. Either way, the art they choose is the art their token wears on the
map.

**Why this priority**: it is the owner's "eventually for users". It reuses
everything phases (a) and (b) proved. The new work is authorisation: a player
must be able to change the imagery of the character they hold, and nothing
else.

**Independent Test**: as an invited player in a world that allows
player-created characters, create a character with a built hero. Confirm its
portrait and token are stored, then confirm a second player cannot change
them.

**Acceptance Scenarios**:

1. **Given** the "Create your own character" form, **When** a player uses it,
   **Then** they can build a hero before creating. The character is created
   first and its art uploaded second. If the upload fails, the character exists
   without art and the player can add it from their character's page.
2. **Given** a player's own character (created or claimed), **When** they open
   its page, **Then** the imagery panel and "Build a hero" are offered.
3. **Given** a character the player does not hold, **When** they try to change
   its imagery by any route, including a direct call to the mutation, **Then**
   the server refuses it.
4. **Given** a character a player holds, **When** the GM changes its art,
   **Then** the GM's change stands. The GM's authority over every actor in their
   world is unchanged.
5. **Given** a player whose claim is released, **When** they next try to change
   that character's imagery, **Then** they are refused.

---

### User Story 6 - A saved hero re-opens as a hero, not a picture (Priority: P2, phase d)

A GM who built an NPC's hero last week opens the builder on it again. It opens
with every choice as they left it. Changing the hat takes one click, not
rebuilding the hero from scratch.

**Why this priority**: without it, the builder writes pictures and forgets how
it made them, so every edit is a redraw. It is P2 because phases (b) and (c)
are useful without it, and it is the one phase that changes the data model.

**Independent Test**: build and save a hero for an NPC, reload, and re-open
the builder. Confirm every control shows the saved choice and the previews
match the stored images.

**Acceptance Scenarios**:

1. **Given** an actor whose images were saved from the builder, **When** the
   builder is opened on it, **Then** it starts from the saved spec, not from
   defaults.
2. **Given** an actor whose token was later replaced by an uploaded file,
   **When** the builder is opened, **Then** it starts from the portrait's spec
   and says the token no longer comes from a built hero.
3. **Given** an actor whose images were never built, **When** the builder is
   opened, **Then** it starts from the actor's name, as in phase (b).
4. **Given** a saved spec that no longer validates, for example because it
   names a choice the catalogue has since lost, **When** the builder opens on
   it, **Then** the problem is shown by field and the stored images are
   untouched. Nothing is silently redrawn with a default.

---

### User Story 7 - Heroes travel in collections (Priority: P3, phase d)

A GM puts their NPCs in a collection (spec 026) and shares it. A recipient who
copies it gets the NPCs with their faces, as spec 026 FR-018 already
guarantees. With phase (d), they also get the hero behind each face and can
re-open it in the builder in their own world.

**Why this priority**: it follows from US6 almost for free, because a saved spec
lives with the image and imagery already travels with a copied actor. It is P3
because it depends on US6.

**Independent Test**: build a hero for an NPC, add the NPC to a collection,
copy the collection into a second world, and re-open the builder there.
Confirm it is the same hero.

**Acceptance Scenarios**:

1. **Given** a collection containing an actor with a built hero, **When** it
   is copied into another world, **Then** the copy's images and the spec
   behind them both arrive.
2. **Given** the copied actor, **When** its new owner opens the builder,
   **Then** it opens from the copied spec, and saving changes the copy only.
3. **Given** a recipient previewing a collection before copying, **When** it
   lists what it contains, **Then** nothing about heroes changes what spec 026
   already shows.

---

### Edge Cases

- **Two heroes on one page.** The builder shows a portrait, a token and choice
  thumbnails at once, and a hero's default id prefix is derived from the hero.
  So the current hero's thumbnail and its main preview would share ids. A
  gradient reference that resolves to a hidden copy draws nothing. The builder
  therefore sets an explicit, unique id prefix on every SVG it puts in the
  document, and never relies on the default (FR-011).
- **A name that is markup.** `Bob <script>` is written as text by the
  renderer, and `validateHero` bounds its length. The builder never inserts the
  name anywhere as HTML.
- **An imported spec with an extra field** (`hat`, a misspelt `hair`).
  `validateHero` refuses it, the builder lists the problem, and nothing is
  drawn from it. "Mostly valid" is not partially applied.
- **An imported SVG instead of a spec.** Refused. The builder imports specs,
  never drawings (FR-026).
- **A colour typed as `red`, `#fff` or `#ffffff" onload="…`.** It is refused
  by the same rule the package enforces. The colour field shows the problem,
  and the hero keeps its previous colour.
- **Derived colours.** Trim follows the outfit, the ring follows the outfit, the
  beard follows the hair, and the accent follows the headgear. A user who never
  touched the trim sees it follow the outfit. A user who set it keeps their
  choice, and can put it back to following.
- **A preset whose skin is a named tone.** "Copy as preset" writes
  `SKIN_TONES.peach` rather than the hex value where the colour matches a
  named tone, so the pasted entry reads like its neighbours.
- **Saving to an NPC while another GM saves to the same NPC.** Each upload
  replaces its role, and the last write wins per role. This matches
  `uploadActorImage` today, and the panel shows what was actually stored.
- **The NPC is deleted while the builder is open.** Saving fails with the
  server's refusal, and the builder keeps the hero so it can be exported.
- **A part's drawing changes after a hero was saved.** Stored images are
  pixels and do not change. Re-opening the saved spec (phase d) shows the new
  drawing, and saving would store it. That is expected, and the builder does
  not warn about it.
- **A choice is renamed or removed from the catalogue.** Every saved spec and
  every copy in a collection that names it stops validating. Choices are
  therefore append-only once phase (d) ships (FR-038).
- **A phone, portrait orientation, 375 px wide.** The previews stay visible
  while the controls scroll, and nothing scrolls sideways (FR-020).
- **Keyboard only, or a screen reader.** Every control can be reached, operated
  and understood without a pointer (FR-017 to FR-019).
- **A slow or offline connection in phase (b).** The builder itself works
  offline once loaded, because drawing happens on the client. Only saving needs
  the server, and it says so when it fails.
- **Randomise producing an unreadable hero**, for example an outfit the same
  colour as the backdrop. Randomising draws colours from curated palettes, not
  from the whole colour space, so the result is a plausible hero (FR-008).

## Requirements *(mandatory)*

### Functional Requirements

**The builder, everywhere it appears**

- **FR-001**: The builder MUST render one control group for every field in
  `HERO_PARTS`, `HERO_COLORS` and `HERO_FLAGS`, plus the name and the title,
  and MUST derive both the fields and their choices from the package's exported
  data at run time. It MUST NOT carry its own list of parts, choices or colour
  fields.
- **FR-002**: Every choice MUST be shown with a human-readable label that
  comes from `packages/heroes`. Where a choice has no label, the builder MUST
  show its key rather than omit it.
- **FR-003**: A change to any control MUST redraw the portrait and token
  previews. Neither the page nor the builder may reload.
- **FR-004**: The builder MUST hold its state as a hero spec, and MUST run every
  spec it draws through `validateHero` first.
- **FR-005**: The builder MUST distinguish a field the user set from a field
  that follows its default, including defaults derived from other fields
  (trim, ring, beard colour, accent). It MUST let the user put a set field back
  to following.
- **FR-006**: The builder MUST offer every preset in `PRESET_HEROES` as a
  starting point.
- **FR-007**: Randomise MUST choose only from the closed lists and the
  package's palettes. It MUST be reproducible from a seed that is shown, and it
  MUST leave locked fields unchanged. Randomise is deterministic code in
  `packages/heroes`, and no model or service is involved.
- **FR-008**: Colour controls MUST offer curated swatches from
  `packages/heroes` (`SKIN_TONES` for skin, and a palette per colour field for
  the rest). Every colour control MUST also accept any `#rrggbb`. Randomise
  MUST use the swatches only.
- **FR-009**: The builder MUST export the spec as JSON containing only the
  fields that change the drawing. A field MUST be omitted exactly when removing
  it leaves the resolved hero unchanged, so derived colours keep following.
- **FR-010**: The builder MUST import a spec from pasted text or a file. A spec
  that fails `validateHero` MUST be refused whole, with every problem named by
  field, and MUST leave the current hero unchanged.
- **FR-011**: Every SVG the builder puts into the document MUST carry an id
  prefix unique within that document. No two elements in the document may share
  an id, however many heroes are shown.

**Phase (a): the internal tool**

- **FR-012**: A standalone builder MUST run with no server, no authentication
  and no GraphQL, from one workspace command, in the manner of
  `apps/engine-sandbox`.
- **FR-013**: The standalone builder MUST export the portrait SVG, the token
  SVG and the spec JSON, each as a file the user saves.
- **FR-014**: "Copy as preset" MUST produce a `HeroPreset` entry (a slug and
  a spec) that can be pasted into `presets.ts`. The entry MUST use a
  `SKIN_TONES` name where the skin matches one. Once pasted, it MUST pass the
  package's existing preset tests and draw exactly what the builder showed.
- **FR-015**: The builder MUST NOT write to the repository itself. Presets reach
  `presets.ts` through a human paste and a reviewed diff.
- **FR-016**: The builder library MUST be a workspace package that `apps/web`
  imports and that the standalone app hosts. It MUST take React as a peer
  dependency, MUST NOT import from `apps/web`, and MUST NOT add React or any
  other framework to `packages/heroes`.

**Accessibility, layout and performance**

- **FR-017**: Every control MUST be operable by keyboard alone. Each part's
  choices MUST behave as one group that arrow keys move within, and focus MUST
  be visible.
- **FR-018**: Every choice and every colour swatch MUST have an accessible name
  that does not depend on seeing it, for example "Headgear: wizard hat" or
  "Skin: peach". A colour's hex value MUST be available as text.
- **FR-019**: Where the builder opens as a dialog, focus MUST move into it,
  stay within it and return to the control that opened it.
- **FR-020**: At 375 px wide the builder MUST keep both previews visible while
  its controls scroll, MUST NOT scroll horizontally, and MUST give every
  control a touch target of at least 44 px.
- **FR-021**: The builder MUST draw the previews on the client, and a redraw
  MUST NOT wait on the network. Continuous input, such as dragging a colour
  picker, MUST redraw at most once per animation frame.
- **FR-022**: In `apps/web` the builder's code MUST load only when a builder is
  opened. Pages that do not open it MUST NOT grow.

**Phase (b): a GM, an NPC**

- **FR-023**: The actor imagery panel MUST offer "Build a hero" to anyone the
  panel already lets edit imagery. It MUST open the builder with the actor's name
  as the hero's name.
- **FR-024**: "Save to this NPC" MUST upload the portrait and the token through
  the existing `uploadActorImage` mutation, as SVG. The server's existing
  rasterisation is the only path by which a built hero becomes stored pixels.
- **FR-025**: Saving MUST report each role's outcome separately, MUST NOT report
  success for a role that was not stored, and MUST allow a failed role to be
  retried alone. Where the actor already has images, saving MUST say before it
  starts that they will be replaced.
- **FR-026**: The builder MUST NOT accept an SVG or any other drawing as input.
  The only thing it imports is a spec.
- **FR-027**: The compendium MUST offer "Quick NPC" to the users who may
  create NPCs there. It MUST make a named NPC with a random portrait and token
  from a name and one confirming action. Rerolling MUST be available before
  confirming, and "Open in builder" MUST hand the current hero to the full
  builder.
- **FR-028**: Where quick NPC creates the NPC but cannot store its art, the NPC
  MUST remain. It MUST be shown as lacking art, with a way to add it. It MUST
  NOT be deleted to hide the failure.
- **FR-029**: The builder MUST NOT communicate with the engine. A built token
  reaches the map the way any uploaded token does today.

**Phase (c): a player, their character**

- **FR-030**: A player MUST be able to set the portrait and token of a character
  they hold, whether they claimed it or created it (spec 017). The server MUST
  enforce this at the data boundary, and the grant MUST cover imagery only, not
  the rest of the actor.
- **FR-031**: That right MUST end when the claim ends. It MUST NOT extend to any
  actor the player does not hold.
- **FR-032**: The GM's existing authority over every actor in their world MUST be
  unchanged, including the authority to replace a player's art.
- **FR-033**: The imagery panel, with "Build a hero", MUST be available on a
  character's own page to anyone permitted to change that character's imagery.
- **FR-034**: "Create your own character" MUST allow a hero to be built as part
  of creating. It MUST create the character first and upload the art second,
  and it MUST follow FR-028 when the upload fails.

**Phase (d): saved specs and collections**

- **FR-035**: When the builder saves an image, the spec that drew it MUST be
  stored with that image, recorded per role. An image stored any other way MUST
  have no spec. Replacing a built image with an uploaded file MUST clear that
  role's spec.
- **FR-036**: Opening the builder on an actor MUST start from its stored spec,
  preferring the portrait's. Where the two roles' specs differ, or only one role
  has a spec, the builder MUST say so.
- **FR-037**: The server MUST treat a stored spec as untrusted input. It MUST
  refuse to store one that is not a JSON object, that is over a stated size, or
  that fails the package's published schema. Every client MUST still run
  `validateHero` before drawing a stored spec.
- **FR-038**: Once saved specs exist, choices in `HERO_PARTS` MUST be
  append-only. A choice may change how it draws, but it MUST NOT be renamed or
  removed, because doing so would stop every stored spec that names it from
  validating.
- **FR-039**: A copied actor (spec 026) MUST carry each image's spec along with
  the image, so the copy re-opens in the builder in its new world.
- **FR-040**: A user's data export MUST include the specs stored against actors
  they own, alongside the images.

### Key Entities

- **Hero spec**: a name, an optional title, and whichever parts, colours and
  flags differ from the defaults. It is plain JSON and always untrusted when it
  comes from outside the code. It is the only thing the builder imports or
  exports as data.
- **Resolved hero**: a spec with every default filled in. It is what the parts
  draw from, and only `validateHero` produces one.
- **Catalogue**: the exported lists of parts, choices, colour fields, flags,
  labels and palettes in `packages/heroes`. The builder's controls come from
  this and from nowhere else.
- **Rendered hero**: the portrait SVG and the token SVG for one resolved hero.
- **Actor image** (existing, ADR-057): one stored WebP per actor per role. From
  phase (d), it can also carry **the spec that drew it**.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001** (a): A developer goes from opening the standalone builder to a new
  preset pasted into `presets.ts` and passing `pnpm -F @thunderforge/heroes
  check` in under five minutes, without writing any code.
- **SC-002** (a): The builder e2e takes its expected controls from the package's
  data. Adding a choice to any part makes the e2e exercise it, with no edit to
  the builder or the e2e. This is shown by doing it once on a throwaway branch.
- **SC-003** (a): Export followed by import reproduces the on-screen hero with a
  byte-identical portrait and token, for every preset.
- **SC-004** (a): Every control can be operated by keyboard alone, and every
  choice and swatch has an accessible name. The e2e checks both.
- **SC-005** (a): At any point, the builder page contains no duplicated element
  id. The e2e checks this after loading every preset.
- **SC-006** (a): A change to any control is reflected in both previews within
  100 ms on the reference machine, measured, not estimated.
- **SC-007** (b): A GM gives an existing NPC a built portrait and token in
  under 60 seconds from opening its edit page.
- **SC-008** (b): From the compendium, a GM creates a named NPC with a portrait
  and a token in at most three interactions (open quick NPC, type the name,
  create) and under 20 seconds.
- **SC-009** (b, c): Everything a built hero stores is WebP, and the engine
  draws the token on the map as it draws any uploaded token.
- **SC-010** (c): A player can change the imagery of the character they hold and
  of no other actor. This is shown by attempting both in the e2e, including
  through a direct mutation call.
- **SC-011** (d): Re-opening a saved hero shows every choice exactly as saved,
  in the world it was made in and in a world it was copied to.
- **SC-012**: The builder adds nothing to the initial load of any `apps/web`
  page that does not open it, as measured by the built chunk graph.

## Assumptions

- **Randomising is not AI.** Quick NPC and randomise choose from closed lists
  with a seeded generator. They make no decision a GM would make and replace
  nobody. ADR-051 governs anything AI-adjacent, and no AI feature is proposed
  here (see Out of Scope).
- **The art style is ThunderForge's own.** Every part is drawn by us in
  `parts.ts`, so every hero is ours to redistribute. No part is copied or traced
  from a publisher's artwork or a game system's sheet. Where a system suggests
  what a character looks like, that is scope, not style.
- **The server never sees a spec in phases (a) to (c).** It sees an SVG, which
  it already accepts from anyone with Editor on the actor, and it stores WebP.
  Phase (d) is the first time a spec is stored, and FR-037 covers it.
- **One hero per actor.** A portrait and a token drawn from the same spec is the
  normal case. A different hero per role is possible but gets no extra support.
- **The hero's name is the actor's name.** In phases (b) and (c) the builder
  takes the actor's label and does not offer to rename the actor. It is the
  portrait's accessible label, and it is not drawn.
- **Uploading SVG, not rasterising in the browser.** The existing server path
  is the single rasteriser, at a fixed 1024 px edge, for every client. Research
  R3 records the alternative.
- **The standalone builder is a developer tool** and is not deployed with a
  release, like the engine sandbox.
- **Twelve presets are enough to start.** Growing the catalogue is ongoing work
  that FR-001 and US2 make cheap. It is not a phase of this spec.

## Security: what is and is not trusted

| Thing | Trusted? | Why |
|---|---|---|
| Code in `packages/heroes` (parts, lists, palettes) | Yes | It is ours and reviewed. It is the only source of SVG markup. |
| A spec written in code (presets, tests) | Validated anyway | `createHero` resolves every spec through `validateHero` and throws on a bad one. |
| A spec from import, paste, file, storage or a collection copy | **No** | `validateHero` is the client-side gate. Unknown fields and choices are refused, colours must match `#rrggbb`, and text is bounded. Nothing draws before it passes (FR-004, FR-010). |
| The name and title | **No** | Free text. The renderer escapes it into the SVG as text, and the builder never inserts it as HTML. |
| An SVG from anywhere | **No, and not accepted** | The builder imports specs only (FR-026). |
| The SVG the builder uploads | **No, from the server's side** | The server cannot tell a built SVG from a hand-made one, and does not try. `svg.rs` loads no referenced image, follows no data or file URL, draws no text and ignores the declared size. The upload keeps its size limit and its Editor gate. A built hero widens nothing, because any Editor could already upload any SVG. |
| A stored spec (phase d) | **No** | The server shape-checks it and caps its size before storing it (FR-037), and clients validate again before drawing. The server cannot verify that a spec matches the image stored beside it. A mismatch requires a hand-made request by someone who could already upload any image, and it misleads only the builder's starting point. |
| The id prefix | Validated | The renderer accepts only `^[A-Za-z][\w-]{0,63}$`, and the builder generates it (FR-011). |

## Out of Scope

- **Any AI-generated art or AI-chosen hero.** Per ADR-051 an AI tool may
  assist a human GM, and could one day suggest a hero that a GM then edits in
  this builder. That would be optional assistance to a human, and it is not
  proposed here.
- **User-supplied parts.** The catalogue is code in `packages/heroes`. Drawing
  new parts in the product is a different feature with a different trust model.
- **Heroes outside a world.** A hero stored on a user's profile, or a hero as a
  collection member of its own without an actor, belongs to the future
  profile-content spec. Until then, a hero travels by travelling with the actor
  that wears it (US7).
- **Extra image roles** (talking, not-talking, background, from ADR-057). They
  are additive later, and the builder would draw them from the same spec.
- **A stat block for quick NPC.** Quick NPC makes a named NPC with a face, and
  the sheet is filled in the way it is today.
- **Name suggestions.** The GM types the name.
- **Animated or layered tokens.**
- **Items and scenes.** The builder draws heroes only.

## Dependencies

- `packages/heroes`, which phase (a) extends with labels, palettes, a seeded
  randomiser and minimal-spec export. See research R2.
- ADR-057 and `uploadActorImage` (spec 031 FR-036), for phases (b) and (c).
- Spec 017 (player onboarding: claiming and player-created characters), for
  phase (c).
- Spec 026 (content collections), FR-018 in particular, for phase (d).
- Phase (d) changes the data model. Under Constitution Principle IV it needs an
  ADR, landed with it, recording R4's decision.

## Questions for the owner

1. **Does a player's art need a GM's say-so?** As specified, a player can change
   the portrait and token of the character they hold at any time, and the GM can
   always replace them (FR-030 to FR-032). Some tables will want the GM to be
   able to lock a character's look, or to approve it. Is either wanted, and if so,
   is it a per-world setting?
2. **Should quick NPC ever do more than a face and a name?** It currently stops
   there, and the sheet is filled in as it is today. If you want it to also apply
   a game system's NPC template, that is a second spec touching the system packs,
   and it is worth knowing now so the dialog leaves room for it.
3. **Should the standalone builder ever be public?** It is currently a
   developer tool, like the engine sandbox, and users get the builder inside the
   product. A public "make your hero" page on the site would need hosting and a
   privacy note, but no server work, since phase (a) needs none.
