# Phase 0 Research: Interface Packs

Seven decisions. Each one was checked against the code as it stands on
2026-09-02, and the findings are recorded because several of them changed the
answer.

---

## 1. A pack is data, not a module

**Decision**: An interface pack is a single JSON manifest declaring values —
CSS custom properties for the React chrome, an `AppearanceOverride` for the
canvas. It contributes no JavaScript, no stylesheet, no ES module, and no
asset that executes.

**Rationale**: FR-003 requires that an interface pack *cannot* contribute
behaviour, enforced by automated validation rather than reviewer judgement.
There are two ways to get there: let a pack ship code and then police what the
code does, or give the format nowhere to put code. The first is the problem
ADR-029 exists to answer and has not; the second needs no answer at all. A
manifest of scalars validated with `serde`'s `deny_unknown_fields` — the exact
pattern `AppearanceOverride` already uses in `crates/thunderforge-canvas-core/
src/resource_display.rs` — rejects an unknown key before it can mean anything.

This is what makes SC-011 true: the interface half reaches a shippable state
with no dependency on the pack-code security decision, because it never asks
the question.

**Alternatives considered**:
- *A pack ships a CSS file.* Rejected. CSS is not inert: it can position a
  control off-screen, collapse it to zero size, or set `pointer-events: none`,
  which is FR-012's "hide, disable, or make unreachable" arriving through the
  side door. A fixed set of named custom properties cannot express any of those.
- *A pack ships an ES module exporting a theme object.* Rejected outright — it
  is arbitrary code, which is the thing this half is designed to avoid.

---

## 2. The theme vocabulary already exists, and there is only one of it

**Decision**: The pack's declarable surface is the CSS custom properties
defined in `apps/web/src/styles/globals.css` under `:root` and `.dark`.

**Finding that mattered**: the repo appears to have two token systems.
`globals.css` defines ~30 custom properties in oklch, consumed through Tailwind
v4's `@theme inline`; `apps/web/src/styles/tokens.scss` defines a separate
fantasy palette as **SCSS variables**. SCSS variables are substituted at build
time and cannot be swapped at runtime, which would have made half the app
unthemeable.

It is not half the app. `tokens.scss` is imported by **zero** files. It is a
fossil. The runtime-swappable custom properties are the whole vocabulary, and
the feature is therefore possible without a styling migration first.

**Consequence**: applying a pack is writing custom properties onto
`document.documentElement`. No stylesheet is fetched, no reload occurs, and
SC-001's 30 seconds has enormous headroom.

---

## 3. Bundled packs only, read from disk — no table, no upload

**Decision**: Interface packs live in `packs/interface/<id>/interface.json` and
are served by a route mirroring `src/server/src/systems.rs`. No
`interface_packs` table and no admin upload path in this increment.

**Rationale**: `game_systems` is a table because a system pack can be uploaded
and installed by an administrator, and installation is a fact that needs
recording. Nothing in User Story 1 requires an interface pack to arrive at
runtime. Adding a table and an upload flow now would build the install half of
a marketplace before the product has decided it wants one — and the moment
packs arrive from outside, the DMCA guardrail and ADR-029 both become live
questions that this increment is deliberately not answering.

Discovery from disk also gives FR-007 for free: Forge is present because it is
in the directory, on exactly the same footing as any other pack there.

**Alternatives considered**:
- *Mirror `game_systems` with an `interface_packs` table now.* Rejected as
  above. It is one migration to add later and nothing about the manifest format
  changes when it is.
- *Compile the packs into the web bundle.* Rejected: it makes Forge privileged
  by construction (it would be the one that ships) and violates FR-007's
  peer requirement in the one way that is hard to undo.

---

## 4. The engine gets the same pack through a command that already exists

**Decision**: The canvas half of a pack is an `AppearanceOverride`, sent as the
existing `set_display_appearance` external command when the world's pack is
resolved and whenever it changes.

**Finding that mattered**: `set_display_appearance` is fully implemented — the
command parses in `src/engine/src/lib.rs`, `StatusDisplayPlugin` holds the
`Appearance` resource, and the TypeScript SDK types it in
`apps/web/src/engine/sdk/commands.ts`. It has **no caller**. Every layer exists
and nothing joins them, which is the same shape as the two dead paths found
during spec 031. This feature is that command's first caller.

**Consequence**: no engine change is required for this increment. The engine
crate is touched by this feature only if the palette proves insufficient, and
that is not expected: `AppearanceOverride`'s seven fields cover track, fill
palette, undisclosed fill, and bar geometry, which is the entire visual surface
the status displays present.

---

## 5. Light/dark stays with the reader; the pack stays with the world

**Decision**: A pack declares **both** a light and a dark palette. The world's
Game Master chooses the pack; each participant keeps their own light/dark
choice, which continues to work exactly as `ThemeProvider` does today.

**Rationale**: the requester's decision makes the look table-wide, and the
accessibility reasoning that argued against that does not evaporate — it has to
land somewhere. It lands in two places: this split, and FR-012a's validation
floor. A reader who needs a dark screen at midnight is not overruled by their
Game Master's taste in ornament, and a Game Master who picks a pack is not
picking a brightness for six other people's rooms.

This also keeps the change small: `.dark` already exists as a variant and
`useTheme` already persists the reader's choice per browser.

**Alternatives considered**:
- *The pack fixes light or dark.* Rejected: it makes every pack a half-pack and
  turns the reader's existing toggle into a control that sometimes does nothing
  — FR-012's unreachable control, arrived at from the inside.

---

## 6. The legibility floor is WCAG contrast, computed in Rust, at validation

**Decision**: `pack_system_spec` gains a contrast check. Every foreground /
background pairing the manifest declares — text on background, text on card,
text on popover, primary-foreground on primary, and the same set again for the
dark palette — must meet a stated ratio. A pack that fails is rejected, naming
the pair and the mode that failed (FR-012a, SC-003a).

**Rationale**: the requester chose rejection over a warning, and the reason it
has to be rejection is FR-009: the reader cannot opt out of the world's look,
so there is no setting for a warning to point them at. An unreadable pack and
an unreachable control are the same failure with different mechanics.

Computed in Rust, in the validator crate, so there is exactly one implementation
of the rule and it runs in the same place the structural validation runs — not
once in a build script and once, differently, in a test. `luma()` in
`thunderforge-canvas-core` is Rec. 709 and already exists; WCAG relative
luminance is a near neighbour of it and the two should not be confused, so the
contrast module states which it uses and why.

**Open, for tasks**: the exact ratio. WCAG AA is 4.5:1 for body text and 3:1 for
large text and UI components. The floor should be AA, with the two thresholds
distinguished — a single 4.5 applied to a border colour would reject packs for
no reader's benefit.

---

## 7. Propagation is a world event, not a poll

**Decision**: Changing a world's interface pack records
`EVENT_CODE_WORLD_APPEARANCE_CHANGED` (code 23, the next free value in
`src/server/src/world_events.rs`), and every client in the world re-resolves
its appearance on receipt.

**Rationale**: SC-001 requires other participants to see the change without
reloading. The world-event channel is how every other cross-participant change
in this product travels — walls, lights, tokens, doors, combat — and a second
mechanism for this one would be a second thing to get wrong on reconnect. The
existing catch-up path (spec 028 US7) then covers a client that was offline
when the pack changed, at no additional cost.

**Alternatives considered**:
- *Re-fetch the world on an interval.* Rejected: it is a poll, and it makes the
  30-second bound in SC-001 a property of the interval rather than of the
  product.
