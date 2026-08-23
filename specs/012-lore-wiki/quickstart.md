# Quickstart: Validating the World Lore Wiki

Prerequisites: local dev stack running (`docker compose up` for Postgres/RustFS, server on its configured port, `apps/web` dev server running), at least two user accounts in the same world (one DM/Owner, one Player), and one existing actor in that world (for correlation testing).

## US1 — DM authors a lore entry with rich Markdown

1. As the world's DM, open `/world/<id>/compendium`, select the "Lore" tab, and create a new lore entry with a title (e.g. "Ancient Ruins of Veldrath").
2. In the entry's editor, write content containing at least one table, one task list, one fenced code block, one blockquote, headings at two levels, and a raw URL.
3. Save. **Expect**: the rendered view shows each construct correctly formatted (see contracts/lore-crud.md's `renderedHtml`), matching standard GFM rendering; the raw URL renders as a clickable link.

## US2 — Correlate lore entries with each other and with actors

1. As the DM, create a second lore entry ("Entry B"). In the first entry ("Entry A")'s body, type `[[Entry B]]` and confirm the autocomplete resolves it.
2. In Entry A's body, also type `[[` followed by the existing actor's name and confirm the autocomplete offers it as a distinct option from any same-titled lore entry (if one exists).
3. Save Entry A. **Expect**: both references render as working links.
4. Open Entry B's detail page. **Expect**: Entry A appears in its "linked from" list. Open the actor's detail page. **Expect**: Entry A appears in its "linked from (lore)" list.
5. Type `[[Nonexistent Title]]` in Entry A and save. **Expect**: renders as a distinct unresolved/broken-link state, not a crash or silent drop.
6. Remove the `[[Entry B]]` link from Entry A and re-save. **Expect**: Entry A no longer appears in Entry B's "linked from" list.

## US3 — Paste and manage images inline

1. As the DM, editing any lore entry, copy an image to the clipboard (e.g. a screenshot) and paste it into the editor.
2. **Expect**: the image uploads, a Markdown image reference is inserted at the cursor automatically, and the rendered preview shows the processed image within ~10 seconds (SC-003).
3. Reload the entry. **Expect**: the image still renders (durable storage, RustFS-backed per contracts/lore-images.md).
4. Attempt to paste/drop a file larger than 25 MB, or an unsupported format. **Expect**: a clear rejection error; the entry's content is not left referencing a broken asset.

## US4 — Share a lore entry via a readable URL

1. As any world member with at least Viewer access, open a lore entry's detail page. **Expect**: the URL is `/world/<id>/lore/<slug>` where `<slug>` is a urlified version of the title (e.g. `ancient-ruins-of-veldrath`), not the entry's UUID.
2. As the DM, rename the entry's title. **Expect**: the URL's slug updates to match the new title on next view; the entry remains reachable at its new URL.
3. Create a second entry whose title urlifies to the same slug as an existing one. **Expect**: its slug is automatically disambiguated (e.g. `-2` suffix) so both remain independently reachable.
4. As a user without at least Viewer access to the entry (e.g. a non-member of the world), attempt to open its URL directly. **Expect**: access denied, consistent with the entry's permission model.

## US5 — View and restore prior revisions

1. As the DM, edit and save a lore entry's content three separate times with different content each time.
2. Open its revision history. **Expect**: all three saves (plus the original) are listed chronologically with timestamp and author.
3. Open an earlier revision. **Expect**: its full rendered content displays exactly as it was at that time.
4. Restore that earlier revision. **Expect**: the entry's current content now matches the restored revision, and a new revision (marked as a restore) is appended — no existing revision disappears from the history list.

## Cross-cutting checks

- **Deletion permission**: as a Player who has been granted entry-level Owner on a lore entry (not the DM), delete that entry. **Expect**: the deletion succeeds (FR-021 — entry Owner, not DM-only). As a Player with only Editor access on a different entry, attempt to delete it. **Expect**: denied.
- **Ownership block DM-only**: as the same Player who holds entry-level Owner on their own lore entry, attempt to open/change its ownership block. **Expect**: no controls shown/editable — only the DM can (mirrors spec 010's actor precedent exactly).
- **Upload size limits**: attempt to save a lore entry with Markdown content exceeding 25 MB. **Expect**: rejected with a clear error, no truncation (FR-010a, SC-008).
