/**
 * Spec 048: what a refusal says to the person, from the browser's reader or
 * the server, with the code the server gives it (SHEET_ENCRYPTED, …).
 */
import { GraphQLRequestError } from "@/api/graphqlClient";

export interface Problem {
  text: string;
  code?: string;
}

/** The browser reader's refusal, with the code the server would give. */
export class SheetRefused extends Error {
  constructor(
    message: string,
    readonly code?: string,
  ) {
    super(message);
  }
}

/** What a refusal says to the person, from its code where there is one. */
export function refusalText(err: unknown): string {
  if (err instanceof GraphQLRequestError) {
    if (err.codes.includes("PLAN_CHANGED")) {
      return "The character changed while you were reviewing. Read the sheet again.";
    }
    if (err.codes.includes("FORBIDDEN")) {
      return "You may not change this character.";
    }
    if (err.codes.includes("FEATURE_DISABLED")) {
      return "Bringing in sheets is switched off on this server.";
    }
    return err.errors[0] ?? err.message;
  }
  return err instanceof Error ? err.message : String(err);
}

/** A refusal as the page shows it: the sentence, and its code if any. */
export function refusalProblem(err: unknown): Problem {
  if (err instanceof SheetRefused) return { text: err.message, code: err.code };
  const code = err instanceof GraphQLRequestError ? err.codes[0] : undefined;
  return { text: refusalText(err), code };
}
