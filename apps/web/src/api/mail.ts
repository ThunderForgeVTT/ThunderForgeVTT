import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 040 US4: whether this instance can send mail, proof that it can, and
 * what it has failed to send.
 *
 * **No type in this file has a body field, and that is the point.** The server
 * asserts the absence against its own generated SDL; this side keeps the same
 * shape so a component cannot render a message body even if one appeared.
 * `subject` is optional because the server discloses it for exactly one thing
 * — the instance's own test message — and null for everything else, since a
 * subject about a person is content.
 */

export type OutboxState = "QUEUED" | "BLOCKED" | "SENDING" | "SENT" | "FAILED";

export interface MailAvailability {
  ready: boolean;
  /** Setting keys that are unset, by name. Never a value. */
  missing: string[];
  limitedFeatures: string[];
}

export interface OutboxEntry {
  id: string;
  purpose: string;
  toAddress: string;
  /** Present only for the instance's own test message. */
  subject: string | null;
  state: OutboxState;
  attempts: number;
  lastAttemptAt: string | null;
  nextAttemptAt: string | null;
  lastFailureReason: string | null;
  sentAt: string | null;
  createdAt: string;
}

export interface TestMailResult {
  delivered: boolean;
  /** On failure: what to fix. Never a password or any fragment of one. */
  reason: string | null;
  outboxId: string;
}

const OUTBOX_FIELDS = `
  id
  purpose
  toAddress
  subject
  state
  attempts
  lastAttemptAt
  nextAttemptAt
  lastFailureReason
  sentAt
  createdAt
`;

export async function fetchMailAvailability(): Promise<MailAvailability> {
  const data = await postGraphQL<{ mailAvailability: MailAvailability }>(
    `query MailAvailability {
      mailAvailability { ready missing limitedFeatures }
    }`,
  );
  return data.mailAvailability;
}

export async function fetchMailOutbox(
  state?: OutboxState,
  limit?: number,
): Promise<OutboxEntry[]> {
  const data = await postGraphQL<{ mailOutbox: OutboxEntry[] }>(
    `query MailOutbox($state: OutboxState, $limit: Int) {
      mailOutbox(state: $state, limit: $limit) { ${OUTBOX_FIELDS} }
    }`,
    { state, limit },
  );
  return data.mailOutbox;
}

/**
 * Send a real message through the real outbox, using the settings as they
 * resolve now. Rate limited on the server as well as administrators-only —
 * without both it is an open relay with a login page.
 */
export async function sendTestMail(to: string): Promise<TestMailResult> {
  const data = await postGraphQL<{ sendTestMail: TestMailResult }>(
    `mutation SendTestMail($to: String!) {
      sendTestMail(to: $to) { delivered reason outboxId }
    }`,
    { to },
  );
  return data.sendTestMail;
}

export async function retryOutboxMessage(id: string): Promise<OutboxEntry> {
  const data = await postGraphQL<{ retryOutboxMessage: OutboxEntry }>(
    `mutation RetryOutboxMessage($id: UUID!) {
      retryOutboxMessage(id: $id) { ${OUTBOX_FIELDS} }
    }`,
    { id },
  );
  return data.retryOutboxMessage;
}
