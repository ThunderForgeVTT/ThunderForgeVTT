import { test, expect } from "@playwright/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";
import { openAdminPage, type GqlResult } from "./fixtures/admin";

/**
 * Spec 037 against the real stack.
 *
 * The server half shipped with unit tests only, which is the exact shape of
 * the defect this session already paid for once: the publishing gate passed
 * every unit test and refused every share link on the e2e stack, because the
 * tests configured a fixture the real stack does not have. So the assertions
 * here are chosen for the things a fixture would paper over — an instance
 * with **no destination configured**, which is what the e2e stack genuinely
 * is, and what a fresh instance genuinely is too.
 *
 * That makes FR-018 the spine of the file. Recording is separated from
 * delivering so that a submission survives a destination that is missing,
 * misconfigured or down; on this stack the destination is missing, so every
 * successful submission below is that requirement being exercised rather than
 * merely asserted.
 */

const SUBMIT = `
  mutation SubmitFeedback($input: SubmitFeedbackInput!) {
    submitFeedback(input: $input) {
      id
      kind
      message
      deliveryState
      issueUrl
      attachments { id kind byteSize redactionCount }
      attachmentsExpireAt
    }
  }
`;

const MY_SUBMISSIONS = `
  query MySubmissions {
    mySubmissions { id kind message deliveryState issueUrl }
  }
`;

const DESTINATION_NOTICE = `
  query DestinationNotice {
    feedbackDestinationNotice {
      configured
      repository
      isPublic
      visibilityCheckedAt
    }
  }
`;

const UNDELIVERED = `
  query Undelivered {
    undeliveredFeedback {
      id
      kind
      deliveryState
      attemptCount
      lastFailureReason
    }
  }
`;

interface Submission {
  id: string;
  kind: string;
  message: string;
  deliveryState: string;
  issueUrl: string | null;
  attachments?: { id: string; kind: string; byteSize: number }[];
  attachmentsExpireAt?: string;
}

/** The fields every submission must carry, so each test names only its point. */
function submission(message: string, extra: Record<string, unknown> = {}) {
  return {
    kind: "ISSUE",
    message,
    clientVersion: "e2e",
    browser: "Chromium on Linux",
    attachments: [],
    ...extra,
  };
}

const base64 = (text: string) => Buffer.from(text, "utf8").toString("base64");

test.describe("Spec 037: feedback reaches the instance before it reaches anywhere else", () => {
  test("a submission is kept even though this instance has nowhere to send it", async ({
    page,
  }) => {
    const creds = freshCredentials("e2efb");
    await register(page, creds);

    // The notice a person is shown *before* they submit (FR-014). On this
    // stack it says nothing is configured — and that is the point of reading
    // it here rather than stubbing it: the submission below then happens
    // under the condition FR-018 exists for.
    const notice = await graphql<
      GqlResult<{ feedbackDestinationNotice: { configured: boolean } }>
    >(page, DESTINATION_NOTICE, {});
    expect(notice.errors).toBeFalsy();
    expect(notice.data?.feedbackDestinationNotice.configured).toBe(false);

    const text = `The initiative tracker skipped a turn — ${Date.now().toString(36)}`;
    const sent = await graphql<GqlResult<{ submitFeedback: Submission }>>(
      page,
      SUBMIT,
      { input: submission(text) },
    );

    expect(
      sent.errors,
      "a missing destination must not cost the person their report",
    ).toBeFalsy();
    const recorded = sent.data?.submitFeedback;
    expect(recorded?.message).toBe(text);

    // PENDING, not failed. At the instance — which FR-018 makes the record —
    // it did arrive, and the person is never shown somebody else's outage.
    expect(recorded?.deliveryState).toBe("PENDING");
    expect(recorded?.issueUrl).toBeNull();

    // And it is readable afterwards, which is the half that proves the write
    // committed rather than merely being echoed back.
    const mine = await graphql<GqlResult<{ mySubmissions: Submission[] }>>(
      page,
      MY_SUBMISSIONS,
      {},
    );
    expect(mine.data?.mySubmissions.map((s) => s.id)).toContain(recorded?.id);
  });

  test("an attachment carrying a secret is refused, and takes nothing with it", async ({
    page,
  }) => {
    const creds = freshCredentials("e2efbsec");
    await register(page, creds);

    // A log line the client's own filter should have caught. Sending it
    // anyway is the case the server validator exists for: FR-012 is not a
    // client-side courtesy, because the client is the untrusted half.
    const leaking = base64(
      "GET /api/worlds\nauthorization: Bearer abcdefgh12345678ABCDEFGH\n",
    );
    const refused = await graphql<
      GqlResult<{ submitFeedback: Submission }> & {
        errors?: { message: string; extensions?: Record<string, unknown> }[];
      }
    >(page, SUBMIT, {
      input: submission("Attaching my logs", {
        attachments: [{ kind: "LOGS", content: leaking }],
      }),
    });

    expect(refused.errors?.length).toBeTruthy();
    // The contract is the closed vocabulary in the extensions, not the prose
    // — which is why the assertion is here. `kind_name` answers
    // `bearer_token`, the rule's own identifier, and the message interpolates
    // it verbatim, so a person currently reads "(bearer_token)". Worth
    // smoothing; asserted as it is rather than as it ought to read, because a
    // test that encodes the wished-for string tests nothing.
    expect(refused.errors?.[0]?.extensions?.code).toBe(
      "FEEDBACK_CONTAINS_SECRET",
    );
    expect(refused.errors?.[0]?.extensions?.kind).toBe("bearer_token");
    // The refusal names a *kind* and never quotes the payload — an error
    // message is one of the places a credential gets logged next.
    expect(refused.errors?.[0]?.message ?? "").not.toContain(
      "abcdefgh12345678",
    );

    // The whole submission is refused, not the attachment alone. A row
    // without the evidence it was written for is worse than no row.
    const mine = await graphql<GqlResult<{ mySubmissions: Submission[] }>>(
      page,
      MY_SUBMISSIONS,
      {},
    );
    expect(mine.data?.mySubmissions ?? []).toHaveLength(0);

    // A redacted bundle carrying the marker the client leaves behind must
    // still be accepted. `[redacted: bearer token]` contains "bearer token",
    // which the shared bearer pattern matches — so this is the false positive
    // that would silently refuse every *correctly* filtered log bundle.
    const clean = base64(
      "GET /api/worlds\nauthorization: [redacted: bearer token]\n",
    );
    const accepted = await graphql<GqlResult<{ submitFeedback: Submission }>>(
      page,
      SUBMIT,
      {
        input: submission("Attaching my logs, filtered", {
          attachments: [{ kind: "LOGS", content: clean }],
        }),
      },
    );
    expect(
      accepted.errors,
      "a correctly redacted bundle must not be refused for its own marker",
    ).toBeFalsy();
    expect(accepted.data?.submitFeedback.attachments?.[0]?.kind).toBe("LOGS");
  });

  test("one account cannot read another's submissions", async ({
    page,
    browser,
  }) => {
    const mineCreds = freshCredentials("e2efbmine");
    await register(page, mineCreds);
    const sent = await graphql<GqlResult<{ submitFeedback: Submission }>>(
      page,
      SUBMIT,
      { input: submission(`Private note ${Date.now().toString(36)}`) },
    );
    const id = sent.data?.submitFeedback.id;
    expect(id).toBeTruthy();

    const otherContext = await browser.newContext();
    const other = await otherContext.newPage();
    await register(other, freshCredentials("e2efbother"));

    // FR-022. The enforcement is that `mySubmissions` takes no argument by
    // which another account could be named — so what this asserts is the
    // consequence: a second account sees its own empty list, not a filtered
    // view of everybody's.
    const theirs = await graphql<GqlResult<{ mySubmissions: Submission[] }>>(
      other,
      MY_SUBMISSIONS,
      {},
    );
    expect(theirs.errors).toBeFalsy();
    expect(theirs.data?.mySubmissions.map((s) => s.id) ?? []).not.toContain(id);

    await otherContext.close();
  });

  test("the sixth submission in ten minutes is refused, and says for how long", async ({
    page,
  }) => {
    const creds = freshCredentials("e2efbrate");
    await register(page, creds);

    for (let i = 0; i < 5; i += 1) {
      const ok = await graphql<GqlResult<{ submitFeedback: Submission }>>(
        page,
        SUBMIT,
        { input: submission(`Report ${i}`) },
      );
      expect(
        ok.errors,
        `submission ${i} should be within the limit`,
      ).toBeFalsy();
    }

    const refused = await graphql<
      GqlResult<{ submitFeedback: Submission }> & {
        errors?: { message: string; extensions?: Record<string, unknown> }[];
      }
    >(page, SUBMIT, { input: submission("Report 6") });

    expect(refused.errors?.length).toBeTruthy();
    expect(refused.errors?.[0]?.extensions?.code).toBe("FEEDBACK_RATE_LIMITED");
    // A refusal that does not say when to come back makes a client either
    // hammer the endpoint or give up; both are worse than waiting.
    expect(
      Number(refused.errors?.[0]?.extensions?.retryAfterSeconds),
    ).toBeGreaterThan(0);
  });

  test("the undelivered queue is the operator's, not everybody's", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2efbadm");
    await register(page, creds);

    // An ordinary account is refused outright — not given an empty list,
    // which would read the same in a passing test and quite differently in a
    // deployment where the queue was not empty.
    const refused = await graphql<
      GqlResult<{ undeliveredFeedback: unknown[] }>
    >(page, UNDELIVERED, {});
    expect(refused.errors?.length).toBeTruthy();

    const admin = await openAdminPage(browser);
    const allowed = await graphql<
      GqlResult<{
        undeliveredFeedback: { id: string; deliveryState: string }[];
      }>
    >(admin, UNDELIVERED, {});
    expect(allowed.errors).toBeFalsy();
    expect(Array.isArray(allowed.data?.undeliveredFeedback)).toBe(true);

    await admin.context().close();
  });
});
