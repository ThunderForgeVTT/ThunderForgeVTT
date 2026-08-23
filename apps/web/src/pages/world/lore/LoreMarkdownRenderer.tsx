/**
 * Spec 012 (T027, T035): renders the server-produced, sanitized GFM HTML
 * (`renderedHtml` from `markdown::mod`, research.md §1) directly — the
 * server is the single source of rendering truth, so the client never
 * re-parses Markdown. Broken/unresolved `[[...]]` links are already
 * marked by the server (`markdown/links.rs`) as
 * `<span class="lore-link-broken" title="Unresolved link">`; this
 * component just styles whatever the server emits — it does not itself
 * decide resolution.
 *
 * No client-side syntax-highlighter dependency is introduced here
 * (research.md §1 mentions shiki/highlight.js as an example, not a
 * requirement); code blocks render with comrak's own `<pre><code
 * class="language-...">` output styled via plain CSS, consistent with
 * this codebase not yet depending on a highlighter library.
 */
export interface LoreMarkdownRendererProps {
  html: string;
  className?: string;
}

export function LoreMarkdownRenderer({ html, className }: LoreMarkdownRendererProps) {
  return (
    <div
      className={
        "lore-markdown prose prose-sm max-w-none break-words " +
        "prose-headings:font-semibold prose-a:text-primary prose-a:no-underline hover:prose-a:underline " +
        "prose-code:rounded prose-code:bg-muted prose-code:px-1 prose-code:py-0.5 prose-code:text-xs " +
        "prose-pre:rounded-lg prose-pre:bg-muted prose-pre:p-3 prose-table:text-sm " +
        "[&_.lore-link-broken]:cursor-help [&_.lore-link-broken]:rounded [&_.lore-link-broken]:border " +
        "[&_.lore-link-broken]:border-dashed [&_.lore-link-broken]:border-destructive/50 [&_.lore-link-broken]:px-1 " +
        "[&_.lore-link-broken]:text-destructive [&_.lore-link-broken]:no-underline " +
        (className ?? "")
      }
      data-testid="lore-markdown-rendered"
      // Safe: `html` is server-rendered via comrak with `unsafe_` off, piped
      // through `ammonia` for allowlist sanitization (research.md §1) — this
      // is not raw, unsanitized user input.
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
