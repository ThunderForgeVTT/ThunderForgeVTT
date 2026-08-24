# Phase 0 Research: Unified Code-Editor Markdown Experience

## R1 — Does `@codemirror/lang-markdown` actually support fold-by-heading?

**Decision**: Yes — use `foldGutter()` (from `@codemirror/language`, already a
transitive dependency) alongside the existing `markdown()` language
extension. No custom fold logic needed.

**Rationale**: Inspected `@codemirror/lang-markdown`'s own source
(`node_modules/.pnpm/@codemirror+lang-markdown@6.5.2/.../dist/index.js`):
it registers a `foldService.of(...)` specifically for header-based
folding (`headerIndent`), plus `foldNodeProp` entries for blockquotes and
lists. This was the one real feasibility risk identified before planning
(a code-language fold gutter with no meaningful folding for markdown
would have been a weak/misleading UI) — it's resolved: heading-based
folding is real, shipped, first-party behavior of the package already in
this codebase, not something to build.

**Alternatives considered**: A custom `foldNodeProp` extension targeting
markdown heading syntax nodes directly — unnecessary now that the
language package's own fold service already does this.

## R2 — How to rebuild `[[Title]]` autocomplete on CodeMirror

**Decision**: `@codemirror/autocomplete`'s `autocompletion()` extension
with a custom `CompletionSource` function, matching CodeMirror's standard
idiomatic pattern for editor-integrated autocomplete.

**Rationale**: `CompletionContext.matchBefore(/\[\[[^\]]*/)` detects the
same "`[[` plus everything since, up to a `]`" trigger condition
`LoreMarkdownEditor.tsx`'s current regex (`/\[\[([^[\]]*)$/`) already
implements by hand against `textarea.value.slice(0, caret)`. The
completion source can be `async`, so the existing `getLoreLinkTargets`
GraphQL call plugs in directly — return one `Completion` per matching
lore entry/actor, each with an `apply` function that performs the same
replace-the-`[[partial`-with-`[[Title]]` insertion
`LoreMarkdownEditor.tsx`'s `replaceRange` does today. `@codemirror/autocomplete`
renders its own popover (positioned relative to the cursor
automatically) and already handles keyboard navigation/dismissal
(arrow keys, Enter, Escape) — this directly satisfies FR-007 (keyboard
reachability) essentially for free, an improvement over today's
mouse-oriented `Popover`/`onClick` implementation, not just parity.

**Alternatives considered**: Keep the existing Radix `Popover` positioned
manually against CodeMirror's cursor coordinates (`view.coordsAtPos`) —
rejected as needless extra work reimplementing positioning/keyboard-nav
that `@codemirror/autocomplete` already provides natively.

## R3 — How to rebuild paste/drop image upload on CodeMirror

**Decision**: `EditorView.domEventHandlers({ paste, drop })`, an
extension provided directly on the `CodeMirror` component via its
`extensions` prop (same mechanism `markdown()` and `autocompletion()`
are already passed through).

**Rationale**: These handlers receive the raw DOM `ClipboardEvent`/
`DragEvent` exactly as `LoreMarkdownEditor.tsx`'s current `onPaste`/
`onDrop` textarea handlers do today — the existing image-detection logic
(`Array.from(event.clipboardData.items)`, `.find(item.type.startsWith("image/"))`)
carries over unchanged. Insertion uses
`view.dispatch({ changes: { from: view.state.selection.main.head, insert: text } })`
instead of the textarea's `setSelectionRange` dance — CodeMirror's
transaction model additionally makes this atomic and undo-stack-aware
for free (today's textarea version's cursor-restore already works, but
CodeMirror's version doesn't need `requestAnimationFrame` timing tricks
to do it).

**Alternatives considered**: A separate hidden file-input trigger instead
of raw event interception — rejected; changes user-facing behavior
(spec.md FR-004 requires today's exact paste/drop behavior, not a new
interaction model), and the domEventHandlers approach already achieves
full parity.

## R4 — `apps/web/e2e/lore-wiki.spec.ts` rewrite scope

**Decision**: Every interaction currently using `Locator.fill()` or
`Locator.inputValue()` against `[data-testid="lore-markdown-editor-textarea"]`
must be rewritten — those Playwright APIs only work against a real
`<textarea>`/`<input>` element; CodeMirror 6 renders a `contenteditable`
DOM with no such element to target.

**Rationale**: Confirmed by reading the current spec file directly —
`createLoreEntry` (helper, `.fill(markdown)`), the GFM-rendering test,
both link-autocomplete tests (`.fill()` to seed content), and the
paste-image test's assertion (`.inputValue()` to read back the inserted
markdown) all depend on textarea-only APIs. The paste-image test's own
`pasteImageIntoEditor` helper already dispatches a synthetic
`ClipboardEvent` via `.evaluate()` against the element found by that
testid — this keeps working as long as the same `data-testid` is present
on CodeMirror's outer container (`@uiw/react-codemirror` accepts
arbitrary DOM props on its root element), since dispatching a `paste`
event on an ancestor of the actual focused editable element still
reaches R3's `domEventHandlers` (DOM events bubble). Replacement pattern:
`page.locator('[data-testid="..."] .cm-content').click()` to focus, then
`page.keyboard.type(...)` to enter text (CodeMirror has no scriptable
"set value" Playwright API equivalent to `.fill()` — typing is the
standard way tests drive CodeMirror instances in the broader ecosystem),
and read back content via `.textContent()` on `.cm-content` (CodeMirror's
own content DOM class name) instead of `.inputValue()`.

**Alternatives considered**: Keeping a hidden real `<textarea>` in sync
with CodeMirror purely so `.fill()`/`.inputValue()` keep working —
rejected; adds a permanent synchronization-bug surface to production code
just to avoid updating test helpers, the wrong trade.

## R5 — Dependency manifest

**Decision**: Add `@codemirror/autocomplete` and `@codemirror/view` as
direct `apps/web/package.json` dependencies (pinned to the versions
already resolved transitively per `pnpm-lock.yaml`: `@codemirror/autocomplete@6.20.3`).

**Rationale**: Both are already installed transitively (pulled in by
`@uiw/react-codemirror`'s own dependencies) and importable today with no
lockfile change — but importing a package apps/web doesn't declare
itself is exactly the kind of implicit-transitive-dependency risk that
breaks on an unrelated upstream version bump. Declaring them directly
costs nothing (already installed, no version to actually change) and
removes that risk.

**Alternatives considered**: Leave them undeclared, relying on the
transitive install — rejected for the reason above.

## Outcome

All Technical Context unknowns are resolved; no `NEEDS CLARIFICATION`
markers remain. Proceeding to Phase 1.
