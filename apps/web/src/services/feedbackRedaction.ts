/**
 * Redaction for in-app feedback evidence (spec 037, FR-012).
 *
 * # The promise this defends
 *
 * FR-012 says session identifiers, tokens, cookies and credentials must not
 * leave the browser in any attachment, *whatever the person approves*, and
 * that redaction happens **before** the review step. The second half is the
 * part with a plausible wrong answer: a design that redacts on submit, or on
 * the server, satisfies the words and breaks the promise, because the person
 * would have approved something other than what was sent.
 *
 * So this is a **pure function applied at push time** into the log ring
 * buffer (`feedbackLogBuffer.ts`). The buffer never holds a secret at any
 * instant. The review renders the buffer and the submission sends the buffer,
 * and they are the same bytes because they are the same array.
 *
 * # Where the rules live, and why not here
 *
 * `config/feedback-redaction.json`. The server's validator reads the same
 * file, so a rule cannot exist on one side only: a server-only rule would
 * produce refusals the person could not have anticipated, and a client-only
 * rule would be no rule at all. See research.md § R6 and
 * contracts/attachments.md § 2.
 *
 * The server's half is to **refuse, never rewrite** — a match in an approved
 * payload is `FEEDBACK_CONTAINS_SECRET`, not an edit — because an edit after
 * approval is the exact failure FR-012 exists to prevent, even when the edit
 * is an improvement.
 *
 * # What is never redacted
 *
 * The message the person typed. spec.md's Edge Cases settle it: "what a
 * person deliberately types is theirs, and the review step is where they see
 * it." This module is applied to what the *app* collected.
 */

import ruleSet from "../../../../config/feedback-redaction.json";

/** One rule as it is written in the shared file. */
interface RuleDefinition {
  kind: string;
  why: string;
  pattern?: string;
  ignore_case?: boolean;
  dynamic?: boolean;
  replacement: string;
}

/** The result of redacting one string: what is left, and what was taken. */
export interface Redaction {
  /** The text with every match replaced by its visible marker. */
  text: string;
  /** How many replacements were made. */
  count: number;
  /** Which rule kinds fired, in the order the rules are declared. */
  kinds: string[];
}

const definitions = ruleSet.rules as RuleDefinition[];

/** The identifier of the rule set both sides are agreeing on. */
export const REDACTION_SCHEMA_VERSION = ruleSet.schema_version;

/** Every rule kind declared, dynamic ones included. Order is rule order. */
export const REDACTION_KINDS: readonly string[] = definitions.map(
  (rule) => rule.kind,
);

interface CompiledRule {
  kind: string;
  expression: RegExp;
  replacement: string;
}

/**
 * Compiled once at module load. `g` is not optional: a rule that stopped at
 * the first match would leave the second token on the line, which is the
 * common case for a logged request that carries both a header and a URL.
 */
const staticRules: CompiledRule[] = definitions
  .filter((rule) => !rule.dynamic && rule.pattern !== undefined)
  .map((rule) => ({
    kind: rule.kind,
    expression: new RegExp(
      rule.pattern as string,
      rule.ignore_case ? "gi" : "g",
    ),
    replacement: rule.replacement,
  }));

const emailRule = definitions.find((rule) => rule.kind === "submitter_email");

/**
 * The submitter's own address, once the app knows it.
 *
 * FR-013 keeps the submitter's email out of the destination, and a log line
 * is one of the two routes it could take. The address is not a constant, so
 * this rule is declared `dynamic` in the shared file and built here from
 * whatever the signed-in account turns out to be. Until it is set, the rule
 * simply does not fire — which is correct for a session with nobody signed
 * in, where there is no address to protect.
 */
let submitterRule: CompiledRule | null = null;

/** Escape a literal so it can be embedded in a pattern as itself. */
function escapeForPattern(literal: string): string {
  return literal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Teach the redactor the signed-in account's address, or forget it.
 *
 * Called with the account's email when authentication resolves and with
 * `null` on sign-out. Entries already in the buffer were redacted with the
 * rules in force when they were captured — this cannot reach back and change
 * them, which is the correct behaviour for a buffer whose whole claim is that
 * its bytes do not change after the fact.
 */
export function setSubmitterEmail(email: string | null): void {
  const trimmed = email?.trim() ?? "";
  if (trimmed === "" || emailRule === undefined) {
    submitterRule = null;
    return;
  }

  submitterRule = {
    kind: emailRule.kind,
    expression: new RegExp(escapeForPattern(trimmed), "gi"),
    replacement: emailRule.replacement,
  };
}

/**
 * Remove every secret shape this product knows about from one string.
 *
 * Pure: same input, same output, no state read except the submitter address
 * set above. Applied at push time, never later.
 *
 * Nothing is deleted — every match becomes a visible marker naming what kind
 * of thing was taken, because the person is entitled to see that something
 * was removed, and because a silent removal is indistinguishable from a rule
 * that never fired.
 */
export function redact(line: string): Redaction {
  let text = line;
  let count = 0;
  const kinds: string[] = [];

  const rules = submitterRule ? [...staticRules, submitterRule] : staticRules;

  for (const rule of rules) {
    let fired = 0;
    // `replace` resets `lastIndex` itself, so sharing one compiled global
    // expression across calls is safe.
    text = text.replace(rule.expression, () => {
      fired += 1;
      return rule.replacement;
    });

    if (fired > 0) {
      count += fired;
      kinds.push(rule.kind);
    }
  }

  return { text, count, kinds };
}
