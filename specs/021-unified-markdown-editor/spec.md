# Feature Specification: Unified Code-Editor Markdown Experience

**Feature Branch**: `021-unified-markdown-editor`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description: "Unify markdown editing across the app onto CodeMirror. Session Notes already uses CodeMirror but with line numbers and fold gutter turned off, so it doesn't read as a real code editor. Lore entries use a plain `<textarea>` instead, deliberately, with two custom features built directly on raw textarea cursor-position tracking: a `[[`-triggered autocomplete popover that resolves lore entry/actor titles and inserts `[[Title]]` references, and paste/drop image upload that intercepts image `DataTransfer` items, uploads them, and inserts markdown image syntax at the cursor. Goal: migrate the Lore editor onto the same real code-editor experience Session Notes uses, preserving both custom features. Also worth deciding whether Session Notes' editor should gain visible line numbers/fold gutter as part of this, now that 'a real browser code editor' is the explicit goal."

## Clarifications

### Session 2026-08-24

- Q: Does the Lore editor's drag-and-drop image upload need to actually work on touch/mobile devices, or is authoring treated as a desktop-only GM/author workflow? → A: Desktop-only authoring for this feature — drag-and-drop image upload only needs to work with a mouse; on touch devices, paste-from-clipboard (where supported) is enough, no touch-specific drag-and-drop implementation required. Mobile authoring support generally is wanted eventually, but is explicitly deferred past this feature's MVP, not ruled out long-term.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Session Notes reads as a real code editor (Priority: P1)

A GM writing session recap notes currently types into what looks like a
plain text box — the syntax highlighting exists under the hood, but with
no line numbers or fold affordance it doesn't read as a code editor at
all. The GM should see the same visual signals (line numbers, foldable
sections) they'd expect from any modern in-browser code editor.

**Why this priority**: Smallest independently-shippable slice — it's a
configuration change to an editor that's already wired up, not a
migration. Ships value on its own with the least risk.

**Independent Test**: Open a world's Session Setup page as GM, open the
session notes editor, and confirm line numbers and a fold gutter are
visible and functional (e.g., collapsing a heading's section) without
needing any other part of this feature to exist.

**Acceptance Scenarios**:

1. **Given** a GM viewing Session Setup, **When** they open the session
   notes field, **Then** each line is numbered and a fold gutter is
   visible alongside the text.
2. **Given** notes containing multiple markdown headings, **When** the GM
   clicks a fold control next to a heading, **Then** the content under
   that heading collapses/expands, matching standard code-editor folding
   behavior.
3. **Given** the same notes field, **When** the GM types markdown syntax
   (headings, bold, links, code spans), **Then** that syntax is visually
   highlighted as they type, exactly as it already is today.

---

### User Story 2 - Lore entries move onto the same real code editor (Priority: P1)

A lore author currently writes Lore entries in a plain text box with no
syntax highlighting at all — markdown structure (headings, emphasis,
links, code blocks) is invisible until they preview or save. They should
get the same syntax-highlighted, line-numbered editing experience Session
Notes already has.

**Why this priority**: This is the core of the request — without it,
Lore entries remain the one markdown surface still using a plain text
box, so unifying the two surfaces isn't actually done.

**Independent Test**: Open a lore entry in edit mode and confirm the
content area is the same code-editor component used by Session Notes —
line numbers, fold gutter, and live markdown syntax highlighting — with
existing entry content displayed unchanged and still editable/saveable.

**Acceptance Scenarios**:

1. **Given** an existing lore entry with markdown content, **When** a GM
   opens it in edit mode, **Then** its content appears unchanged, now
   rendered with line numbers, a fold gutter, and live syntax
   highlighting.
2. **Given** the lore editor open, **When** the author types new markdown
   content, **Then** cursor movement, text selection, undo, and redo all
   behave at least as well as they did in the previous plain-text-box
   editor.
3. **Given** a saved lore entry edited through the new editor, **When**
   it's reloaded or viewed by another user, **Then** its rendered output
   is identical to what the old editor would have produced for the same
   markdown source — no silent content transformation.

---

### User Story 3 - `[[Title]]` link autocomplete keeps working (Priority: P2)

A lore author typing `[[` to reference another lore entry or actor
currently gets a popover suggesting matching titles, and selecting one
inserts a resolved `[[Title]]` reference at the cursor. This must keep
working exactly as it does today once the editor underneath it changes.

**Why this priority**: A real, actively-used authoring feature — losing
it would be a regression, not just a missing nice-to-have. Depends on
User Story 2's editor migration existing first.

**Independent Test**: In the migrated lore editor, type `[[` followed by
a few characters of an existing lore entry's or actor's title, confirm a
matching popover of suggestions appears, and confirm selecting one
inserts the correct `[[Title]]` text at the right position.

**Acceptance Scenarios**:

1. **Given** the lore editor open, **When** the author types `[[` plus a
   partial title that matches an existing lore entry or actor,
   **Then** a popover lists matching titles.
2. **Given** that popover is showing, **When** the author selects a
   suggestion, **Then** the typed `[[partial` text is replaced with the
   full `[[Title]]` reference and the cursor lands immediately after it.
3. **Given** the author keeps typing past the `]]` or presses a key that
   isn't part of choosing a suggestion, **Then** the popover closes
   without altering the text further.
4. **Given** no lore entry or actor title matches what's been typed after
   `[[`, **Then** no popover appears (not an empty one).

---

### User Story 4 - Paste/drop image upload keeps working (Priority: P2)

A lore author can currently paste a copied image or drag-and-drop an
image file directly into the editor; it uploads automatically and a
markdown image reference is inserted at the cursor. This must keep
working exactly as it does today once the editor underneath it changes.

**Why this priority**: Same class as User Story 3 — an existing,
actively-used feature that must not regress. Depends on User Story 2's
editor migration existing first.

**Independent Test**: In the migrated lore editor, paste a copied image
and separately drag-and-drop an image file into the editor; confirm both
upload and insert a working markdown image reference at the cursor
position.

**Acceptance Scenarios**:

1. **Given** the lore editor open with the cursor at some position,
   **When** the author pastes a copied image, **Then** it uploads and a
   markdown image reference pointing at the uploaded asset is inserted at
   that cursor position.
2. **Given** the same editor, **When** the author drags an image file
   from their file system and drops it onto the editor, **Then** the same
   upload-and-insert behavior occurs.
3. **Given** an upload is in progress, **Then** the author sees a clear
   in-progress indicator, matching today's behavior.
4. **Given** an upload fails, **Then** the author sees a clear error
   message and the editor's text content is left unchanged.
5. **Given** a paste or drop that contains a non-image file, **Then** it
   is not intercepted — normal paste/drop behavior for text/other content
   is unaffected.

### Edge Cases

- What happens when a lore entry's existing content is extremely long
  (a large lore document)? The editor must remain responsive.
- What happens when the author pastes an image while the `[[` autocomplete
  popover is already open? The two features must not conflict with each
  other's cursor-position assumptions.
- What happens when the author drops multiple image files at once? At
  minimum this must not corrupt the editor's content or silently drop the
  upload without feedback.
- What happens when the network request for an autocomplete title lookup
  or an image upload is slow or fails mid-interaction? The author must get
  clear feedback in both editors, not a silently stuck UI.
- What happens on a narrow/mobile viewport? Both editors must remain
  readable and typable (line numbers, fold controls, `[[` autocomplete)
  at the smallest supported screen width already targeted elsewhere in
  the app — but drag-and-drop image upload is a desktop-only affordance
  and is not required to work via touch (Clarifications).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every markdown-authoring surface in the app (Session Notes,
  Lore entries) MUST present a syntax-highlighted, line-numbered
  code-editor experience — not a plain, unstyled text box.
- **FR-002**: Both editors MUST provide a working fold gutter that lets
  the author collapse and expand foldable sections of the document (at
  minimum, sections under a markdown heading).
- **FR-003**: The Lore entry editor MUST continue to support `[[`-triggered
  autocomplete that resolves matching lore entry and actor titles and
  inserts a `[[Title]]` reference at the cursor, matching today's behavior
  exactly (User Story 3's acceptance scenarios).
- **FR-004**: The Lore entry editor MUST continue to support pasting or
  dragging an image directly into the editor, uploading it, and inserting
  the resulting markdown image reference at the cursor, matching today's
  behavior exactly (User Story 4's acceptance scenarios), including
  in-progress and error feedback. This is a desktop (mouse-driven)
  affordance — touch/mobile drag-and-drop support is explicitly out of
  scope (Clarifications).
- **FR-005**: The migration MUST NOT change how any existing lore entry's
  stored markdown content renders — no automatic reformatting, no
  silent content transformation.
- **FR-006**: Both editors MUST support standard text-editing operations
  (cursor movement, selection, undo/redo, copy/paste of plain text) at
  least as well as the implementation they replace.
- **FR-007**: The `[[Title]]` autocomplete popover MUST remain reachable
  and dismissible without requiring a mouse, at least matching today's
  level of keyboard accessibility.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A GM or lore author can visually identify markdown structure
  (headings, emphasis, links, code spans) while typing, in either editor,
  without needing to open a separate preview.
- **SC-002**: Every capability of today's Lore editor (title-link
  autocomplete, paste/drop image upload) is still present and working
  after the migration — zero feature regressions found in a manual pass
  covering every acceptance scenario in User Stories 3 and 4.
- **SC-003**: Line numbers and a working fold gutter are visibly present
  in both the Session Notes editor and the Lore entry editor.
- **SC-004**: A sample of existing lore entries, compared before and after
  the migration, show byte-identical stored markdown content and
  identical rendered output.

## Assumptions

- Authoring (Session Notes, Lore entries) is treated as a desktop-oriented
  GM/author workflow for this feature; drag-and-drop image upload is not
  required to work on touch/mobile devices (Clarifications). General
  mobile authoring support is a known future goal, tracked separately —
  this feature's MVP explicitly does not attempt it, not a permanent
  decision to exclude mobile.
- Session Notes' editor is not being extended with Lore's `[[Title]]`
  autocomplete or image-paste-upload features as part of this change —
  those are Lore-specific (session notes have no comparable "link to
  another entry" concept today). Out of scope unless requested separately.
- The two editors continuing to be implemented as separate component
  instances (one per authoring surface) rather than merged into a single
  shared component is acceptable, as long as both present the same
  editing experience described above — this spec is about the experience
  being unified, not necessarily the code being deduplicated (that's a
  planning-level decision).
- No new data is persisted as part of this change — lore entries and
  session notes already store plain markdown text; this feature only
  changes how that text is authored, not how or where it's stored.
- Existing automated test coverage for the Lore editor's autocomplete and
  image-upload behavior is expected to need updating to target the new
  editor's DOM/interaction shape, since it currently targets a plain
  `<textarea>` (`data-testid="lore-markdown-editor-textarea"`).
