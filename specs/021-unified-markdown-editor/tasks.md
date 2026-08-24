---

description: "Task list for Unified Code-Editor Markdown Experience"
---

# Tasks: Unified Code-Editor Markdown Experience

**Input**: Design documents from `/specs/021-unified-markdown-editor/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/markdown-editor-props.md, quickstart.md

**Tests**: Included — quickstart.md commits to running `apps/web/e2e/lore-wiki.spec.ts`, and research.md R4 already scoped exactly what in it must change.

**Organization**: Tasks are grouped by user story per spec.md's priorities (US1 P1, US2 P1, US3 P2, US4 P2).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1 / US2 / US3 / US4

## Path Conventions

Frontend-only change inside the existing `apps/web` structure — no new top-level directories (plan.md's Project Structure).

---

## Phase 1: Setup

**Purpose**: Declare the CodeMirror packages this feature needs directly, rather than relying on their current transitive/undeclared availability.

- [X] T001 Add `@codemirror/language`, `@codemirror/view`, and `@codemirror/autocomplete` as direct dependencies in `apps/web/package.json`, pinned to the versions already resolved in `pnpm-lock.yaml` (`@codemirror/autocomplete@6.20.3` per research.md R5); run `pnpm install` and confirm the lockfile doesn't otherwise change

**Checkpoint**: Dependencies declared; `pnpm install` completes clean.

---

## Phase 2: Foundational

No shared blocking code is needed beyond Phase 1's dependency install — User Story 1 (Session Notes) and the User Story 2→3→4 chain (Lore) touch entirely separate files with no shared new abstraction between them (plan.md's Project Structure). Proceed directly to Phase 3.

---

## Phase 3: User Story 1 — Session Notes reads as a real code editor (Priority: P1) 🎯 MVP

**Goal**: Session Notes' existing CodeMirror instance gains visible line numbers and a working fold gutter.

**Independent Test**: Open Session Setup as GM, open the notes editor, confirm line numbers and a functional fold gutter are visible — no other part of this feature needs to exist.

### Tests for User Story 1

- [X] T002 [P] [US1] Add a test to `apps/web/e2e/session-notes.spec.ts` asserting line numbers (`.cm-gutterElement` or CodeMirror's line-number gutter class) and a working fold gutter (collapsing a heading's section) are visible in the session notes editor — follow this file's own existing `.cm-content`/`.cm-line` interaction pattern (already correct for CodeMirror, per research.md R4), not `.fill()`/`.inputValue()`

### Implementation for User Story 1

- [X] T003 [US1] In `apps/web/src/components/world/SessionNotesPanel/MarkdownCodeEditor.tsx`, import `foldGutter` from `@codemirror/language`, add it to the `extensions` array alongside `markdown()`, and change `basicSetup={{ lineNumbers: false, foldGutter: false }}` to enable both (research.md R1 confirms `@codemirror/lang-markdown` already ships real heading-based folding — no custom fold logic needed)
- [X] T004 [US1] Run `pnpm --filter web exec tsc --noEmit` and manually validate quickstart.md Scenario A

**Checkpoint**: User Story 1 fully functional and independently testable/shippable — Session Notes now reads as a real code editor.

---

## Phase 4: User Story 2 — Lore entries move onto the same real code editor (Priority: P1)

**Goal**: The Lore entry editor's plain `<textarea>` is replaced with the same CodeMirror-based experience Session Notes uses (base editing only — autocomplete and image upload are User Stories 3/4).

**Independent Test**: Open an existing lore entry in edit mode; confirm its content displays unchanged in a line-numbered, fold-gutter, syntax-highlighted CodeMirror instance, and that typing/selection/undo/redo/save all work.

### Tests for User Story 2

- [X] T005 [US2] Update `apps/web/e2e/lore-wiki.spec.ts`'s `createLoreEntry` helper and the GFM-constructs rendering test (`US1: DM authors a lore entry...`) to type content via `page.keyboard.type(...)` after clicking `.cm-content` and to read content back via a `.cm-line`-based helper, instead of `Locator.fill()`/`.inputValue()` against `[data-testid="lore-markdown-editor-textarea"]` — mirror `apps/web/e2e/session-notes.spec.ts`'s existing helpers (`getEditorValue`-style pattern) rather than inventing a new one (research.md R4)

### Implementation for User Story 2

- [X] T006 [US2] Rewrite `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx`'s rendering: replace the `<Textarea>` with a `<CodeMirror>` instance (`@uiw/react-codemirror`, `markdown()` from `@codemirror/lang-markdown`, `foldGutter()`, `lineNumbers: true`), keeping the exact same public props (`loreEntryId`, `worldId`, `value`, `onChange`, `disabled`) per contracts/markdown-editor-props.md, and keep `data-testid="lore-markdown-editor-textarea"` on CodeMirror's outer root element. Do not yet port the `[[` autocomplete or paste/drop handlers (User Stories 3/4) — this task is the base editing swap only, matching data-model.md's "no content transformation" constraint (FR-005)
- [X] T007 [US2] Run `pnpm --filter web exec tsc --noEmit` and manually validate quickstart.md Scenario B, including reloading an existing lore entry to confirm its stored content renders unchanged

**Checkpoint**: User Stories 1 AND 2 both work independently — both editors now present the same real code-editor experience (autocomplete/image-upload parity lands in the next two phases).

---

## Phase 5: User Story 3 — `[[Title]]` link autocomplete keeps working (Priority: P2)

**Goal**: Typing `[[` in the migrated Lore editor still surfaces a matching-title popover and inserts a resolved `[[Title]]` reference on selection.

**Independent Test**: In the migrated lore editor, type `[[` plus a partial title matching an existing lore entry/actor; confirm a popover of matches appears and selecting one inserts the correct reference at the right position.

### Tests for User Story 3

- [X] T008 [P] [US3] Rewrite the two link-autocomplete tests in `apps/web/e2e/lore-wiki.spec.ts` (currently asserting `page.getByTestId("lore-link-autocomplete")`/`lore-link-target-{id}`) to assert against `@codemirror/autocomplete`'s own rendered DOM (`.cm-tooltip-autocomplete`, `.cm-completionLabel` — contracts/markdown-editor-props.md's testid table) instead, keeping the same test intent (type `[[partial`, confirm a match appears, select it, confirm the correct `[[Title]]` text lands)

### Implementation for User Story 3

- [X] T009 [US3] In `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx`, add `autocompletion()` (from `@codemirror/autocomplete`) to the CodeMirror `extensions` array with a custom async `CompletionSource`: use `context.matchBefore(/\[\[[^\]]*/)` to detect the trigger (mirrors the existing `/\[\[([^[\]]*)$/` regex this file already has), call the existing `getLoreLinkTargets(worldId, prefix)` for matches, and return one `Completion` per match whose `apply` replaces the matched range with `[[Title]]` (research.md R2). Remove the now-unused Radix `Popover`/`PopoverAnchor`/`PopoverContent` JSX and the manual `linkQuery`/`linkTargets` state this replaces
- [X] T010 [US3] Run `pnpm --filter web exec tsc --noEmit` and manually validate quickstart.md Scenario C, including the "no match → no popover" and "typing past `]]` closes it" cases

**Checkpoint**: User Stories 1, 2, and 3 all independently functional — link autocomplete has full parity on the new editor.

---

## Phase 6: User Story 4 — Paste/drop image upload keeps working (Priority: P2)

**Goal**: Pasting or dragging an image into the migrated Lore editor still uploads it and inserts a markdown image reference at the cursor.

**Independent Test**: In the migrated lore editor, paste a copied image and separately drag-and-drop an image file; confirm both upload and insert a working markdown image reference at the cursor.

### Tests for User Story 4

- [X] T011 [P] [US4] Update `apps/web/e2e/lore-wiki.spec.ts`'s paste-image test: its `pasteImageIntoEditor` helper (dispatches a synthetic `ClipboardEvent` via `.evaluate()` against `[data-testid="lore-markdown-editor-textarea"]`) keeps working unchanged since the testid stays on CodeMirror's root and the event bubbles to the `domEventHandlers.paste` extension (research.md R3/R4) — only its assertion needs to change, from `.inputValue()` to the `.cm-line`-based read helper introduced in T005

### Implementation for User Story 4

- [X] T012 [US4] In `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx`, add an `EditorView.domEventHandlers({ paste, drop })` extension (from `@codemirror/view`) reusing the existing image-detection logic (`Array.from(event.clipboardData/dataTransfer.items/files).find(item => item.type.startsWith("image/"))`) and the existing `uploadFile`/`uploadLoreImage(loreEntryId, file)` call; insert the resulting `![name](url)` markdown via `view.dispatch({ changes: { from: view.state.selection.main.head, insert: text } })` instead of the old `insertAtCursor`/`setSelectionRange` approach (research.md R3). Keep the existing `isUploading`/`uploadError` state and `StatusBadge` feedback unchanged
- [X] T013 [US4] Run `pnpm --filter web exec tsc --noEmit` and manually validate quickstart.md Scenario D, including the upload-failure and non-image-file cases

**Checkpoint**: All four user stories independently functional — the Lore editor now has full parity with, and the same underlying editor as, Session Notes.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final cleanup and full-suite verification across all four stories together.

- [X] T014 [P] Update the doc comments at the top of `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx` (currently "deliberately not a rich-text/WYSIWYG framework... a plain `<textarea>`-based Markdown editor") and `apps/web/src/components/world/SessionNotesPanel/MarkdownCodeEditor.tsx` to reflect the new shared CodeMirror-based design, removing now-stale rationale
- [X] T015 [P] Run `pnpm --filter web exec tsc --noEmit` across the whole `apps/web` project (not just the touched files) and confirm zero new errors
- [X] T016 Run the full `apps/web/e2e/lore-wiki.spec.ts` and `apps/web/e2e/session-notes.spec.ts` suites and confirm zero regressions in any test not directly touched by this feature (e.g. the GM staging page / renaming / lore-linked-from tests in `lore-wiki.spec.ts` that don't touch the editor directly)
- [X] T017 Run quickstart.md's full validation (Scenarios A–D) end-to-end in a real browser session

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Empty — no blocking shared code beyond Setup.
- **User Stories (Phase 3-6)**: All depend on Phase 1 completion (the declared dependencies).
  - **User Story 1 has no dependency on User Stories 2-4** — separate file (`MarkdownCodeEditor.tsx`), ships alone as the MVP.
  - **User Story 2 has no dependency on User Story 1** — separate file (`LoreMarkdownEditor.tsx`) — but User Stories 3 and 4 both depend on User Story 2's base CodeMirror migration existing first (they extend the same component's `extensions` array).
  - **User Stories 3 and 4 have no dependency on each other** — both add an independent CodeMirror extension to the same file; can be implemented in either order or in parallel by different people, at the cost of a likely merge in that one file.
- **Polish (Phase 7)**: Depends on all four user stories being complete.

### Parallel Opportunities

- T002 (US1 test) can be written in parallel with nothing else in its phase (it's the only task before T003's implementation) — but Phase 3 as a whole can run fully in parallel with Phases 4-6, since User Story 1 touches a completely different file than User Stories 2-4.
- T008 (US3 test) and T011 (US4 test) can be done in parallel with each other once T006 (US2's base migration) is complete, since they target different existing tests within the same spec file (real risk of git-merge conflict in that one file, not a logical dependency).
- T014/T015 in Polish are file-independent and can run in parallel with each other.

---

## Parallel Example: User Story 1 (fully parallel with User Stories 2-4)

```bash
# Once Phase 1 (Setup) is done, these can run at the same time by different people:
Task: "US1 — enable line numbers + fold gutter in MarkdownCodeEditor.tsx"
Task: "US2 — migrate LoreMarkdownEditor.tsx off <Textarea> onto CodeMirror"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 3: User Story 1.
3. **STOP and VALIDATE**: quickstart.md Scenario A.
4. Ship — Session Notes alone already delivers "a real browser code editor" for one of the two surfaces, independent of anything else in this feature.

### Incremental Delivery

1. Setup → Foundation ready (trivially — Phase 2 is empty).
2. Add User Story 1 → validate Scenario A → ship (MVP: Session Notes looks like a real editor).
3. Add User Story 2 → validate Scenario B → ship (Lore entries now use the same editor, base editing only).
4. Add User Story 3 → validate Scenario C → ship (`[[Title]]` autocomplete parity restored).
5. Add User Story 4 → validate Scenario D → ship (paste/drop image upload parity restored) — at this point the Lore editor has full parity with today's `<textarea>` version plus the new editor experience.
6. Phase 7 polish once all four are in.

Note: shipping User Story 2 alone (without 3 and 4) is a real, if temporary, regression for lore authors who use the autocomplete/image-upload features today — the "incremental delivery" framing above describes what's independently *testable*, not a recommendation to leave User Stories 3/4 undone for long in production.
