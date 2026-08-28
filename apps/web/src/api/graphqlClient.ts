import { withCsrf } from "@/api/auth";

/**
 * The single GraphQL transport for `apps/web`.
 *
 * Before this, `postGraphQL` was privately re-declared in 23 files under
 * `src/api/`. Auditing them found the divergence was almost entirely
 * formatting — only two real differences existed (a custom endpoint in
 * `moderation.ts`, and the multipart uploader in `assets.ts`/`lore.ts`), both
 * folded in here. Consolidating was therefore safe; the value is in what every
 * copy got *wrong* together, fixed once below.
 *
 * Failure modes the duplicated version handled badly:
 *
 * 1. **Non-JSON responses.** Every copy did `await response.json()`
 *    unconditionally. A 502 from a proxy, an HTML error page, or an empty body
 *    threw `SyntaxError: Unexpected token '<'` — which tells a user nothing and
 *    a developer almost nothing. This is the most common real-world failure.
 * 2. **Network failures.** A rejected `fetch` surfaced as the browser's raw
 *    `TypeError: Failed to fetch`.
 * 3. **Multiple GraphQL errors.** Only `errors[0]` was ever shown; the rest
 *    were silently dropped.
 * 4. **No operation context.** Nothing said *which* query failed, so a logged
 *    "GraphQL request failed" was untraceable.
 * 5. **No timeout.** A hung request hung the calling view forever.
 *
 * Partial responses (`data` AND `errors` both present) still throw rather than
 * returning partial data. That is deliberate: callers here are typed as
 * receiving complete results, and silently handing back half a payload would
 * push the failure somewhere harder to diagnose.
 */

export const GRAPHQL_ENDPOINT = "/api/graphql";

/** Generous by design — every GraphQL call in this app is small, so this is a
 * guard against a hung connection rather than a latency budget. Callers doing
 * something genuinely slow can raise or disable it. */
export const DEFAULT_TIMEOUT_MS = 30_000;

type GraphQLErrorEntry = { message?: string };

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLErrorEntry[];
};

export interface GraphQLRequestOptions {
  /** Override the endpoint (used by the moderation admin surface). */
  endpoint?: string;
  /** Milliseconds before aborting. `null` disables the timeout entirely. */
  timeoutMs?: number | null;
}

/**
 * An error from a GraphQL call, carrying enough context to be diagnosable
 * from a log line alone.
 */
export class GraphQLRequestError extends Error {
  readonly operation: string | undefined;
  readonly status: number | undefined;
  /** Every message the server returned, not just the first. */
  readonly errors: string[];

  constructor(
    message: string,
    details: { operation?: string; status?: number; errors?: string[] } = {},
  ) {
    super(message);
    this.name = "GraphQLRequestError";
    this.operation = details.operation;
    this.status = details.status;
    this.errors = details.errors ?? [];
  }
}

/**
 * Best-effort operation name for error context, e.g. `WorldAbilities` from
 * `query WorldAbilities($worldId: UUID!) { ... }`. Purely diagnostic — a query
 * without a name is fine and simply yields `undefined`.
 */
export function operationNameOf(query: string): string | undefined {
  return /\b(?:query|mutation|subscription)\s+([A-Za-z_][A-Za-z0-9_]*)/.exec(
    query,
  )?.[1];
}

function collectMessages(errors: GraphQLErrorEntry[] | undefined): string[] {
  return (errors ?? [])
    .map((e) => e?.message)
    .filter((m): m is string => typeof m === "string" && m.trim() !== "");
}

/**
 * Reads a response body as a GraphQL payload without assuming it is JSON.
 *
 * Returns `null` when the body is not parseable, so the caller can report the
 * HTTP status instead of a `SyntaxError` about an unexpected `<`.
 */
async function readPayload<TData>(
  response: Response,
): Promise<GraphQLResponse<TData> | null> {
  let raw: string;
  try {
    raw = await response.text();
  } catch {
    return null;
  }
  if (raw.trim() === "") {
    return null;
  }
  try {
    return JSON.parse(raw) as GraphQLResponse<TData>;
  } catch {
    return null;
  }
}

/** Shared result handling for both the JSON and multipart paths. */
function unwrap<TData>(
  payload: GraphQLResponse<TData> | null,
  response: Response,
  operation: string | undefined,
  notOkFallback: string,
): TData {
  if (payload === null) {
    // Non-JSON or empty body — report the status, which is the only real
    // information available, rather than a JSON parse error.
    throw new GraphQLRequestError(
      response.ok
        ? "The server returned an unreadable response."
        : `${notOkFallback} (HTTP ${response.status})`,
      { operation, status: response.status },
    );
  }

  const messages = collectMessages(payload.errors);

  if (!response.ok || messages.length > 0) {
    throw new GraphQLRequestError(
      messages.length > 0
        ? messages.join("; ")
        : `${notOkFallback} (HTTP ${response.status})`,
      { operation, status: response.status, errors: messages },
    );
  }

  if (payload.data === undefined || payload.data === null) {
    throw new GraphQLRequestError("GraphQL response did not include data", {
      operation,
      status: response.status,
    });
  }

  return payload.data;
}

async function send(
  url: string,
  init: RequestInit,
  operation: string | undefined,
  timeoutMs: number | null,
): Promise<Response> {
  const controller = timeoutMs === null ? null : new AbortController();
  const timer =
    controller === null
      ? null
      : setTimeout(() => controller.abort(), timeoutMs ?? 0);

  try {
    return await fetch(url, { ...init, signal: controller?.signal });
  } catch {
    if (controller?.signal.aborted) {
      throw new GraphQLRequestError(
        "The request timed out. Please try again.",
        { operation },
      );
    }
    // A rejected fetch is a transport failure — DNS, offline, CORS, connection
    // reset. The browser's raw TypeError is not useful to a user.
    throw new GraphQLRequestError(
      "Could not reach the server. Check your connection.",
      {
        operation,
      },
    );
  } finally {
    if (timer !== null) {
      clearTimeout(timer);
    }
  }
}

/** POST a GraphQL query or mutation. */
export async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
  options: GraphQLRequestOptions = {},
): Promise<TData> {
  const operation = operationNameOf(query);
  const response = await send(
    options.endpoint ?? GRAPHQL_ENDPOINT,
    {
      method: "POST",
      credentials: "same-origin",
      headers: withCsrf({ "Content-Type": "application/json" }),
      body: JSON.stringify({ query, variables }),
    },
    operation,
    options.timeoutMs === undefined ? DEFAULT_TIMEOUT_MS : options.timeoutMs,
  );

  return unwrap<TData>(
    await readPayload<TData>(response),
    response,
    operation,
    "Request failed",
  );
}

/**
 * POST a GraphQL mutation carrying one file, per the GraphQL multipart request
 * spec. Used by the lore and canvas image uploads.
 *
 * Uploads get **no timeout by default** — a large file over a slow link is
 * legitimately slow, and aborting it mid-transfer would be worse than waiting.
 */
export async function postGraphQLMultipart<TData>(
  query: string,
  variables: Record<string, unknown>,
  file: Blob,
  filePathInVariables: string,
  options: GraphQLRequestOptions = {},
): Promise<TData> {
  const operation = operationNameOf(query);

  const formData = new FormData();
  formData.append(
    "operations",
    JSON.stringify({
      query,
      variables: { ...variables, [filePathInVariables]: null },
    }),
  );
  formData.append(
    "map",
    JSON.stringify({ "0": [`variables.${filePathInVariables}`] }),
  );
  formData.append("0", file);

  const response = await send(
    options.endpoint ?? GRAPHQL_ENDPOINT,
    {
      method: "POST",
      credentials: "same-origin",
      // Deliberately no Content-Type: the browser sets the multipart boundary
      // itself when the body is a FormData instance.
      headers: withCsrf(),
      body: formData,
    },
    operation,
    options.timeoutMs === undefined ? null : options.timeoutMs,
  );

  return unwrap<TData>(
    await readPayload<TData>(response),
    response,
    operation,
    "Upload failed",
  );
}
