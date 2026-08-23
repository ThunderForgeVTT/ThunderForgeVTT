import "@/pages/world/lore/LoreMarkdownRenderer.scss";
import { cn } from "@/lib/utils";

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
 * Styling lives in the co-located `LoreMarkdownRenderer.scss`, not
 * Tailwind Typography `prose` utilities — that plugin was never actually
 * installed in this project, so an earlier version of this component's
 * `prose-*` classes silently did nothing (caught during UX verification).
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
      className={cn("lore-markdown break-words", className)}
      data-testid="lore-markdown-rendered"
      // Safe: `html` is server-rendered via comrak with `unsafe_` off, piped
      // through `ammonia` for allowlist sanitization (research.md §1) — this
      // is not raw, unsanitized user input.
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
