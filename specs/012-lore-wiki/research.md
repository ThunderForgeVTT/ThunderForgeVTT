# Phase 0 Research: World Lore Wiki

## 1. Markdown parser/renderer (server-side, GFM)

**Decision**: Use `comrak` on the server as the single source of rendering truth. The server parses+renders Markdown to sanitized HTML at save/view time (not just at display time in the client), and the frontend renders that server-produced HTML directly (with a client-side syntax-highlighter pass for code blocks, e.g. `shiki` or `highlight.js`, applied to the rendered HTML's `<pre><code>` blocks).

**Rationale**: `comrak` implements GitHub-Flavored Markdown (tables, task lists, strikethrough, autolinks) out of the box, matching FR-004 without hand-assembling `pulldown-cmark` extensions. Rendering once, server-side, guarantees the "linked from" backlink extraction (research §2) and the rendered view always agree on what a link resolved to — a client-side-only renderer would need to duplicate the link-resolution logic to decide autocomplete/broken-link state, doubling the surface for spec/render mismatches. Server-side rendering also lets the 25 MB content-size check (FR-010a) and the link-extraction pass share one parse.

**Alternatives considered**:
- `pulldown-cmark` + manual GFM extension wiring: more granular control, but requires manually enabling/testing each GFM extension (tables, tasklists, strikethrough) and doesn't reduce the link-resolution duplication problem. Rejected — no benefit over `comrak` for this feature's needs.
- Client-only rendering (`react-markdown` + `remark-gfm`), no server involvement: rejected — would require reimplementing `[[...]]` link resolution and broken-link detection in the browser using data already computed server-side, and gives no server-side hook for the 25 MB content check without a redundant parse.
- Not sanitizing server-rendered HTML: rejected outright — user-authored Markdown can contain raw HTML/script per CommonMark; comrak's `unsafe_` rendering flag must stay **off** (default), and any raw-HTML passthrough must go through an allowlist sanitizer (e.g. `ammonia`) before storage/display, since this is user-generated content rendered to other users.

## 2. In-text link syntax and resolution (`[[Entry Name]]` → lore entry or actor)

**Decision**: Author-facing syntax is `[[Title]]` (optionally `[[Title|Display Text]]`). At save time, the server extracts every `[[...]]` occurrence from the raw Markdown (regex/scan pass before/alongside comrak parsing), resolves each `Title` against both `world_lore_entries.title` and `world_actors.name` scoped to the current world (exact case-insensitive match), and persists one `world_lore_links` row per resolved-or-unresolved reference (with a `target_kind` of `lore_entry` / `actor` / `unresolved`). Rendering replaces each `[[...]]` span with a real `<a href>` (or a styled "broken link" span for unresolved) using the persisted resolution — not a live re-resolution on every view — so a title change elsewhere doesn't retroactively "unbreak" old content until the next save (consistent with FR-014's slug-update-on-title-change behavior and the immutable-revision model).

**Rationale**: Persisting resolution at save time (rather than resolving live on every read) keeps read paths cheap and keeps revision history meaningful — a past revision's rendered links reflect what existed *at that save time*, matching the "immutable snapshot" semantics already decided for FR-016. The `[[Title|Display Text]]` extension is a common wiki-link convention (used by Obsidian, Foundry VTT's own journal system, MediaWiki) and costs nothing extra to parse.

**Alternatives considered**:
- Live resolution on every render (no persisted `world_lore_links` rows, just parse-and-resolve at read time): rejected — makes "linked from" backlinks (FR-006) expensive to compute (would require scanning every entry's live content on every target view) and breaks the "past revision renders as it was" guarantee from FR-016/017.
- Markdown standard link syntax `[Title](lore:entry-slug)` instead of wiki-style `[[Title]]`: rejected — the spec's own wording and the "wiki" framing call for the lower-friction `[[...]]` convention; standard link syntax is still available for plain external URLs per FR-004, so authors get both.

## 3. Slug generation (title → urlified, collision-disambiguated)

**Decision**: Add the `slug` crate (ASCII-fold + lowercase + hyphenate) server-side. On create or title-change, compute the candidate slug, then check uniqueness within `(world_id)` scope; on collision, append `-2`, `-3`, etc. (first free numeric suffix). Store the slug in `world_lore_entries.slug` (indexed, unique per world) alongside the UUID primary key; the entry's canonical URL is `/world/:id/lore/:slug`, resolved server-side by `(world_id, slug)` lookup, never by UUID directly in user-facing URLs (matches FR-012).

**Rationale**: No existing slugify utility exists anywhere in the codebase (confirmed by search — only `game_systems.slug` is a manually-assigned package identifier, unrelated). `slug` is a small, dependency-light, well-maintained crate that handles Unicode-to-ASCII folding correctly (important for fantasy-name titles with accented characters), which a hand-rolled regex would get wrong for non-ASCII input.

**Alternatives considered**:
- Hand-rolled regex-based slugify (`title.to_lowercase().replace(non-alnum, "-")`): rejected — mishandles Unicode normalization (e.g. "Château" → wrong result without proper ASCII transliteration); not worth reinventing for a one-dependency fix.
- Slug stored as the primary key (no separate UUID): rejected — contradicts FR-011 (UUID-based on-disk/storage identifiers not derived from the human name) and would break FR-014's "slug changes, entry stays reachable" requirement, since primary keys shouldn't change.

## 4. Revision history storage

**Decision**: `world_lore_revisions` table: one row per save, `(id UUID, lore_entry_id, content_markdown TEXT, author_id, created_at, restored_from_revision_id NULLABLE)`. The entry's "current" content is always `content` on `world_lore_entries` itself (denormalized for cheap reads), kept in sync with the latest revision row on every save; restoring copies a prior revision's `content_markdown` into both a new revision row and the entry's current `content`. Markdown bodies stay in Postgres `TEXT` (not object storage) up to the 25 MB cap (FR-010a) — Postgres handles multi-MB `TEXT` values without issue at this scale, and keeping revisions in the same transactional store as the entry avoids a two-phase-commit-like consistency problem between DB and object storage on every single save.

**Rationale**: No existing append-only/versioned-content pattern exists in the codebase to mirror (confirmed by search — only a `schema_version` column on `world_events`, unrelated). Keeping revision bodies in Postgres (rather than one RustFS object per revision) is simpler and sufficient at the stated scale (25 MB/entry cap, no stated high-frequency-save requirement) and avoids adding a second storage round-trip to the hot "save a lore entry" path; RustFS remains reserved for binary image assets, consistent with ADR-039's "one asset storage mechanism" intent (which was scoped to images, not text).

**Alternatives considered**:
- One RustFS object per revision (text bodies in object storage, only metadata in Postgres): rejected for this scale — adds a network round-trip and a not-yet-needed consistency mechanism (DB row + object write must both succeed) for content that comfortably fits in a Postgres `TEXT` column; revisit only if entries were expected to be far larger or far more numerous than 25 MB/entry implies.
- No `restored_from_revision_id` link: rejected — losing the "this save was a restore, and of what" provenance would make FR-018's "new revision recording the restore" harder to distinguish from an ordinary edit in the history UI.

## 5. Image processing (thumbnail generation, extending existing transcode path)

**Decision**: Extend `storage/transcode.rs`'s existing `transcode_to_webp` (currently transcode-only, no resize) with a second output: decode once, produce (a) a normalized full-size WebP capped at a max dimension (e.g. 2048px longest edge, downscaled if larger, using the already-imported `image` crate's resize) and (b) a fixed-size thumbnail WebP (e.g. 256px longest edge). Both are written to RustFS under sibling UUID-based keys (e.g. `{asset_id}.webp` and `{asset_id}-thumb.webp`), reusing the existing per-object STS-scoped `write_object` path unchanged (two scoped credential mints per upload instead of one).

**Rationale**: The `image` crate (already a dependency, already used for decode+encode in the existing transcode path) includes resize functionality (`imageops::resize` / `DynamicImage::resize`), so no new image-processing dependency is needed — only new code in the module ADR-039 already designates as the single choke point for asset writes. This satisfies FR-009 (web-appropriate rendition + thumbnail) with minimal new surface.

**Alternatives considered**:
- External image-processing service (e.g. a dedicated thumbnailing microservice): rejected — massive overkill for this scale and contradicts ADR-039's single-service, self-hosted-RustFS-plus-server-transcode design; no functional requirement calls for it.
- Client-side thumbnail generation (browser canvas resize before upload): rejected — the full-size image still needs server-side processing/validation regardless (25 MB cap enforcement, FR-010, must be server-side to be trustworthy per Principle III), so client-side resize would be redundant work that doesn't remove the need for server-side thumbnailing.

## 6. Frontend Markdown editor UX (paste/drop images, `[[...]]` autocomplete)

**Decision**: Build `LoreMarkdownEditor.tsx` as a plain `<textarea>`-based editor (not a full WYSIWYG/rich-text framework) with: (a) a `paste`/`drop` event handler that intercepts image `DataTransfer` items, uploads them via the new `mutations_lore_images` GraphQL mutation, and inserts `![](url)` Markdown at the cursor position on success; (b) a lightweight `[[` trigger that opens an autocomplete popover (reusing the existing `@radix-ui/react-popover` dependency already in `apps/web/package.json`) querying lore entries + actors by title/name prefix, inserting the resolved `[[Title]]` text on selection. Rendering uses `LoreMarkdownRenderer.tsx`, which trusts and displays the server-rendered sanitized HTML from research §1 (no client-side Markdown parsing needed for display).

**Rationale**: No rich-text editor (tiptap/slate/quill/lexical) exists anywhere in `apps/web` today, and introducing one is a much larger dependency/UX surface than this feature's scope calls for — GitHub's own lore/issue editor is itself a plain textarea with paste-to-upload and `@`-style autocomplete, which is the exact interaction model the spec asks to match. Reusing the already-present `@radix-ui/react-popover` avoids a new UI-primitive dependency.

**Alternatives considered**:
- A full rich-text/WYSIWYG editor (tiptap, Lexical) with Markdown serialization: rejected as disproportionate — adds a large new dependency and a serialize/deserialize-to-Markdown layer this codebase has never needed, for a feature whose spec explicitly targets "GitHub markdown functionality" (i.e., a plain-text Markdown source format, not WYSIWYG).
- CodeMirror-based editor (syntax-highlighted Markdown source): plausible upgrade path, noted as a low-risk future enhancement, but not required by any FR/SC in the spec — deferred to keep this pass's new-dependency footprint minimal.
