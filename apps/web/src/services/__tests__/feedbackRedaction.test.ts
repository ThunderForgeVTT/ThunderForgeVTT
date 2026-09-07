import { afterEach, describe, expect, it } from "vitest";
import {
  REDACTION_KINDS,
  redact,
  setSubmitterEmail,
} from "../feedbackRedaction";

/**
 * The rules FR-012 is judged against (spec 037, contracts/attachments.md § 2).
 *
 * Two kinds of assertion here, and both are needed. The first is that each
 * shape is caught — a rule that does not fire is not a rule. The second, and
 * the one that keeps this feature usable, is that ordinary log lines survive
 * intact: a redactor that ate every dotted identifier and every URL would
 * pass every "secret removed" test and deliver a bug report with nothing in
 * it.
 */

afterEach(() => {
  setSubmitterEmail(null);
});

describe("the shared rule set", () => {
  it("declares every kind the contract names", () => {
    expect([...REDACTION_KINDS].sort()).toEqual(
      [
        "aws_key",
        "bearer_token",
        "cookie",
        "jwt",
        "pem",
        "submitter_email",
        "url_credential",
      ].sort(),
    );
  });
});

describe("redact", () => {
  it("takes an Authorization header and says so", () => {
    const result = redact(
      "POST /graphql failed, Authorization: Bearer sk-live-9f2ab7d41c0e",
    );

    expect(result.text).not.toContain("sk-live-9f2ab7d41c0e");
    expect(result.text).toContain("[redacted: bearer token]");
    expect(result.kinds).toContain("bearer_token");
    expect(result.count).toBe(1);
  });

  it("takes a bare bearer token with no header around it", () => {
    const result = redact("retrying with Bearer abcdef0123456789");

    expect(result.text).not.toContain("abcdef0123456789");
    expect(result.kinds).toContain("bearer_token");
  });

  it("takes cookie headers and the session cookie by name", () => {
    const header = redact("Set-Cookie: session=0191d4ac-3f1e; HttpOnly");
    expect(header.text).not.toContain("0191d4ac-3f1e");
    expect(header.text).toContain("[redacted: cookie]");

    const inline = redact("document.cookie → session=abc123; csrf_token=zzz9");
    expect(inline.text).not.toContain("abc123");
    expect(inline.text).not.toContain("zzz9");
    expect(inline.count).toBe(2);
  });

  it("takes a JWT wherever it appears", () => {
    const jwt =
      "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const result = redact(`ws close: token ${jwt} rejected`);

    expect(result.text).not.toContain("eyJhbGciOiJIUzI1NiJ9");
    expect(result.text).toContain("[redacted: token]");
    expect(result.kinds).toContain("jwt");
  });

  it("takes the credentials out of a presigned storage URL", () => {
    const result = redact(
      "GET https://rustfs.local/bucket/a.webp?X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20260907&X-Amz-Signature=deadbeefcafe 403",
    );

    expect(result.text).not.toContain("deadbeefcafe");
    expect(result.text).not.toContain("AKIAIOSFODNN7EXAMPLE");
    expect(result.text).toContain("[redacted: signed url]");
    // The URL is still recognisable as the thing that failed, which is the
    // whole reason the line is worth keeping.
    expect(result.text).toContain("https://rustfs.local/bucket/a.webp");
    expect(result.text).toContain("403");
  });

  it("takes AWS key shapes and a secret named as one", () => {
    const keyId = redact("using AKIAIOSFODNN7EXAMPLE for the write");
    expect(keyId.text).not.toContain("AKIAIOSFODNN7EXAMPLE");
    expect(keyId.text).toContain("[redacted: storage credential]");

    const secret = redact(
      "config: aws_secret_access_key=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
    );
    expect(secret.text).not.toContain("wJalrXUtnFEMI");
  });

  it("takes a whole PEM block, and a PEM that lost its end", () => {
    const whole = redact(
      "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA7f\nQnQ==\n-----END RSA PRIVATE KEY-----",
    );
    expect(whole.text).toBe("[redacted: private key]");

    // Truncation by the per-entry bound cuts the END line off. What is left
    // is still a key, and the rule's second alternative takes it.
    const truncated = redact(
      "key was -----BEGIN PRIVATE KEY-----\nMIIEpAIBAAKCAQEA7fQnQ",
    );
    expect(truncated.text).not.toContain("MIIEpAIBAAKCAQEA7fQnQ");
    expect(truncated.text).toContain("[redacted: private key]");
  });

  it("takes the submitter's own address once it knows it", () => {
    const before = redact("failed for player@example.com");
    expect(before.text).toContain("player@example.com");

    setSubmitterEmail("Player@Example.com");
    const after = redact("failed for player@example.com");

    expect(after.text).not.toContain("player@example.com");
    expect(after.text).toContain("[redacted: email]");
    expect(after.kinds).toContain("submitter_email");
  });

  it("forgets the address on sign-out rather than keeping it compiled in", () => {
    setSubmitterEmail("player@example.com");
    setSubmitterEmail(null);

    expect(redact("player@example.com").text).toContain("player@example.com");
  });

  it("never deletes silently — every removal leaves a visible marker", () => {
    const result = redact("Authorization: Bearer abc123def456");

    expect(result.text).toMatch(/\[redacted: [a-z ]+\]/);
    expect(result.text.length).toBeGreaterThan(0);
  });

  it("leaves an ordinary log line completely alone", () => {
    const lines = [
      "TypeError: Cannot read properties of undefined (reading 'sceneId')",
      "    at useCanvasEngine (http://127.0.0.1:5173/src/engine/bevy/useCanvasEngine.ts:118:24)",
      "GET /api/graphql 200 in 31ms",
      "world 0191d4ac-3f1e-7b02-9a11-2b6f0c1d4e55 loaded scene tavern-ground-floor",
      "react-dom.production.min.js hydration mismatch",
      "session restored",
    ];

    for (const line of lines) {
      const result = redact(line);
      expect(result.text, `redactor chewed a hole in: ${line}`).toBe(line);
      expect(result.count).toBe(0);
    }
  });

  it("is pure — the same input twice gives the same output", () => {
    const line = "Authorization: Bearer abc123 and again Bearer def456";

    expect(redact(line).text).toBe(redact(line).text);
    expect(redact(line).count).toBe(2);
  });
});
