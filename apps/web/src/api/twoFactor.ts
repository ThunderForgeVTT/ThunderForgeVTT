import { withCsrf } from "@/api/auth";
import type {
  PendingTwoFactorEnrolment,
  TwoFactorConfirmation,
  TwoFactorQrMatrix,
  TwoFactorStatus,
} from "@/types/twoFactor";

const API_BASE = "/api";

/**
 * Spec 041 US1/US4 — the three calls the enrolment flow makes, and the two
 * ways a caller is allowed to have earned them.
 *
 * # The authorisation, and why there are two of them
 *
 * `contracts/enrolment.md` says these routes take "exactly one" of a live
 * session or a single-use ticket, and never a username. The server takes
 * exactly one of a username-and-password or a single-use ticket, which is the
 * same shape with one entrance still spelled the old way:
 *
 *   - **Account settings** (`TwoFactorEnrolmentPanel`) sends the account
 *     password. It reads as a re-authentication, which is normal on a security
 *     screen, and it is what the server has always accepted.
 *   - **A sign-in that requires enrolment** (`LoginView`, FR-019) sends the
 *     `login_two_factor_challenge_id` the login response just handed it.
 *     There is no session yet and re-posting the password from a challenge
 *     screen is not what the contract asks for. The challenge is minted only
 *     after a correct password, is bound to one account, dies in ten minutes,
 *     and is spent by the confirmation — which is also where the session
 *     cookie is issued, so the sign-in finishes where it left off (FR-020).
 *
 * Both entrances drive the identical flow: same reducer
 * (`services/twoFactorEnrolment`), same steps and wording
 * (`components/security/TwoFactorEnrolmentSteps`), differing only in where the
 * person arrives afterwards. That is FR-001a, and it is why the credentials
 * are an opaque handle here rather than a password the flow knows about.
 */
export type TwoFactorEnrolmentCredentials =
  /** Account settings and first-run setup. */
  | { username: string; password: string }
  /** A sign-in that requires enrolment: the challenge it was just handed. */
  | { challengeId: string };

/**
 * The credentials as the server names them. One of the two shapes, never
 * both — `authorise_enrolment` refuses a request carrying two proofs before it
 * evaluates either.
 */
export function enrolmentAuthorisationBody(
  credentials: TwoFactorEnrolmentCredentials,
): Record<string, string> {
  return "challengeId" in credentials
    ? { challenge_id: credentials.challengeId }
    : { username: credentials.username, password: credentials.password };
}

/** An error that keeps the server's machine-readable `status` alongside its prose. */
export class TwoFactorRequestError extends Error {
  readonly status: string;

  constructor(status: string, message: string) {
    super(message);
    this.name = "TwoFactorRequestError";
    this.status = status;
  }
}

interface SetupStartPayload {
  status?: string;
  message?: string;
  otpauth_url?: string | null;
  /** Not sent by the server yet. See `TwoFactorQrMatrix`. */
  secret?: string | null;
  qr?: { size?: number; modules?: string[] } | null;
}

interface SetupConfirmPayload {
  status?: string;
  message?: string;
  confirmed_at?: string | null;
  recovery_codes?: string[] | null;
  recovery_codes_notice?: string | null;
  /** True when a challenge authorised this and a session cookie came back. */
  signed_in?: boolean | null;
}

interface StatusPayload {
  enabled?: boolean;
  confirmed_at?: string | null;
  recovery_codes_remaining?: number | null;
  recovery_codes_low?: boolean;
  enrolment_pending?: boolean;
}

async function readJson<T>(response: Response): Promise<T | null> {
  const contentType = response.headers.get("content-type") ?? "";
  if (!contentType.includes("application/json")) {
    return null;
  }

  return (await response.json()) as T;
}

/**
 * The base32 secret carried by an `otpauth://` URI.
 *
 * The server puts the same bytes in the URI's `secret` parameter that it
 * stored, so the URI is a complete source for the typeable form and no extra
 * field is needed to show one. Contract rule 4 promises a `secret` field
 * grouped in fours; when the server starts sending it we prefer it, and until
 * then we derive the identical value from the URI.
 */
export function secretFromOtpauthUri(otpauthUrl: string): string | null {
  const query = otpauthUrl.slice(otpauthUrl.indexOf("?") + 1);
  if (!otpauthUrl.includes("?")) {
    return null;
  }

  const secret = new URLSearchParams(query).get("secret");
  return secret && secret.trim() ? secret.trim() : null;
}

/**
 * Group a base32 secret in fours so a person can read it off a screen without
 * losing their place (FR-002).
 *
 * Whitespace is cosmetic. Authenticators strip it, and the server never
 * receives a secret back, so the grouping cannot desynchronise anything.
 */
export function groupSecretForTyping(secret: string): string {
  const compact = secret.replace(/\s+/g, "").toUpperCase();
  return compact.replace(/(.{4})(?=.)/g, "$1 ");
}

function normalizeQr(qr: SetupStartPayload["qr"]): TwoFactorQrMatrix | null {
  if (!qr || typeof qr.size !== "number" || !Array.isArray(qr.modules)) {
    return null;
  }

  // A matrix that does not describe a square of `size` rows of `size`
  // characters is not one we will draw. Contract rule 6: a QR that cannot be
  // built is not an enrolment that cannot be completed, so this degrades to
  // the typeable secret rather than to an error.
  const rows = qr.modules;
  if (
    qr.size <= 0 ||
    rows.length !== qr.size ||
    rows.some((row) => typeof row !== "string" || row.length !== qr.size)
  ) {
    return null;
  }

  return { size: qr.size, modules: rows };
}

/**
 * Begin (or restart) an enrolment.
 *
 * Writes nothing live on the account: `two_factor_setup_start` stores the new
 * secret in `two_factor_pending_secret_encrypted` and leaves
 * `two_factor_enabled`, `two_factor_secret_encrypted` and
 * `two_factor_confirmed_at` alone (FR-013, ADR-081). Abandoning after this
 * call therefore leaves the account exactly as it was (FR-004), and the panel
 * is allowed to say so.
 */
export async function beginTwoFactorEnrolment(
  credentials: TwoFactorEnrolmentCredentials,
): Promise<PendingTwoFactorEnrolment> {
  const response = await fetch(`${API_BASE}/authentication/2fa/setup/start`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify(enrolmentAuthorisationBody(credentials)),
  });

  const payload = await readJson<SetupStartPayload>(response);

  if (!response.ok) {
    throw new TwoFactorRequestError(
      payload?.status ?? "error",
      payload?.message || "Could not start two-factor setup.",
    );
  }

  const otpauthUrl = payload?.otpauth_url ?? null;
  if (!otpauthUrl) {
    throw new TwoFactorRequestError(
      "two_factor_error",
      "The server did not return a provisioning URI.",
    );
  }

  const secret =
    payload?.secret?.trim() || secretFromOtpauthUri(otpauthUrl) || "";

  return {
    otpauthUrl,
    secret: secret ? groupSecretForTyping(secret) : "",
    qr: normalizeQr(payload?.qr),
  };
}

/**
 * Prove the pending secret with a current code, and receive the recovery codes.
 *
 * A wrong code rejects and changes nothing: `two_factor_setup_confirm` only
 * promotes the pending secret inside the `Ok(true)` arm, so the enrolment
 * survives a rejection and the same secret may be retried without re-scanning
 * (FR-001c). The panel depends on that and does not re-start on failure.
 *
 * The resolved value is the only place the recovery codes will ever exist
 * outside a hash (FR-009). There is no route that can return them again.
 */
export async function confirmTwoFactorEnrolment(
  credentials: TwoFactorEnrolmentCredentials,
  code: string,
): Promise<TwoFactorConfirmation> {
  const response = await fetch(`${API_BASE}/authentication/2fa/setup/confirm`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({
      ...enrolmentAuthorisationBody(credentials),
      code: code.replace(/\s+/g, ""),
    }),
  });

  const payload = await readJson<SetupConfirmPayload>(response);

  if (!response.ok) {
    throw new TwoFactorRequestError(
      payload?.status ?? "error",
      payload?.message || "That code was not accepted.",
    );
  }

  return {
    confirmedAt: payload?.confirmed_at ?? null,
    signedIn: payload?.signed_in === true,
    recoveryCodes: payload?.recovery_codes ?? [],
    recoveryCodesNotice:
      payload?.recovery_codes_notice ||
      "Keep these somewhere other than the device you just set up. Each works once. They will not be shown again.",
  };
}

/**
 * Read whether a second factor is in force and when it was confirmed (FR-005).
 *
 * Resolves to `null` when the instance cannot answer. `GET
 * /api/authentication/2fa/status` is specified in `contracts/enrolment.md`
 * but is not routed in `src/server/src/auth/mod.rs`, and no other endpoint or
 * GraphQL field exposes an account's own two-factor state — `PublicUser`
 * carries id, username, email, role, is_admin and timestamps and nothing
 * else. So on this instance the call 404s and the panel says it cannot tell,
 * rather than inventing an answer.
 *
 * TODO(spec-041): delete this comment once the route exists; the function
 * needs no change when it does.
 */
export async function readTwoFactorStatus(): Promise<TwoFactorStatus | null> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}/authentication/2fa/status`, {
      credentials: "same-origin",
    });
  } catch {
    return null;
  }

  if (!response.ok) {
    return null;
  }

  const payload = await readJson<StatusPayload>(response);
  if (!payload || typeof payload.enabled !== "boolean") {
    return null;
  }

  return {
    enabled: payload.enabled,
    confirmedAt: payload.confirmed_at ?? null,
    recoveryCodesRemaining: payload.recovery_codes_remaining ?? null,
    recoveryCodesLow: payload.recovery_codes_low ?? false,
    enrolmentPending: payload.enrolment_pending ?? false,
  };
}

/**
 * Spec 041 US4 (FR-012, FR-014): turn the second factor off, deliberately.
 *
 * Password **and** possession, which is exactly what adding one cost. The
 * session alone is not enough — it proves the password was held at sign-in,
 * possibly days ago on a machine that has since changed hands, and this is the
 * one action that makes every future sign-in cheaper.
 *
 * Exactly one of `code` or `recoveryCode`. Sending both would be asking for
 * two chances counted as one attempt, and the server refuses it.
 *
 * Returns the server's message on success, or throws with the refusal — which
 * is one of the few refusals in this product that is allowed to be specific,
 * because by the time it is reached the caller has already proved both
 * factors and there is no attacker left to keep in the dark.
 */
export async function disableTwoFactor(input: {
  password: string;
  code?: string;
  recoveryCode?: string;
}): Promise<string> {
  const response = await fetch(`${API_BASE}/authentication/2fa/disable`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({
      password: input.password,
      code: input.code?.trim() || undefined,
      recovery_code: input.recoveryCode?.trim() || undefined,
    }),
  });

  const body = (await response.json().catch(() => null)) as {
    status?: string;
    message?: string;
  } | null;

  if (!response.ok || body?.status !== "success") {
    throw new Error(
      body?.message ?? "The second factor could not be turned off.",
    );
  }
  return body.message ?? "Two-factor authentication is off for this account.";
}
