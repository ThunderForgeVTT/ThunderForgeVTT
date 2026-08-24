# Implementation Plan: Unified Code-Editor Markdown Experience

**Branch**: `021-unified-markdown-editor` | **Date**: 2026-08-24 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/021-unified-markdown-editor/spec.md`

## Summary

Two markdown-authoring surfaces exist today with inconsistent editing
experiences: Session Notes already uses CodeMirror 6 but with line
numbers and the fold gutter turned off, so it doesn't read as a code
editor; Lore entries use a plain `<textarea>` with two hand-built
features (`[[Title]]` link autocomplete, paste/drop image upload) wired
directly to raw textarea cursor-position/DOM events. This plan turns on
Session Notes' existing line numbers/fold gutter, migrates the Lore
editor onto the same CodeMirror component, and rebuilds its two custom
features against CodeMirror's own extension system (`@codemirror/autocomplete`
for the link popover, CodeMirror's `EditorView.domEventHandlers` for
paste/drop) instead of raw textarea events — with no changes to any
backend contract, since both features already call existing GraphQL
operations (`loreLinkTargets`, `uploadLoreImage`) unchanged.

## Technical Context

**Language/Version**: TypeScript, React 19 (apps/web)

**Primary Dependencies**: `@uiw/react-codemirror` (already used by `MarkdownCodeEditor.tsx`), `@codemirror/lang-markdown` (already a direct dependency), `@codemirror/autocomplete` and `@codemirror/view` (already present transitively via `@uiw/react-codemirror`'s own dependency tree per `pnpm-lock.yaml`, not yet a direct `package.json` dependency — promoting them to direct deps is this plan's only dependency-manifest change)

**Storage**: N/A — no new persisted data; reuses `world_lore_entries.content`/`worlds.session_notes` (existing columns) and the existing `loreLinkTargets`/`uploadLoreImage` GraphQL operations unchanged

**Testing**: Playwright e2e (`apps/web/e2e/lore-wiki.spec.ts` — existing coverage for US1/US3/US4's acceptance scenarios that must be rewritten, not just left passing, since it currently drives the editor via `Locator.fill()`/`.inputValue()`, APIs that only work against a real `<textarea>`/`<input>`, not CodeMirror's contenteditable-based DOM)

**Target Platform**: Browser (Chromium, per existing Playwright project config)

**Project Type**: Web application frontend-only change (apps/web) — no backend changes

**Performance Goals**: No new performance target beyond "at least as responsive as the `<textarea>` it replaces" (spec.md Edge Cases) — CodeMirror 6's own viewport-virtualized rendering already handles large documents better than a plain textarea by default, so no additional work is anticipated here

**Constraints**: Must not change any lore entry's stored/rendered markdown content (FR-005); must preserve existing keyboard text-editing behavior (FR-006) and existing keyboard reachability of the autocomplete popover (FR-007); drag-and-drop image upload remains a desktop/mouse-only affordance for this feature's MVP (Clarifications)

**Scale/Scope**: Two components (`MarkdownCodeEditor.tsx` config change; `LoreMarkdownEditor.tsx` full rewrite), one e2e spec file rewrite (`lore-wiki.spec.ts`), no backend/database changes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS Owns Simulation)**: N/A — this touches no canvas/engine state; it's React chrome (markdown authoring panels), exactly the layer Principle I already assigns to React.
- **Principle II (Plugin-Modular Engine)**: N/A — no Bevy engine changes.
- **Principle III (Ownership & Authorization at the Data Boundary)**: Unaffected — both editors continue calling the same already-authorized GraphQL operations (`uploadLoreImage`, `loreLinkTargets`, lore entry save, session notes save) with no change to what's sent or how it's authorized server-side. No new mutation, no new authorization surface.
- **Principle IV (Real ADRs and Specs Before Divergent Implementation)**: Satisfied by this spec + plan; no new architectural boundary is introduced (CodeMirror is already an established dependency in this codebase, not a new one) so no new ADR is warranted.
- **Principle V (Verify Before Claiming Done)**: Implementation phase will run `pnpm exec tsc --noEmit` and the (rewritten) `lore-wiki.spec.ts` e2e suite before claiming done, per standing project convention. N/A to gate at planning time; noted for the tasks phase.
- **DMCA Guardrail**: N/A — this feature doesn't expose one world's content beyond that world; it only changes how existing in-world content is authored.

No violations. Complexity Tracking not needed.

## Project Structure

### Documentation (this feature)

```text
specs/021-unified-markdown-editor/
├── plan.md              # This file
├── research.md           # Phase 0 output
├── data-model.md          # Phase 1 output (no new entities — documents why)
├── contracts/
│   └── markdown-editor-props.md   # Phase 1 output — component prop contracts, not a network API
└── quickstart.md          # Phase 1 output
```

(`tasks.md` is Phase 2, produced by `/speckit-tasks`, not this command.)

### Source Code (repository root)

Frontend-only change within the existing `apps/web` structure — no new
top-level directories:

```text
apps/web/src/
├── components/world/SessionNotesPanel/
│   └── MarkdownCodeEditor.tsx           # config change: enable lineNumbers + foldGutter
├── pages/world/lore/
│   └── LoreMarkdownEditor.tsx           # rewritten: CodeMirror + custom autocomplete/paste extensions
├── e2e/
│   └── lore-wiki.spec.ts                # rewritten: drive CodeMirror via keyboard events, not .fill()/.inputValue()
└── package.json                         # +@codemirror/autocomplete, +@codemirror/view as direct deps
```

**Structure Decision**: No new directories or architectural layers — this
is a targeted rewrite of two existing components plus their e2e coverage,
staying inside `apps/web`'s established structure. The two editors remain
separate component instances (spec.md's own Assumptions section already
accepts this — the experience is unified, not necessarily the code).

## Complexity Tracking

*No Constitution Check violations — this section is not applicable.*
