import type { Disclosed } from "@/engine/sdk/Disclosed";
import type { ResourceDefinition } from "@/engine/sdk/ResourceDefinition";

/**
 * Reading back what the engine would draw for a token.
 *
 * Spec 029 FR-021. The engine holds the resolved status; this reads it. That
 * is what lets the React corner panel show a token's resources without holding
 * its own copy — Constitution I permits React to *observe* engine state and
 * forbids it becoming a second source of truth, and the difference between the
 * two is exactly whether a read like this exists.
 *
 * Mirrors `stats.ts`'s approach and for the same reason: importing the engine
 * module here is free (ES module instances are shared, so this reaches the
 * instance `index.ts` mounted) and it never mounts one itself. If the engine
 * is not up, this answers `null` rather than starting a wasm load on behalf of
 * a panel.
 *
 * Read-only, deliberately. `probe.ts` states the principle this follows: a
 * debugging surface that can also mutate state becomes a way to write tests
 * that pass against situations the application cannot reach.
 */

export interface TokenStatusResource {
  definition: ResourceDefinition;
  disclosed: Disclosed;
}

type StatusModule = {
  get_token_status?: (tokenId: string) => string;
  list_token_status?: () => string;
};

let module: StatusModule | null = null;
let loadFailed = false;

async function statusModule(): Promise<StatusModule | null> {
  if (module || loadFailed) return module;
  try {
    module = (await import("@thunderforge/engine/engine")) as StatusModule;
  } catch {
    loadFailed = true;
  }
  return module;
}

/**
 * What the engine would draw for one token, or `null`.
 *
 * `null` covers the engine not being mounted, a bundle predating this call,
 * and the token having no status. All three mean "show no panel", never "show
 * an empty one" — an empty panel claims the token has resources at zero, which
 * is a different and much stronger statement.
 */
export async function readTokenStatus(
  tokenId: string,
): Promise<TokenStatusResource[] | null> {
  const engine = await statusModule();
  if (!engine?.get_token_status) return null;
  try {
    const raw = engine.get_token_status(tokenId);
    const parsed = JSON.parse(raw) as TokenStatusResource[] | null;
    return parsed && parsed.length > 0 ? parsed : null;
  } catch {
    return null;
  }
}

/**
 * Every token the engine currently holds status for, keyed by token id.
 *
 * `readTokenStatus` answers about one token, which is what a panel needs. This
 * answers about the board, which is what a measurement needs: whether the
 * displays a capacity figure claims to include are actually on the screen yet.
 * Status resolves progressively as a scene loads, and a frame time sampled
 * before it finishes describes a board that is still filling in.
 *
 * `{}` covers the engine not being mounted and a bundle predating the call —
 * both mean "nothing is displaying", which is the honest answer to ask this.
 */
export async function listTokenStatus(): Promise<
  Record<string, TokenStatusResource[]>
> {
  const engine = await statusModule();
  if (!engine?.list_token_status) return {};
  try {
    return JSON.parse(engine.list_token_status()) as Record<
      string,
      TokenStatusResource[]
    >;
  } catch {
    return {};
  }
}
