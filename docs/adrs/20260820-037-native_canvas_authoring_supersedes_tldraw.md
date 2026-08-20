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

All canvas authoring — walls, lighting, and freeform annotations — moves
into the Bevy engine as independently modular plugins
(`WallPlugin`, `LightingPlugin`, `AnnotationPlugin`), per Constitution
Principle II. tldraw is removed once `AnnotationPlugin` reaches parity
with the freeform-drawing capability it previously provided (sequencing
detail: `specs/001-bevy-canvas-authoring/research.md` §6).

This directly implements Constitution Principle I (ECS owns canvas
simulation) and closes the dual-store problem ADR-004 introduced: there is
now exactly one canvas authority.

Full requirements, data model, and API contract:
`specs/001-bevy-canvas-authoring/`.

## Rationale (Y-Statement)

In the context of ThunderForgeVTT's canvas architecture, facing the need
to author walls/lighting/annotations without a second competing state
store, we decided to build native Bevy authoring plugins for all three
and remove tldraw, accepting the cost of writing our own drawing/selection
UX, to achieve a single source of truth for canvas state and a
consistent, modular extension point for future authoring tools, since a
wrapped third-party editor cannot be extended to understand walls/lighting
without becoming a second simulation authority.

## Consequences

- **Positive**: single canvas authority; walls become usable for the
  first time; lighting and annotations share one occlusion/rendering
  pipeline; each tool is independently addable/removable per Principle II.
- **Negative**: loses tldraw's mature drawing UX (shape library, snapping,
  multi-select) — v1 annotation scope is intentionally narrower (spec
  Assumptions: strokes/shapes/text only, no rich text/images).
- **Follow-up**: `tldraw` package dependency and `engine/tldraw/` removed
  from `apps/web` once Scenario 3 (quickstart.md) passes.
