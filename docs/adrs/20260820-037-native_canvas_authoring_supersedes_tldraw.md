# ADR-037: Native Bevy Canvas Authoring Supersedes Wrapped tldraw

**Date:** 2026-08-20
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Supersedes:** ADR-004 (Fantasy UI Shell with Radix Primitives, Dicebear Identity Surfaces, and Wrapped tldraw Chrome) — specifically its "wrapped tldraw" decision. ADR-004's Radix/Dicebear/fantasy-shell decisions are unaffected and remain in force.
**Extends:** ADR-032 (Canvas Rendering Strategy — Bevy)

---

## Problem Statement

ADR-004 wrapped tldraw to own whiteboard/annotation document editing on
the world canvas, while Bevy (ADR-032) separately owned tokens, grid, and
fog. This split required a sync bridge between two independent document
stores and left the canvas's most tactically important authoring surface
— walls (line-of-sight/movement blockers) — with no editor at all, even
after the Phase 6 walls backend shipped.

## Decision

This is a **full replacement** of tldraw, not a subset: all canvas
authoring — walls (with door semantics), lighting, and the complete
shape/drawing tool set tldraw provided (freehand, rectangle, ellipse,
line/arrow, text) — moves into the Bevy engine as independently modular
plugins (`WallPlugin`, `LightingPlugin`, `ShapePlugin`), per Constitution
Principle II. All of them render through one shared, explicit layer
ordering (`CanvasLayerPlugin` — background/map art, grid, walls, lighting,
shapes, tokens, fog-of-war), generalizing the layer stack ADR-032 already
sketched into a first-class resource instead of ad hoc per-plugin z-order.

Scope also includes native import of Universal VTT (`.dd2vtt`) map files
via a new `MapImportPlugin` + server-side import endpoint: a GM can bring
in an existing map's background art, wall/vision geometry, doors, and
lights in one action instead of retracing them by hand — this was
identified as a hard "out of the gate" requirement, not an optional
nice-to-have, since a substantial library of pre-authored maps in this
format already exists and manual retracing would undercut the value of
the wall/lighting authoring tools above.

tldraw is removed once `ShapePlugin` reaches full parity with tldraw's
tool set (sequencing detail: `specs/001-bevy-canvas-authoring/research.md`
§6).

This directly implements Constitution Principle I (ECS owns canvas
simulation) and closes the dual-store problem ADR-004 introduced: there is
now exactly one canvas authority.

Full requirements, data model, and API contract:
`specs/001-bevy-canvas-authoring/`.

## Rationale (Y-Statement)

In the context of ThunderForgeVTT's canvas architecture, facing the need
to author walls/lighting/shapes and import pre-authored map content
without a second competing state store, we decided to build native Bevy
authoring plugins for all of it (including a `.dd2vtt` import pipeline)
and remove tldraw, accepting the cost of writing our own drawing/selection
UX and a format parser, to achieve a single source of truth for canvas
state, day-one usability of existing map libraries, and a consistent,
modular extension point for future authoring tools, since a wrapped
third-party editor cannot be extended to understand walls/lighting/import
without becoming a second simulation authority.

## Consequences

- **Positive**: single canvas authority; walls become usable for the
  first time; lighting, shapes, and imported map content share one
  occlusion/rendering/layering pipeline; each capability is independently
  addable/removable per Principle II; existing `.dd2vtt` map libraries are
  usable on day one instead of requiring manual retracing.
- **Negative**: loses tldraw's mature drawing UX polish (snapping,
  multi-select, undo history depth) — v1 shape scope matches tldraw's
  core tool set but not every refinement; import is one-shot ingestion,
  not a live/two-way sync with the source file (spec Assumptions).
- **Follow-up**: done, 2026-08-20 (T061-T063). `tldraw` package dependency
  and `engine/tldraw/` (`WorldWhiteboard.tsx` + its stylesheet) removed
  from `apps/web`; `WorldLayout`'s dedicated whiteboard panel column
  removed too (the Bevy canvas now fills that reclaimed space), and every
  remaining prose reference to tldraw across the shell (SEO copy,
  dashboard/layout descriptions, the `EngineCommandSource` union's dead
  `"tldraw"` variant) was swept out — confirmed zero repo-wide matches for
  "tldraw" outside this ADR's own history. Full e2e coverage
  (`apps/web/e2e/canvas-authoring.spec.ts`) passing throughout the
  removal, including immediately after, confirms this wasn't done on
  faith — this superseded ADR-004's tldraw decision without regressing
  the feature it replaced.
