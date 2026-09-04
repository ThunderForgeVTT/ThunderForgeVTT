/**
 * Renders a legal document's prose: paragraphs, emphasis, and links.
 *
 * # Why this is not a markdown library
 *
 * Because it renders four constructs, all of them from text this repository
 * authors. Legal prose is paragraphs; the only inline markup `legal/*.md` uses
 * is `**bold**` and `[text](url)`. Adding a parser to the bundle for that would
 * cost more than it explains.
 *
 * It is deliberately *not* the lore renderer either. That one takes HTML the
 * server produced with comrak and sanitized with ammonia, because lore is
 * whatever a player typed. This text is ours, compiled in from `legal/`, and
 * `legalDocuments.ts` says plainly that nothing outside that glob may be
 * rendered here.
 *
 * # What it does not support, on purpose
 *
 * Headings (the page owns those — each `##` becomes a card title), lists,
 * tables, images, code, and raw HTML. If a legal document ever needs one, add
 * it here deliberately rather than reaching for a library, and keep the
 * "trusted input only" invariant intact.
 */
import type { ReactNode } from "react";

/** `**bold**` and `[text](url)`, applied to one paragraph's text. */
function inline(text: string, keyPrefix: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern = /\*\*([^*]+)\*\*|\[([^\]]+)\]\(([^)]+)\)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let index = 0;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) {
      nodes.push(text.slice(lastIndex, match.index));
    }
    if (match[1] !== undefined) {
      nodes.push(
        <strong key={`${keyPrefix}-b${index}`} className="font-medium">
          {match[1]}
        </strong>,
      );
    } else {
      nodes.push(
        <a
          key={`${keyPrefix}-a${index}`}
          href={match[3]}
          className="underline underline-offset-2"
        >
          {match[2]}
        </a>,
      );
    }
    lastIndex = pattern.lastIndex;
    index += 1;
  }

  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
  }
  return nodes;
}

export interface LegalProseProps {
  /** One section's markdown body. Blank lines separate paragraphs. */
  body: string;
  className?: string;
}

export function LegalProse({
  body,
  className = "text-sm text-muted-foreground",
}: LegalProseProps) {
  const paragraphs = body
    .split(/\n\s*\n/)
    .map((p) => p.replace(/\s*\n\s*/g, " ").trim())
    .filter((p) => p.length > 0);

  return (
    <>
      {paragraphs.map((paragraph, i) => (
        <p key={i} className={className}>
          {inline(paragraph, `p${i}`)}
        </p>
      ))}
    </>
  );
}
