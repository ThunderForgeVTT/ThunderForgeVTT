/**
 * Spec 041 US1 — the shapes the enrolment flow moves around.
 *
 * These are the *client's* names for what `/api/authentication/2fa/...`
 * returns. They are deliberately a little wider than what the server sends
 * today, in exactly one direction: `qr` and `secret` are optional. That was
 * once because the server did not send them; since 2026-09-09 it does
 * (`src/server/src/qr.rs`), and they stay optional because
 * `contracts/enrolment.md` rule 6 says a failure to build the QR is not a
 * failure to enrol. Nothing in the flow requires either — see
 * `TwoFactorQrMatrix` below.
 */

/**
 * A QR code as a grid of dark/light modules, not as markup.
 *
 * research.md § R10 decided this: the server encodes the matrix, the client
 * draws rectangles. A server-rendered SVG *string* could only reach the page
 * through `dangerouslySetInnerHTML`, and a security screen is the last place
 * to introduce that. Drawing rects also means no QR dependency in the bundle.
 *
 * `modules` is one string of `0`/`1` per row, each `size` characters long.
 *
 * The server does not produce this field yet. When it does, nothing in the
 * client changes: `TwoFactorEnrolmentPanel` already renders it when present
 * and falls back to the typeable secret when it is absent, which is what
 * contract rule 6 requires ("a failure to build the QR is not a failure to
 * enrol").
 */
export interface TwoFactorQrMatrix {
  size: number;
  modules: readonly string[];
}

/** An enrolment that has been started but not yet proved by a code. */
export interface PendingTwoFactorEnrolment {
  /**
   * The full `otpauth://totp/...` URI. This is what a QR code would encode,
   * and what a person can paste into a desktop authenticator or password
   * manager that accepts a URI directly.
   */
  otpauthUrl: string;
  /**
   * The same secret the URI carries, grouped in fours so it can be read off a
   * screen and typed into an authenticator by hand (FR-002). Whitespace is
   * cosmetic; the server never receives a secret back at all.
   */
  secret: string;
  /** Present only once the server encodes one. See `TwoFactorQrMatrix`. */
  qr: TwoFactorQrMatrix | null;
}

/** The one response that ever carries recovery-code plaintext (FR-006/FR-009). */
export interface TwoFactorConfirmation {
  confirmedAt: string | null;
  /**
   * FR-020: true when a login challenge authorised the enrolment, in which
   * case the server issued a session cookie with this response and the
   * sign-in that was interrupted is finished.
   */
  signedIn: boolean;
  recoveryCodes: readonly string[];
  recoveryCodesNotice: string;
}

/**
 * What the account holder's own security page reads (FR-005).
 *
 * `GET /api/authentication/2fa/status` is specified in
 * `contracts/enrolment.md` but is not routed in `src/server/src/auth/mod.rs`
 * yet, so `readTwoFactorStatus` resolves to `null` on this instance rather
 * than throwing. The page says so instead of guessing.
 */
export interface TwoFactorStatus {
  enabled: boolean;
  confirmedAt: string | null;
  recoveryCodesRemaining: number | null;
  recoveryCodesLow: boolean;
  enrolmentPending: boolean;
}

/**
 * One entry in this account's second-factor history (FR-015).
 *
 * `bySomeoneElse` rather than an actor: "an administrator did this" is what
 * the account holder needs and can act on. *Which* administrator is an
 * operator's question, asked of an operator's surface.
 */
export interface TwoFactorHistoryEntry {
  occurredAt: string;
  eventType: string;
  bySomeoneElse: boolean;
}
