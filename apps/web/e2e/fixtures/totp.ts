import { createHmac } from "node:crypto";

/**
 * A TOTP code, computed the way an authenticator app would.
 *
 * Extracted from `two-factor-enrolment.spec.ts` when the first-run setup spec
 * needed the same thing: setup cannot complete without a confirmed second
 * factor (spec 040 FR-002a), so *every* first-run test has to enrol one, and a
 * second copy of an RFC 6238 implementation is a second place for it to be
 * subtly wrong.
 */

/** RFC 4648 base32, unpadded — what the provisioning URI carries. */
export function decodeBase32(secret: string): Buffer {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = 0;
  let value = 0;
  const out: number[] = [];
  for (const char of secret.toUpperCase().replace(/=+$/, "")) {
    const index = alphabet.indexOf(char);
    if (index < 0) {
      throw new Error(`not base32: ${char}`);
    }
    value = (value << 5) | index;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      out.push((value >>> bits) & 0xff);
    }
  }
  return Buffer.from(out);
}

/** RFC 6238, matching `thunderforge-axum-auth-core`: SHA1, 6 digits, 30s. */
export function totpAt(secretBase32: string, unixSeconds: number): string {
  const counter = Buffer.alloc(8);
  counter.writeBigUInt64BE(BigInt(Math.floor(unixSeconds / 30)));
  const digest = createHmac("sha1", decodeBase32(secretBase32))
    .update(counter)
    .digest();
  const offset = digest[digest.length - 1] & 0x0f;
  const binary =
    ((digest[offset] & 0x7f) << 24) |
    (digest[offset + 1] << 16) |
    (digest[offset + 2] << 8) |
    digest[offset + 3];
  return (binary % 1_000_000).toString().padStart(6, "0");
}

/** The code for right now. */
export function totpNow(secretBase32: string): string {
  return totpAt(secretBase32, Math.floor(Date.now() / 1000));
}
