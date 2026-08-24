# Contract: Markdown Editor Component Props

No network API changes — both editors keep calling the exact same
GraphQL operations they call today (`loreLinkTargets`, `uploadLoreImage`,
and each host page's own save mutation), unchanged. This document is the
closest analog to a contract for this feature: the public prop shape each
editor component exposes to its host page, which MUST stay stable across
the migration so `SessionNotesPanel.tsx` and `LoreEntryDetailPage.tsx`
need no changes beyond how they render the component.

## `MarkdownCodeEditor` (Session Notes)

```ts
export interface MarkdownCodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}
```

**Unchanged** from today. This feature only changes the `extensions`/
`basicSetup` passed to the underlying `<CodeMirror>` internally
(`lineNumbers: true, foldGutter: true`, replacing today's `false, false`)
— not this public prop shape.

## `LoreMarkdownEditor` (Lore entries)

```ts
export interface LoreMarkdownEditorProps {
  loreEntryId: string;
  worldId: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}
```

**Unchanged** from today — `LoreEntryDetailPage.tsx` continues passing
the same four required props plus `disabled`. Internally, `loreEntryId`
now feeds a CodeMirror `domEventHandlers.paste`/`.drop` extension
(instead of a React `onPaste`/`onDrop` handler) for image upload, and
`worldId` now feeds a CodeMirror `autocompletion()` completion source
(instead of a `useEffect` calling `getLoreLinkTargets`) for the `[[Title]]`
popover.

## Test-facing contract (`data-testid`s)

Existing automated coverage (`apps/web/e2e/lore-wiki.spec.ts`) and any
future coverage depend on these remaining stable:

| `data-testid` | Today | After migration |
|---|---|---|
| `lore-markdown-editor-textarea` | On the `<Textarea>` element itself | Moved to the `<CodeMirror>` component's outer root element (accepts arbitrary DOM props) — same testid, different underlying DOM shape (see research.md R4) |
| `lore-link-autocomplete` | On the Radix `PopoverContent` | Not applicable in the same form — `@codemirror/autocomplete`'s own popover has no equivalent host element to attach a custom testid to; tests instead assert against CodeMirror's own `.cm-tooltip-autocomplete` class (see quickstart.md) |
| `lore-link-target-{id}` | On each suggestion `<button>` | Not applicable — `@codemirror/autocomplete` renders its own completion list items; tests instead assert against `.cm-completionLabel` text content |

The middle two rows are a deliberate, unavoidable break from today's
exact DOM shape (R2's decision to use `@codemirror/autocomplete`'s own
popover rather than reimplementing positioning against a `Popover`) —
`apps/web/e2e/lore-wiki.spec.ts` MUST be updated accordingly, not left
targeting testids that no longer exist.
