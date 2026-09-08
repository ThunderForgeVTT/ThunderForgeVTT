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
 *
 * # Operator values are rendered here and parsed nowhere
 *
 * Spec 040 substitutes a closed set of tokens — the operator's name, their
 * contact — from settings an operator typed. That text is *not* trusted the
 * way the surrounding prose is, so `substituteOperatorValues` hands back
 * alternating segments and only the prose ones reach `inline()`.
 *
 * The tempting shortcut was to substitute into the paragraph string and let
 * the existing parser run over the result. That was tried and rejected: an
 * operator whose display name is `[click here](https://elsewhere.example)`
 * would then have published a link inside the terms of service, which is
 * precisely the trust boundary `legalDocuments.ts` documents. Do not "fix" a
 * value that renders with visible markup by teaching `inline()` about it.
 */
import { Fragment, type ReactNode } from "react";
import {
  substituteOperatorValues,
  type OperatorValues,
} from "@/legal/operatorTokens";
import { usePublishedOperatorValues } from "@/legal/publishedOperatorValues";

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
  /**
   * Resolved operator settings. Omitted, the component reads them itself, so a
   * page rendering legal text names the operator without having to remember
   * to; pass them to render a known state, as a test does.
   */
  values?: OperatorValues;
}

export function LegalProse({
  body,
  className = "text-sm text-muted-foreground",
  values,
}: LegalProseProps) {
  const fetched = usePublishedOperatorValues();
  const resolved = values ?? fetched;

  // Paragraphs are split *before* substitution, on the repository's own text.
  // A value containing a blank line must not be able to start a new paragraph
  // in a document a lawyer signed off — it is a name, not layout.
  const paragraphs = body
    .split(/\n\s*\n/)
    .map((p) => p.replace(/\s*\n\s*/g, " ").trim())
    .filter((p) => p.length > 0);

  return (
    <>
      {paragraphs.map((paragraph, i) => (
        <p key={i} className={className}>
          {substituteOperatorValues(paragraph, resolved).segments.map(
            (segment, s) =>
              segment.kind === "prose" ? (
                // Keyed fragment: `inline` returns an array, and it is one
                // entry of this list.
                <Fragment key={`p${i}-s${s}`}>
                  {inline(segment.text, `p${i}-s${s}`)}
                </Fragment>
              ) : (
                // Deliberately not `inline()`. See the note at the top of the
                // file: an operator's text is printed, never parsed.
                <span key={`p${i}-v${s}`} data-operator-value="">
                  {segment.text}
                </span>
              ),
          )}
        </p>
      ))}
    </>
  );
}
