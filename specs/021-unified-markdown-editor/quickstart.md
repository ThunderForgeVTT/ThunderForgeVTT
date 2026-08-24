# Quickstart: Validating the Unified Code-Editor Markdown Experience

Validates spec 021's four User Stories end-to-end once implemented.

## Prerequisites

- Local stack running (`make dev`), logged in as a world's GM/owner.
- A world with at least one existing lore entry containing markdown
  (headings, a link, some emphasis) to confirm no content is altered by
  the migration (FR-005).
- A second lore entry or actor to use as an autocomplete target (User
  Story 3), and an image file/clipboard image to test upload (User
  Story 4).

## Scenario A — Session Notes reads as a real code editor (User Story 1)

1. Open the world's Session Setup page as GM.
2. Open the session notes editor; confirm line numbers appear down the
   left edge and a fold gutter is visible.
3. Type a markdown heading followed by a paragraph; click the fold
   control next to the heading; confirm the paragraph collapses/expands.
4. Type further markdown syntax (bold, a link, a code span); confirm
   live syntax highlighting, matching today's existing behavior.

**Expected outcome**: All 3 Acceptance Scenarios of User Story 1 pass.

## Scenario B — Lore entries move onto the same editor (User Story 2)

1. Open an existing lore entry (with markdown content) in edit mode.
2. Confirm the content displays unchanged, now with line numbers, a fold
   gutter, and live syntax highlighting.
3. Type new content; confirm cursor movement, selection, undo (Ctrl/Cmd+Z),
   and redo all work as expected.
4. Save; reload the entry (or view it as another user); confirm the
   rendered output is identical to what it was before the migration for
   the same markdown source (see data-model.md — no content transformation).

**Expected outcome**: All 3 Acceptance Scenarios of User Story 2 pass.

## Scenario C — `[[Title]]` autocomplete still works (User Story 3)

1. In the migrated lore editor, type `[[` followed by a few characters of
   an existing lore entry's or actor's title.
2. Confirm a popover of matching titles appears (see
   contracts/markdown-editor-props.md's note on the popover now being
   `@codemirror/autocomplete`'s own, not the old Radix `Popover`).
3. Select a suggestion (mouse click, or arrow keys + Enter); confirm the
   typed `[[partial` text is replaced with the full `[[Title]]` reference
   and the cursor lands immediately after it.
4. Type `[[` followed by text matching nothing; confirm no popover
   appears.
5. Type `[[` followed by a title, then keep typing past `]]`; confirm the
   popover closes without further altering the text.

**Expected outcome**: All 4 Acceptance Scenarios of User Story 3 pass.

## Scenario D — Paste/drop image upload still works (User Story 4)

1. In the migrated lore editor, copy an image to your clipboard and paste
   it into the editor; confirm it uploads (in-progress indicator shown)
   and a markdown image reference is inserted at the cursor.
2. Drag an image file from your file system and drop it onto the editor;
   confirm the same upload-and-insert behavior.
3. Simulate an upload failure (e.g. disconnect network mid-upload);
   confirm a clear error message appears and the editor's existing text
   is unchanged.
4. Paste or drop a non-image file; confirm it is not intercepted (normal
   paste/drop behavior applies).
5. Save the entry after an image insert; view it; confirm the image
   renders inline (`world_lore_image_assets`-backed URL resolves).

**Expected outcome**: All 5 Acceptance Scenarios of User Story 4 pass.

## Verification commands (per Constitution Principle V)

```bash
# Type check
pnpm --filter web exec tsc --noEmit

# Rewritten e2e coverage (research.md R4)
pnpm --filter web exec playwright test lore-wiki
```
