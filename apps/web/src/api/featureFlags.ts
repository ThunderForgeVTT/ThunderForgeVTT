import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 068 Story 2: what this instance has switched on.
 *
 * A flag is an instance setting in the `Features` group (ADR-091), so it is
 * the server's to resolve: the environment beats the instance's store beats
 * the declared default, per request. This file asks once and remembers the
 * answer; it never reads the environment or a build-time constant, and it
 * holds no defaults of its own — a default restated here would be a second
 * opinion about the same flag.
 *
 * Hiding a control is a courtesy, not the rule. Whatever a flag guards, the
 * server refuses it while the flag is off.
 *
 * # When it asks
 *
 * Once for whoever is looking — a visitor is told the public flags, a
 * signed-in member all of them, so signing in asks again — and once more when
 * an administrator changes a setting in the `Features` group, so the person
 * who flipped the switch sees it take. Everybody else sees it on their next
 * session, and is refused by the server in the meantime.
 *
 * Until the answer arrives every flag reads as off: a control that appears
 * and then is refused is worse than one that appears a moment late.
 */

/** Guards reading a source book into a library (spec 049). */
export const FEATURE_BOOK_IMPORT = "feature.book_import";

/** Whether this instance offers the demo at `/demo/` (spec 074). Public. */
export const FEATURE_DEMO = "feature.demo";

/** The settings group a flag is declared in, as the server names it. */
export const FEATURES_GROUP = "Features";

export type FeatureFlags = Readonly<Record<string, boolean>>;

/**
 * `/api/graphql` turns away anybody without a session before a resolver
 * runs, so a visitor asks the route that does not (spec 015). The resolver
 * is the same one and tells them only the public flags.
 */
const GRAPHQL_PUBLIC_ENDPOINT = "/api/graphql/public";

export async function fetchFeatureFlags(
  who: string | null,
): Promise<FeatureFlags> {
  const data = await postGraphQL<{
    featureFlags: { key: string; on: boolean }[];
  }>(
    `query FeatureFlags { featureFlags { key on } }`,
    undefined,
    who === null ? { endpoint: GRAPHQL_PUBLIC_ENDPOINT } : {},
  );
  return Object.fromEntries(
    data.featureFlags.map((flag) => [flag.key, flag.on]),
  );
}

const NONE: FeatureFlags = Object.freeze({});

let flags: FeatureFlags = NONE;
/** Whose answer `flags` is; `undefined` until anyone has been answered. */
let answeredFor: string | null | undefined;
/** Counts requests, so an answer that arrives late does not replace a newer one. */
let asked = 0;
const listeners = new Set<() => void>();

export function subscribeToFeatureFlags(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function currentFeatureFlags(): FeatureFlags {
  return flags;
}

async function ask(who: string | null): Promise<void> {
  const mine = ++asked;
  answeredFor = who;
  try {
    const answer = await fetchFeatureFlags(who);
    if (mine !== asked) return;
    flags = answer;
  } catch {
    if (mine !== asked) return;
    // Nothing is known, so nothing is shown, and the next mount asks again.
    answeredFor = undefined;
    return;
  }
  for (const listener of listeners) listener();
}

/** Ask for `who` (an account id, or null for a visitor) unless already answered. */
export function loadFeatureFlags(who: string | null): Promise<void> {
  if (answeredFor === who) return Promise.resolve();
  return ask(who);
}

/** Ask again for the same person: an administrator has just changed a flag. */
export function refreshFeatureFlags(): Promise<void> {
  return ask(answeredFor ?? null);
}

/** For tests: forget everything. */
export function forgetFeatureFlags(): void {
  flags = NONE;
  answeredFor = undefined;
  asked += 1;
}
