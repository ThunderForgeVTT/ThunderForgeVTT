# Phase 1 Data Model: Unified Code-Editor Markdown Experience

No new entities, columns, or migrations. This feature changes how
existing markdown text is *authored* in the browser — it does not change
what's stored or how.

## Existing data reused unchanged

| Entity | Where it lives | How this feature touches it |
|---|---|---|
| `worlds.session_notes` | Postgres column (`text`) | Read/written by `MarkdownCodeEditor.tsx` exactly as today — only the editor's own visual config (line numbers, fold gutter) changes, not the save path. |
| `world_lore_entries.content` | Postgres column (`text`) | Read/written by the migrated `LoreMarkdownEditor.tsx` exactly as today — same GraphQL save mutation, same stored markdown text (FR-005). |
| `world_lore_links` / `loreLinkTargets` query | Existing table/query (spec 012) | Read-only lookup, called with the same `(worldId, prefix)` arguments as today — only the *caller* (a CodeMirror `CompletionSource` instead of a `useEffect` + Radix `Popover`) changes. |
| `world_lore_image_assets` / `uploadLoreImage` mutation | Existing table/mutation (spec 012) | Called with the same `(loreEntryId, file)` arguments as today — only the *caller* (a CodeMirror `domEventHandlers.paste`/`.drop` handler instead of a React `onPaste`/`onDrop` handler) changes. |

## Validation rules

Unchanged — content validation (length limits, if any) already happens
server-side on save, independent of which client-side editor produced the
text. This feature introduces no new validation rules.
