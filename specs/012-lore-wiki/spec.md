# Feature Specification: World Lore Wiki

**Feature Branch**: `012-lore-wiki`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "I want you to plan lore similar how we did with actors but the idea with lore is its like a wiki inside our app allowing users to build and correlate. I want tobcover the full github markdown functionality and treat lore like a micro repo backed by s3 with image processing and copy and paste functionality all files on disk should be uuid based withour database housing the supplied name and urlified name for easy sharing"

## Clarifications

### Session 2026-08-22

- Q: Who can create and edit lore entries in a world? → A: DM-only creation, matching the actor precedent (spec 010) — the DM creates every lore entry; editing access beyond the DM is granted per-entry via the same Viewer/Editor/Owner ownership-block pattern already established for actors.
- Q: How do lore entries "correlate" with each other? → A: Wiki-style in-text linking — authors link to another entry directly from the markdown body (e.g., typing `[[Entry Name]]` or picking from an autocomplete), and the system automatically maintains a reverse "linked from" list on the target entry. No separate manual "related entries" picker is required.
- Q: Does "treat lore like a micro repo" mean full version history with revert, or just the storage architecture (each entry's assets live in their own S3 prefix)? → A: Full version history — every save creates a new immutable revision; users can view past revisions and restore an entry to any prior revision. "Micro repo" describes both the storage layout and the version-control behavior.
- Q: Should lore entries be able to correlate with actors (NPCs/PCs), not just with other lore entries? → A: Yes — in-text links can resolve to either a lore entry or an actor; actor pages gain their own "linked from (lore)" list.
- Q: Who is allowed to delete a lore entry? → A: Entry Owner-level — anyone holding Owner-level access on that specific entry can delete it, same as the DM, to support collaborative storytelling where a player-Owner can retire their own content.
- Q: What's the maximum file size for a single lore image upload? → A: 25 MB per image, and 25 MB for a single lore entry's Markdown content, as fixed defaults for this feature. Making these limits instance-admin-configurable (via a future instance portal with configurable storage quotas and related settings) is explicitly deferred as future work, not built in this pass.
- Q: When two people save conflicting edits to the same lore entry at nearly the same time, what should the second saver see happen? → A: Reject the second save outright with a conflict error; the second author must reload the latest content and re-apply their change manually (the user, not the system, resolves the merge).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - DM authors a lore entry with rich Markdown (Priority: P1)

A DM opens a world's lore section, creates a new lore entry, gives it a title, and writes its content using the full range of GitHub-flavored Markdown: headings, bold/italic/strikethrough, ordered and unordered lists, task lists, tables, blockquotes, fenced and syntax-highlighted code blocks, horizontal rules, and inline/autolinked URLs. The entry renders exactly as authored, matching familiar GitHub-style rendering.

**Why this priority**: This is the foundational authoring capability — without real Markdown authoring and rendering, there is no wiki.

**Independent Test**: As a DM, create a lore entry containing at least one of each supported Markdown construct (table, task list, code block, blockquote, heading levels, list, link); save it; confirm the rendered view matches GitHub's rendering of the same source.

**Acceptance Scenarios**:

1. **Given** a DM is in a world's lore section, **When** they create a new lore entry and provide a title, **Then** a new lore entry is created and appears in the world's lore index.
2. **Given** a DM is editing a lore entry's Markdown body, **When** they save content containing tables, task lists, code blocks, blockquotes, and headings, **Then** the rendered view displays each construct correctly formatted, matching standard GitHub-flavored Markdown rendering.
3. **Given** a DM is editing a lore entry, **When** they type a raw URL or an autolink, **Then** the rendered view displays it as a clickable link.

---

### User Story 2 - Authors correlate lore entries with each other and with actors via in-text links (Priority: P1)

While writing an entry, a DM links to another existing lore entry, or to an actor (NPC or PC), directly from the Markdown body (e.g., `[[Entry Name]]` or an autocomplete-assisted link that can resolve to either kind of target). The system resolves this into a working link to the target, and the target — whether a lore entry or an actor — automatically shows a "linked from" list of every lore entry that references it, without any separate manual linking step.

**Why this priority**: Correlation is the core "wiki" value proposition the requester called out by name, and extending it to actors ties lore into the rest of the world's content (NPCs, PCs) rather than leaving lore siloed from everything else already in the world.

**Independent Test**: As a DM, create two lore entries and one actor; from Entry A, link to Entry B and to the actor using the in-text link syntax; confirm both rendered links in Entry A navigate correctly, and that both Entry B's and the actor's detail views list Entry A under "linked from."

**Acceptance Scenarios**:

1. **Given** a DM is editing a lore entry's Markdown body, **When** they type the in-text link syntax and select an existing lore entry or actor (via autocomplete or exact-match resolution), **Then** the saved content renders that reference as a working link to the target.
2. **Given** a lore entry or actor is linked to from one or more lore entries, **When** a user views that target's detail page, **Then** they see a "linked from" list naming every lore entry that references it, kept in sync automatically as links are added or removed elsewhere.
3. **Given** a DM types an in-text link to a title that does not match any existing lore entry or actor, **When** the content is saved, **Then** the reference is shown distinctly as an unresolved/broken link rather than silently failing or crashing.
4. **Given** a DM removes an in-text link from Entry A's body and re-saves, **When** the previously-linked target's "linked from" list is viewed afterward, **Then** Entry A no longer appears in it.
5. **Given** an in-text link's title matches both an existing lore entry and an existing actor, **When** the author selects from the autocomplete, **Then** the two are presented as distinct, disambiguated choices so the author picks the intended target explicitly.

---

### User Story 3 - Paste and manage images inline (Priority: P2)

While editing a lore entry, a DM pastes an image directly from their clipboard (e.g., a screenshot) into the editor, or drags/uploads an image file. The system uploads the image, processes it (e.g., generates a web-friendly rendition and thumbnail), stores it durably, and inserts a working image reference into the Markdown body at the cursor position — matching the paste-to-upload convenience GitHub's own editor provides.

**Why this priority**: Image handling is explicitly called out as required and is central to making lore entries visually rich (maps, portraits, diagrams), but depends on entries and their storage existing first (User Story 1).

**Independent Test**: As a DM editing a lore entry, paste an image from the clipboard into the editor; confirm the image uploads, a processed rendition appears inline in the rendered preview, and reloading the entry still shows the image.

**Acceptance Scenarios**:

1. **Given** a DM is editing a lore entry, **When** they paste an image from their clipboard into the editor, **Then** the image is uploaded, processed, and a Markdown image reference to it is inserted at the cursor position automatically.
2. **Given** a DM drags and drops an image file onto the editor, **When** the drop completes, **Then** the same upload-and-insert behavior occurs as with paste.
3. **Given** a lore entry contains one or more pasted/uploaded images, **When** any user with view access opens the entry, **Then** the images render inline at a web-appropriate size, regardless of the original file's dimensions or format.
4. **Given** an oversized or unsupported image file is pasted or dropped, **When** the upload is attempted, **Then** the system rejects it with a clear message rather than silently failing or corrupting the entry.

---

### User Story 4 - Share a lore entry via a readable, human-friendly URL (Priority: P2)

Any world member with at least Viewer access to a lore entry can copy a link to it and share that link with others; the shared URL includes a human-readable, urlified version of the entry's title (e.g., `ancient-ruins-of-veldrath`) rather than an opaque identifier, making the link self-descriptive when shared in chat or notes.

**Why this priority**: Easy sharing was explicitly requested and is a natural companion to the entry-detail routes, but depends on entries existing (User Story 1) and is secondary to authoring/correlation.

**Independent Test**: As a world member with Viewer access to a lore entry, copy its link; confirm the URL contains a readable slug derived from the entry's title; open the link as a different user with at least Viewer access and confirm the correct entry loads.

**Acceptance Scenarios**:

1. **Given** a lore entry titled "Ancient Ruins of Veldrath" exists, **When** a user views its detail page, **Then** the page's URL includes a urlified slug derived from that title (e.g., `ancient-ruins-of-veldrath`), not just an opaque ID.
2. **Given** a lore entry's title is later changed, **When** the entry is viewed afterward, **Then** the URL's slug updates to reflect the new title, and the entry remains reachable (old links may redirect or fail gracefully rather than silently serving stale content).
3. **Given** two lore entries in the same world would urlify to the same slug, **When** the second entry is created or renamed, **Then** the system disambiguates its slug (e.g., with a numeric suffix) so both entries remain independently reachable.
4. **Given** a user without at least Viewer access to a lore entry opens its shared link, **When** the page loads, **Then** access is denied consistent with the entry's permission model.

---

### User Story 5 - View and restore prior revisions (Priority: P3)

A DM viewing a lore entry opens its revision history and sees a chronological list of every past save, each showing what changed. They can view any past revision's full rendered content and restore the entry to that revision, creating a new current revision equal to the restored one (never destructively overwriting history).

**Why this priority**: This delivers the "micro repo" version-control value, but is additive polish on top of a working authoring/correlation/sharing system, so it is sequenced last.

**Independent Test**: As a DM, edit and save a lore entry three times with different content each time; open its revision history; confirm all three (plus the original) are listed; restore an earlier revision; confirm the entry's current content matches that revision and a new revision recording the restore now exists.

**Acceptance Scenarios**:

1. **Given** a lore entry has been saved multiple times, **When** a DM opens its revision history, **Then** every past save is listed in chronological order with a timestamp and author.
2. **Given** a DM is viewing the revision history, **When** they open a specific past revision, **Then** they see that revision's full rendered content exactly as it was at that time.
3. **Given** a DM restores an entry to a prior revision, **When** the restore completes, **Then** the entry's current content matches the restored revision, and a new revision is appended recording the restore — no existing revision is deleted or overwritten.

---

### Edge Cases

- What happens when a world has zero lore entries yet? The lore index shows a genuine empty state, and creating the first entry is the way to populate it — no placeholder/lorem-ipsum content.
- What happens when a DM deletes a lore entry that other entries link to? Those in-text links become unresolved/broken (per User Story 2's broken-link handling), rather than the deletion being blocked or silently cascading.
- What happens when two DMs edit the same lore entry concurrently? The second save (the one whose expected prior revision no longer matches the entry's actual latest revision) is rejected outright with a conflict error; that author must reload the current content and manually re-apply their change — the system never silently overwrites or auto-merges either save.
- What happens when an uploaded image fails processing after upload (e.g., a corrupt file)? The user sees a clear failure state and the entry's content is not left referencing a broken/missing asset.
- What happens on a very small viewport? The lore index, entry detail, editor, and revision-history views must remain usable — no dedicated mobile layout is required, but nothing should become totally inaccessible.
- What happens when a non-member (or a member without any explicit ownership-block entry) tries to view a lore entry? They get default Viewer access, consistent with the actor permission model, unless the entry has been explicitly restricted.
- What happens when the in-text link syntax references an entry the linking user cannot actually view (e.g., a restricted entry)? The link resolves for users who can see the target and shows as inaccessible (not as content leakage) for users who cannot.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a world-scoped lore section listing all lore entries visible to the current user (its "lore index"), reachable from the world's staging screen's existing extension point.
- **FR-002**: Only the DM (a world member holding the Owner or GM role, per the precedent in spec 010) MUST be able to create a new lore entry.
- **FR-003**: Every lore entry MUST have an ownership block — a per-world-member record of Viewer/Editor/Owner permission level — using the same model, defaults (Viewer for members with no explicit entry), and DM-always-full-control rule established for actors in spec 010.
- **FR-004**: The system MUST provide a Markdown editor and renderer supporting the full GitHub-flavored Markdown feature set: headings, emphasis (bold/italic/strikethrough), ordered/unordered/task lists, tables, blockquotes, fenced code blocks with syntax highlighting, horizontal rules, inline code, and autolinked/inline URLs.
- **FR-005**: The system MUST support an in-text linking syntax that lets an author reference another lore entry OR an actor from within the Markdown body, resolved (via autocomplete or exact-title match) to a working link at save time.
- **FR-006**: Every lore entry and every actor MUST maintain an automatically-derived, always-current "linked from" list of every lore entry whose body currently contains a resolved in-text link to it.
- **FR-007**: An in-text link that does not resolve to any existing lore entry title or actor name MUST render as a distinct "unresolved/broken link" state rather than failing silently or crashing the render.
- **FR-007a**: When an in-text link's title matches both a lore entry and an actor, the authoring UI MUST present both as distinct, disambiguated candidates so the author explicitly picks the intended target.
- **FR-008**: The system MUST let a user with edit access paste an image from the clipboard, or drag-and-drop an image file, directly into the entry editor, automatically uploading it and inserting a Markdown image reference at the cursor position.
- **FR-009**: Every uploaded lore image MUST be processed into at least one web-appropriate rendition (and a thumbnail) regardless of the source file's original dimensions or format, before being referenced in rendered content.
- **FR-010**: The system MUST reject an image upload larger than 25 MB, or of an unsupported format, with a clear error rather than silently failing or leaving the entry referencing a broken asset.
- **FR-010a**: The system MUST reject saving a lore entry whose Markdown content exceeds 25 MB, with a clear error, rather than silently truncating or failing.
- **FR-011**: Every file associated with a lore entry (its Markdown content object and every image asset) MUST be stored on disk/object storage under a UUID-based identifier; the UUID MUST NOT be derived from or expose the entry's or file's human-supplied name.
- **FR-012**: The database MUST store, per lore entry, both the human-supplied title and a urlified slug derived from it, and MUST use the slug (not the UUID) in the entry's shareable/viewable URL.
- **FR-013**: When two lore entries in the same world would urlify to the same slug, the system MUST disambiguate the later one's slug automatically (e.g., numeric suffix) so both remain independently reachable by URL.
- **FR-014**: When a lore entry's title changes, the system MUST update its slug to match the new title while keeping the entry reachable (old slug may redirect or 404-gracefully, but MUST NOT serve a different entry's content).
- **FR-015**: The system MUST deny access to a lore entry's detail route for any user without at least Viewer access under its ownership block, consistent with the actor permission model.
- **FR-016**: Every save of a lore entry's content MUST create a new immutable revision; no prior revision MUST ever be deleted or overwritten by a subsequent save.
- **FR-017**: The system MUST let a user with at least Viewer access to a lore entry view its full revision history (timestamp and author per revision) and open the full rendered content of any past revision.
- **FR-018**: The system MUST let a user with Editor or Owner access to a lore entry restore it to any prior revision, which MUST append a new current revision matching the restored content rather than deleting intervening history.
- **FR-019**: The system MUST prevent silent data loss on concurrent edits to the same lore entry: a save that no longer matches the entry's latest known revision MUST be rejected outright with a clear conflict error, rather than overwritten or silently merged — the rejected author is responsible for reloading the current content and manually re-applying their change.
- **FR-020**: Deleting a lore entry MUST NOT be blocked by the existence of other entries' in-text links to it; those links MUST subsequently render as unresolved/broken (per FR-007).
- **FR-021**: Any world member holding Owner-level access on a specific lore entry (including the DM's always-implicit access) MUST be able to delete that entry; a member with only Viewer or Editor access on that entry MUST NOT be able to delete it.

### Key Entities *(include if feature involves data)*

- **Lore Entry**: A world-scoped wiki page with a human-supplied title, a system-derived urlified slug (unique within the world, disambiguated on collision), a Markdown content body, an ownership block (Viewer/Editor/Owner per world member, same model as Actor), a current revision, and a history of prior revisions.
- **Lore Revision**: An immutable snapshot of a lore entry's Markdown content at a point in save time, with an author and timestamp. Entries retain every revision indefinitely; restoring an old revision appends a new current revision rather than deleting history.
- **Lore Link**: A directional, in-text reference from one lore entry's content to another lore entry or to an actor, extracted from the Markdown body's link syntax. Feeds the target's (lore entry's or actor's) automatically-maintained "linked from" list. May be unresolved if the referenced title/name doesn't match any lore entry or actor.
- **Lore Image Asset**: An uploaded/pasted image attached to a lore entry, stored under a UUID-based object key with at least one processed (resized/normalized) rendition and a thumbnail, referenced from the entry's Markdown body by a stable URL.
- **World Member / World** (existing, reused from spec 010): Supplies the pool of assignable subjects for a lore entry's ownership block and the "DM" authorization concept (Owner or GM role).
- **Actor** (existing, reused from spec 010): Valid in-text link target alongside lore entries; gains its own automatically-maintained "linked from" list of lore entries that reference it, but no other change to the Actor entity itself.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A DM can author a lore entry using every supported GitHub-flavored Markdown construct (tables, task lists, code blocks, etc.) and see it render correctly on first save, with zero constructs silently dropped or mis-rendered.
- **SC-002**: A user can navigate from any lore entry to another correlated entry via an in-text link in one click, and see that correlation reflected on the target entry's "linked from" list without any separate manual linking action.
- **SC-003**: A user can paste a clipboard image into the editor and see it appear, fully processed, in the rendered entry in under 10 seconds for a typical image.
- **SC-004**: 100% of lore-entry URLs shared between users contain a human-readable slug rather than an opaque identifier, and correctly resolve to the intended entry even after the entry's title has since changed.
- **SC-005**: 100% of attempts to view or edit a lore entry by a user without sufficient ownership-block access are blocked, matching the enforcement rate already established for actors.
- **SC-006**: A DM can restore a lore entry to any of its prior revisions in 3 actions or fewer (open history, select revision, confirm restore), with zero loss of the revision history itself.
- **SC-007**: Zero lore entry files or image assets are ever exposed to end users under a human-readable filename — 100% of on-disk/object-storage identifiers are UUID-based.
- **SC-008**: 100% of image uploads over 25 MB and 100% of Markdown saves over 25 MB are rejected with a clear error, with zero silent truncation or corrupted entries.

## Assumptions

- Lore entries are world-scoped only (not scene-scoped); this feature does not introduce a separate lore concept nested under individual scenes.
- The ownership-block/permission model, "DM" authorization definition (Owner or GM role), and default-Viewer-access rule are reused verbatim from the actor system (spec 010) rather than redesigned for lore; any future divergence between the two is out of scope here.
- "Full GitHub markdown functionality" is interpreted as GitHub-flavored Markdown (GFM): tables, task lists, strikethrough, autolinks, fenced code blocks with syntax highlighting, and standard CommonMark constructs — not GitHub-specific non-Markdown features like issue/PR references, emoji shortcodes, or `@mention` autolinking to GitHub accounts (there is no GitHub account concept in this app).
- "Micro repo backed by S3" is interpreted as: each lore entry's content and image assets live under their own storage prefix/namespace, with every content save retained as an immutable revision (per Clarifications) — not a literal Git repository or Git-compatible interface (no git clone/push/pull semantics are implied).
- Image processing is assumed to mean generating at least one normalized web-friendly rendition and a thumbnail (dimensions/format/compression specifics are an implementation decision), not arbitrary image editing (crop/filter tools) within the app.
- The actor system's existing share-link-and-copy-to-another-world mechanism (spec 010, User Story 5) is explicitly called out there as a pattern intended to generalize to other content types, with lore named as a candidate; this spec does not build that generalized cross-world share/copy mechanism for lore — only in-app viewing, correlation, and permissioned access within a single world. A follow-up spec may extend cross-world lore sharing/copying once this in-world version has shipped.
- Lore entries do not have a PC/NPC-style classification; all lore entries share one kind, differentiated only by their content and ownership-block permissions.
- Concurrent-edit conflict handling (FR-019, resolved via Clarifications) is satisfied by a simple "last-writer-detects-conflict" check — the save request carries the revision it was edited against, compared to the entry's actual latest revision at save time — rather than requiring real-time collaborative editing/merging, which is out of scope.
- **Follow-up (out of scope for this spec)**: The 25 MB per-image and 25 MB per-entry-content limits (FR-010, FR-010a) are fixed defaults for this pass. A future instance-admin portal with configurable storage quotas (and likely other instance-level settings) is expected to make these limits adjustable per deployment; building that portal is explicitly out of scope here.
