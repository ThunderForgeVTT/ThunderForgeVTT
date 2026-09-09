/**
 * Driving the GitHub stub from a test.
 *
 * Spec 037, `contracts/e2e-harness.md` § 2. The stub itself is
 * `scripts/github-stub.mjs`, started once per shard by
 * `scripts/e2e-parallel.mjs` — it has to be its own process because the thing
 * that talks to it is the **backend**, not the browser. This file is the
 * control client.
 *
 * # Why there is a stub at all
 *
 * Feedback delivery had never run against anything. The outbox, the backoff
 * curve, the search-before-create adoption path and the "exactly once after an
 * ambiguous failure" rule were all proven against in-process fakes, and no
 * request had ever been put on a wire.
 *
 * The server reaches the stub through `GITHUB_API_BASE` / `GITHUB_WEB_BASE`,
 * which are configuration values an operator running GitHub Enterprise sets
 * too — so this exercises the production code path and not a test branch.
 */

export interface RecordedIssue {
  number: number;
  title: string;
  body: string;
  labels: string[];
  state: "open" | "closed";
}

export interface RecordedFile {
  path: string;
  message: string;
  /** Base64, exactly as the Contents API takes it. */
  content: string;
}

export interface StubState {
  issues: RecordedIssue[];
  files: RecordedFile[];
  /** Every request received, as `METHOD /path`. */
  calls: string[];
}

/** This shard's stub, or a failure that says why there is not one. */
export function stubUrl(): string {
  const url = process.env.THUNDERFORGE_E2E_GITHUB_STUB;
  if (!url) {
    throw new Error(
      "THUNDERFORGE_E2E_GITHUB_STUB is unset. Delivery specs run under " +
        "`node scripts/e2e-parallel.mjs`, which starts a stub per shard.",
    );
  }
  return url.replace(/\/$/, "");
}

async function control<T>(
  path: string,
  init?: RequestInit,
  base = stubUrl(),
): Promise<T> {
  const response = await fetch(`${base}${path}`, init);
  if (!response.ok) {
    throw new Error(`stub answered ${response.status} for ${path}`);
  }
  return (await response.json()) as T;
}

/** Everything the stub has been sent. */
export function stubState(base?: string): Promise<StubState> {
  return control<StubState>("/_control/issues", undefined, base ?? stubUrl());
}

/** Forget everything. Call between tests: one stub serves a whole shard. */
export function resetStub(base?: string): Promise<unknown> {
  return control("/_control/reset", { method: "POST" }, base ?? stubUrl());
}

/**
 * Make the next write fail **after** it has been recorded.
 *
 * This is the whole of US6. The failure worth testing is the request that
 * succeeded at the host and whose answer never arrived — a retry then has to
 * find the issue it already created rather than create a second one. A stub
 * that failed *before* recording would be simulating a different and easier
 * bug.
 */
export function failNext(
  mode: { status: number } | { transport: true },
  base?: string,
): Promise<unknown> {
  return control(
    "/_control/fail-next",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(mode),
    },
    base ?? stubUrl(),
  );
}

/** Close a recorded issue, so a later read sees a closed one. */
export function closeIssue(number: number, base?: string): Promise<unknown> {
  return control(
    `/_control/close/${number}`,
    { method: "POST" },
    base ?? stubUrl(),
  );
}

/** Wait until the stub has recorded at least `count` issues, or fail saying what it has. */
export async function waitForIssues(
  count: number,
  { timeoutMs = 90_000, base }: { timeoutMs?: number; base?: string } = {},
): Promise<RecordedIssue[]> {
  const deadline = Date.now() + timeoutMs;
  let state: StubState = { issues: [], files: [], calls: [] };
  while (Date.now() < deadline) {
    state = await stubState(base);
    if (state.issues.length >= count) return state.issues;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(
    `expected ${count} issue(s); the stub has ${state.issues.length}. ` +
      `Requests it did see: ${state.calls.join(", ") || "none at all"}`,
  );
}
