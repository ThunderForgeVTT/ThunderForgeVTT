import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";
import {
  failNext,
  resetStub,
  stubState,
  waitForIssues,
} from "./fixtures/githubStub";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 037 US3/US6: feedback actually reaching a destination.
 *
 * # What had never been exercised
 *
 * Delivery. All of it. The outbox, the backoff curve, the search-before-create
 * adoption path and the "exactly once after an ambiguous failure" rule were
 * proven against in-process fakes, and no request had ever been put on a wire.
 * `feedback.spec.ts` says as much in its own header: this harness had no
 * destination, so every test there is about an instance that cannot deliver.
 *
 * # How this reaches a destination without touching product code
 *
 * `scripts/github-stub.mjs` speaks the six endpoints delivery uses, and the
 * backend is pointed at it with `GITHUB_API_BASE` — the same configuration
 * value an operator running GitHub Enterprise sets. There is no test branch in
 * the server; a test that exercises a test-only branch proves the branch.
 *
 * # Why the destination is inserted and removed here
 *
 * There is no mutation for it — it is one row, read by the delivery pass. And
 * it **must not be seeded globally**, because `feedback.spec.ts`'s entire
 * premise is an instance with nowhere to send anything. So this file creates
 * it, and `afterAll` removes it whether or not the tests passed. The two files
 * can then share a shard without one deciding what the other proves.
 */

test.describe.configure({ mode: "serial" });

/**
 * The delivery pass ticks every 30 seconds, so a test that waits for one
 * cannot live inside the 30-second default — it would time out at exactly the
 * moment the thing it is waiting for becomes possible. Three minutes covers a
 * submission, a tick, a failure and a retry.
 *
 * Set per test rather than in a `beforeEach`, because Playwright requires that
 * hook's first argument to be an object destructuring pattern and the empty
 * one ESLint then objects to is not worth a disable comment.
 */
const DELIVERY_TIMEOUT_MS = 180_000;

const OWNER_REPO = "thunderforge/feedback-stub";

function sql(statement: string): string {
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";
  return execFileSync(
    "docker",
    [
      "exec",
      "-i",
      container,
      "psql",
      "-U",
      dbUser,
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
      "-t",
      "-A",
    ],
    { input: statement, encoding: "utf-8" },
  ).trim();
}

const SUBMIT = `
  mutation Submit($input: SubmitFeedbackInput!) {
    submitFeedback(input: $input) {
      id
      kind
      deliveryState
      attachments { id kind }
    }
  }
`;

interface Submission {
  id: string;
  kind: string;
  deliveryState: string;
  attachments?: { id: string; kind: string }[];
}
interface GqlResult<T> {
  data?: T | null;
  errors?: { message: string }[];
}

function submission(
  message: string,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  // The same shape `feedback.spec.ts` uses, copied rather than reinvented:
  // the input type is not obvious from the outside and guessing at it wastes a
  // run finding out.
  return {
    kind: "ISSUE",
    message,
    clientVersion: "e2e",
    browser: "Chromium on Linux",
    attachments: [],
    ...extra,
  };
}

async function submit(
  page: Page,
  message: string,
  extra: Record<string, unknown> = {},
): Promise<Submission> {
  const result = await graphql<GqlResult<{ submitFeedback: Submission }>>(
    page,
    SUBMIT,
    { input: submission(message, extra) },
  );
  expect(
    result.errors,
    `submitting was refused: ${JSON.stringify(result.errors)}`,
  ).toBeFalsy();
  return result.data!.submitFeedback;
}

test.beforeAll(async () => {
  // `installation_ref` is whatever the stub will mint a token for; it does not
  // verify. The repository is what the created issue is addressed to.
  sql(
    `INSERT INTO feedback_destination
       (id, installation_ref, repository_ref, attachment_branch, created_at, updated_at)
     VALUES (gen_random_uuid(), '1', '${OWNER_REPO}', 'feedback-attachments', now(), now())
     ON CONFLICT DO NOTHING;`,
  );
  await resetStub();
});

test.afterAll(async () => {
  // Removed whether or not this file passed: `feedback.spec.ts` asserts an
  // instance with no destination, and leaving one behind would make that file
  // fail with a story about feedback that is really a story about ordering.
  sql("DELETE FROM feedback_destination;");
});

test.describe("Spec 037 US3: feedback reaches a destination", () => {
  test("a submission becomes an issue carrying its kind and its delivery key", async ({
    page,
  }) => {
    test.setTimeout(DELIVERY_TIMEOUT_MS);
    await resetStub();
    const creds = freshCredentials("e2efbdel");
    await register(page, creds);

    const sent = await submit(page, "The map will not drag on Firefox");

    // The delivery pass runs on its own schedule; nothing is pressed here,
    // because "an operator does not have to do anything" is the requirement.
    const issues = await waitForIssues(1);
    expect(issues[0].title).toContain("The map will not drag");

    // The delivery key is what makes the adoption path single-flight, so it
    // has to be *in the body* rather than merely computed — a retry finds an
    // existing issue by searching for it. Read from the row rather than
    // guessed, because a test that asserts a key it invented would pass
    // against a body carrying no key at all.
    const deliveryKey = sql(
      `SELECT delivery_key FROM feedback_submissions WHERE id = '${sent.id}';`,
    );
    expect(deliveryKey).toMatch(/^[0-9a-f-]{36}$/);
    expect(
      issues[0].body.toLowerCase(),
      "the issue body carries no delivery key, so a retry could not find it",
    ).toContain(deliveryKey.toLowerCase());
  });

  test("an attachment is committed before the issue that references it", async ({
    page,
  }) => {
    test.setTimeout(DELIVERY_TIMEOUT_MS);
    await resetStub();
    const creds = freshCredentials("e2efbdela");
    await register(page, creds);

    const logs = Buffer.from(
      "GET /api/worlds\nnothing secret in here\n",
      "utf-8",
    ).toString("base64");
    await submit(page, "Logs attached", {
      attachments: [{ kind: "LOGS", content: logs }],
    });

    await waitForIssues(1);
    const state = await stubState();

    expect(
      state.files.length,
      `no file was committed; the stub saw: ${state.calls.join(", ")}`,
    ).toBeGreaterThan(0);

    // Order matters and is not incidental: an issue that references a file
    // committed after it is an issue with a broken link for as long as the
    // second request takes, and forever if it never lands.
    const firstPut = state.calls.findIndex((c) => c.startsWith("PUT "));
    const firstIssue = state.calls.findIndex((c) =>
      /^POST \/repos\/.+\/issues$/.test(c),
    );
    expect(firstPut).toBeGreaterThanOrEqual(0);
    expect(
      firstPut,
      "the issue was created before the attachment it points at",
    ).toBeLessThan(firstIssue);
  });
});

test.describe("Spec 037 US6: a failure that could have succeeded", () => {
  test("an ambiguous failure produces exactly one issue, not two", async ({
    page,
  }) => {
    test.setTimeout(DELIVERY_TIMEOUT_MS);
    await resetStub();
    const creds = freshCredentials("e2efbamb");
    await register(page, creds);

    // The one failure worth testing: the request arrives, the host acts on it,
    // and the answer never comes back. The server cannot know whether it
    // succeeded — and the wrong response is to try again and create a second
    // issue for one report.
    await failNext({ transport: true });
    await submit(page, "This one will be ambiguous");

    // Two issues here would mean the retry did not search first.
    const issues = await waitForIssues(1);
    expect(issues.length).toBeGreaterThanOrEqual(1);

    // Wait for the *retry* rather than for a duration. The backoff is 30
    // seconds and the pass ticks every 30, so "sleep 45" lands in the gap
    // between them about half the time — a flake that would read as "the
    // retry never searched" and send somebody looking at the wrong code.
    const deadline = Date.now() + 150_000;
    let after = await stubState();
    while (
      Date.now() < deadline &&
      !after.calls.some((c) => c.startsWith("GET /search/issues"))
    ) {
      await new Promise((resolve) => setTimeout(resolve, 2_000));
      after = await stubState();
    }

    // It found the existing issue by searching rather than by guessing. This
    // is the assertion that makes the next one mean something: without a
    // search, "exactly once" could hold by luck.
    expect(
      after.calls.some((c) => c.startsWith("GET /search/issues")),
      `the retry never searched. The stub saw: ${after.calls.join(", ")}`,
    ).toBe(true);

    const mine = after.issues.filter((i) =>
      i.title.includes("This one will be ambiguous"),
    );
    expect(
      mine.length,
      `one report produced ${mine.length} issues after an ambiguous failure`,
    ).toBe(1);
  });
});
