/**
 * The closed set of operator values that may be substituted into legal prose.
 *
 * # Why this is a closed set and not a template engine
 *
 * `legalDocuments.ts` states the invariant this file sits next to: the text in
 * `legal/*.md` is **ours**, compiled in by Vite at build time, which is the
 * only reason it may be rendered without the server's sanitizing markdown
 * pipeline. Substitution punches one hole in that: a handful of values an
 * operator typed into setup are rendered inside prose a lawyer signed off.
 *
 * The hole is kept the size of the values. Six named tokens, declared here,
 * tested here. A `{{...}}` that is not in this table is left in the page
 * verbatim — exactly as an `[OPERATOR — ...]` marker is — rather than being
 * resolved, blanked, or treated as an error. There is no general-purpose
 * templating over legal text, and adding one would need the ADR that spec
 * 040's `contracts/legal-rendering.md` describes rather than a helper.
 *
 * # Why substitution returns segments rather than a string
 *
 * Because the two halves have different trust. The prose is ours and may go
 * through `LegalProse`'s inline `**bold**` / `[text](url)` matcher; the value
 * is operator input and must not. An operator whose display name is
 * `[click here](https://elsewhere.example)` must see that name printed in the
 * terms of service, not a link inside them.
 *
 * A `string` return could not express that — whoever rendered it would have to
 * choose "parse it all" or "parse none of it", and both are wrong. So this
 * hands back alternating segments and the renderer parses only the prose ones.
 * That is the whole reason the type exists; see `LegalProse.tsx`.
 *
 * # Why an unset value renders a marker rather than nothing
 *
 * `legalDocuments.test.ts` has asserted since the documents were added that
 * `[OPERATOR — ...]` markers survive rendering: "a page that omits who holds
 * your data while reading as complete is worse than one that visibly has a
 * blank." Substituting an empty string for an unconfigured instance would
 * delete that property silently, on the pages where it matters most. So an
 * unset token renders the marker the markdown used to carry, as prose.
 */

/**
 * The only tokens that may appear in `legal/*.md`. Closed, declared, tested.
 *
 * Keys are the literal text in the markdown; values are the setting each one
 * resolves from — the same six the unauthenticated `publishedOperatorValues`
 * query exposes (spec 039 FR-056: somebody who needs to file a notice has no
 * account here).
 *
 * `operator.jurisdiction` and the `notice.*` three are declared but appear in
 * no document today: the governing-law marker in `terms-of-service.md` is
 * *prose* an operator has to write (research.md § D2), and the notice contact
 * is rendered by `DmcaCompliancePage.tsx` as a definition list rather than
 * inside prose. They are here because the contract's token set is the
 * settings' set, and because a document that starts naming the agent should
 * find the token already resolving.
 */
export const OPERATOR_TOKENS = {
  "{{operator.name}}": "operator.name",
  "{{operator.contact_email}}": "operator.contact_email",
  "{{operator.jurisdiction}}": "operator.jurisdiction",
  "{{notice.contact_name}}": "notice.contact_name",
  "{{notice.contact_email}}": "notice.contact_email",
  "{{notice.contact_postal_address}}": "notice.contact_postal_address",
} as const;

/** The setting a token resolves from. */
export type OperatorValueKey =
  (typeof OPERATOR_TOKENS)[keyof typeof OPERATOR_TOKENS];

/** Resolved settings. A value is absent, null or blank while unset. */
export type OperatorValues = Partial<
  Record<OperatorValueKey, string | null | undefined>
>;

/**
 * What a token renders while its setting is unset.
 *
 * These are the markers the markdown carried before the tokens replaced them,
 * word for word, so an unconfigured instance publishes the same page it
 * published before this feature. Changing one changes what an unconfigured
 * instance says, which is a review-surface decision — keep it in step with
 * `legal/README.md`.
 */
export const OPERATOR_UNSET_MARKERS: Record<OperatorValueKey, string> = {
  "operator.name": "[OPERATOR — name and, if applicable, legal entity]",
  "operator.contact_email": "[OPERATOR — email address]",
  "operator.jurisdiction": "[OPERATOR — the jurisdiction whose law governs]",
  // The DMCA agent designation's own pre-launch placeholder, kept identical:
  // it is the text `DmcaCompliancePage.tsx` has always shown, and spec 039
  // FR-055 makes registering an agent the operator's obligation rather than
  // something this software can complete for them.
  "notice.contact_name":
    "[Configure via instance legal/compliance settings before launch]",
  "notice.contact_email":
    "[Configure via instance legal/compliance settings before launch]",
  "notice.contact_postal_address":
    "[Configure via instance legal/compliance settings before launch]",
};

/** One run of text, and whether it came from the repository or an operator. */
export type LegalSegment =
  /** Text this repository authored. Safe for `LegalProse`'s inline parser. */
  | { kind: "prose"; text: string }
  /** A value an operator typed. Rendered as text, never parsed. */
  | { kind: "value"; text: string };

export interface Substituted {
  /** Segments alternating literal prose and substituted values. */
  segments: LegalSegment[];
}

/** Matches any `{{...}}`, in or out of the closed set — see `resolve` below. */
const TOKEN_PATTERN = /\{\{[a-z_.]+\}\}/g;

function isKnownToken(token: string): token is keyof typeof OPERATOR_TOKENS {
  return Object.prototype.hasOwnProperty.call(OPERATOR_TOKENS, token);
}

/**
 * Split legal text into prose and substituted-value segments.
 *
 * Three cases, and each one is a rule from the contract:
 *
 * - **Known token, value set** → a `value` segment carrying what the operator
 *   typed. Never parsed by the caller.
 * - **Known token, value unset** → a `prose` segment carrying the visible
 *   `[OPERATOR — ...]` marker. Prose, because the marker is our text and
 *   because that keeps an unconfigured page identical to what it was.
 * - **Unknown token** → left verbatim, as prose. Not an error and not a blank:
 *   a typo in a legal document should be visible in review, not silently
 *   render nothing where a name belongs.
 *
 * The input must be trusted repository text. This does not sanitize; it draws
 * the line between text that may be parsed and text that may not.
 */
export function substituteOperatorValues(
  text: string,
  values: OperatorValues = {},
): Substituted {
  const segments: LegalSegment[] = [];
  let lastIndex = 0;

  const pushProse = (chunk: string) => {
    if (chunk.length === 0) {
      return;
    }
    const previous = segments[segments.length - 1];
    if (previous && previous.kind === "prose") {
      // Merge, so an unset token in the middle of a sentence leaves the
      // sentence as one string for the inline parser. Splitting it would break
      // a `**bold**` that spans the token.
      previous.text += chunk;
      return;
    }
    segments.push({ kind: "prose", text: chunk });
  };

  TOKEN_PATTERN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = TOKEN_PATTERN.exec(text)) !== null) {
    pushProse(text.slice(lastIndex, match.index));
    const token = match[0];

    if (!isKnownToken(token)) {
      pushProse(token);
    } else {
      const key = OPERATOR_TOKENS[token];
      const value = values[key];
      const trimmed = typeof value === "string" ? value.trim() : "";
      if (trimmed.length > 0) {
        segments.push({ kind: "value", text: trimmed });
      } else {
        pushProse(OPERATOR_UNSET_MARKERS[key]);
      }
    }

    lastIndex = TOKEN_PATTERN.lastIndex;
  }

  pushProse(text.slice(lastIndex));
  return { segments };
}

/**
 * One value, resolved to what a page should print — its value or its marker.
 *
 * For the surfaces that render a value on its own rather than inside a
 * sentence: the DMCA agent designation is a definition list, not prose, so it
 * needs the resolution without the segmentation. The trust rule is unchanged —
 * `isSet` tells the caller whether the text came from an operator, and a
 * caller renders both cases as plain text either way.
 */
export function resolveOperatorValue(
  key: OperatorValueKey,
  values: OperatorValues = {},
): { text: string; isSet: boolean } {
  const value = values[key];
  const trimmed = typeof value === "string" ? value.trim() : "";
  return trimmed.length > 0
    ? { text: trimmed, isSet: true }
    : { text: OPERATOR_UNSET_MARKERS[key], isSet: false };
}
