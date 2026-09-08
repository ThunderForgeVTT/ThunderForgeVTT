/**
 * Reading a real SMTP sink from a test — spec 040 contracts/e2e-fixtures.md § 2.
 *
 * # Why a real server and not a capture
 *
 * The in-process `CapturingTransport` proves the code *around* `lettre`. It
 * proves nothing about `lettre`, the From address, or STARTTLS negotiation,
 * and those are where mail actually fails. Mailpit speaks real SMTP, so a
 * message that arrives here left the process the way a message leaves it in
 * production.
 *
 * # One sink per shard
 *
 * `scripts/e2e-parallel.mjs` starts a Mailpit container per shard and passes
 * its API base in `THUNDERFORGE_E2E_MAILPIT_API`. A shared sink would make
 * "the newest message" mean whichever shard sent last — a flake with a
 * completely plausible story, which is the worst kind.
 *
 * # What these never read
 *
 * A body. `contracts/mail.md` rule 1 says the product does not expose one, and
 * a suite that reads bodies out of the sink builds the habit the product
 * refuses. `subject` is read for the instance's own test message and nothing
 * else.
 */

export interface MailpitAddress {
  Name: string;
  Address: string;
}

/** One message, as Mailpit's `/api/v1/messages` summarises it. */
export interface MailpitMessage {
  ID: string;
  From: MailpitAddress | null;
  To: MailpitAddress[];
  Subject: string;
  Created: string;
}

interface MailpitListing {
  messages: MailpitMessage[];
}

/**
 * This shard's Mailpit API base, or a failure that says why.
 *
 * Deliberately throws rather than returning a default. A spec that fell back
 * to `localhost:8025` would read the *developer's* dev-stack sink, pass
 * locally against messages it never sent, and fail only in CI — which is the
 * failure mode the per-shard rule exists to prevent.
 */
export function mailpitApi(): string {
  const base = process.env.THUNDERFORGE_E2E_MAILPIT_API;
  if (!base) {
    throw new Error(
      "THUNDERFORGE_E2E_MAILPIT_API is unset. Mail specs run under " +
        "`node scripts/e2e-parallel.mjs`, which starts a Mailpit per shard.",
    );
  }
  return base.replace(/\/$/, "");
}

/** The SMTP port this shard's Mailpit listens on, for configuring the instance. */
export function mailpitSmtpPort(): number {
  const port = Number(process.env.THUNDERFORGE_E2E_MAILPIT_SMTP_PORT);
  if (!Number.isInteger(port) || port <= 0) {
    throw new Error(
      "THUNDERFORGE_E2E_MAILPIT_SMTP_PORT is unset or not a port. Mail specs " +
        "run under `node scripts/e2e-parallel.mjs`.",
    );
  }
  return port;
}

/** The messages Mailpit has received, newest first. */
export async function inbox(baseUrl = mailpitApi()): Promise<MailpitMessage[]> {
  const response = await fetch(`${baseUrl}/api/v1/messages?limit=200`);
  if (!response.ok) {
    throw new Error(
      `Mailpit answered ${response.status} reading the inbox at ${baseUrl}`,
    );
  }
  const listing = (await response.json()) as MailpitListing;
  return listing.messages ?? [];
}

/** Empty the sink. */
export async function clearInbox(baseUrl = mailpitApi()): Promise<void> {
  const response = await fetch(`${baseUrl}/api/v1/messages`, {
    method: "DELETE",
  });
  if (!response.ok) {
    throw new Error(
      `Mailpit answered ${response.status} clearing the inbox at ${baseUrl}`,
    );
  }
}

/**
 * Wait for a message addressed to `to`, or fail saying what did arrive.
 *
 * The "what did arrive" half is the point. A bare timeout on an empty sink and
 * a timeout on a sink holding a message to the wrong address look identical in
 * a report and mean completely different things — the first is delivery
 * failing, the second is the From/To wiring being wrong.
 */
export async function waitForMessage(
  to: string,
  { baseUrl = mailpitApi(), timeoutMs = 20_000 } = {},
): Promise<MailpitMessage> {
  const deadline = Date.now() + timeoutMs;
  let seen: MailpitMessage[] = [];

  while (Date.now() < deadline) {
    seen = await inbox(baseUrl);
    const found = seen.find((message) =>
      message.To.some(
        (recipient) => recipient.Address.toLowerCase() === to.toLowerCase(),
      ),
    );
    if (found) return found;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  const summary = seen.length
    ? seen
        .map((m) => `${m.To.map((t) => t.Address).join(", ")} — "${m.Subject}"`)
        .join("; ")
    : "nothing at all";
  throw new Error(
    `No message to ${to} arrived within ${timeoutMs}ms. Mailpit holds: ${summary}`,
  );
}

/**
 * Assert that nothing arrives for `to` within the window.
 *
 * Needed for the half of Scenario D that matters most: with mail unconfigured
 * a message must be *held and recorded*, not delivered and not discarded.
 * Proving it was not delivered needs a sink that stays empty.
 */
export async function expectNoMessage(
  to: string,
  { baseUrl = mailpitApi(), windowMs = 3_000 } = {},
): Promise<void> {
  const deadline = Date.now() + windowMs;
  while (Date.now() < deadline) {
    const found = (await inbox(baseUrl)).find((message) =>
      message.To.some(
        (recipient) => recipient.Address.toLowerCase() === to.toLowerCase(),
      ),
    );
    if (found) {
      throw new Error(
        `A message to ${to} was delivered when none should have been ` +
          `(subject "${found.Subject}").`,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
}
