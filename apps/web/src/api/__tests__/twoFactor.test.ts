import { afterEach, describe, expect, it, vi } from "vitest";

import {
  beginTwoFactorEnrolment,
  confirmTwoFactorEnrolment,
  groupSecretForTyping,
  readTwoFactorStatus,
  secretFromOtpauthUri,
  TwoFactorRequestError,
} from "../twoFactor";
import * as twoFactorApi from "../twoFactor";

/**
 * Spec 041 US1 — what the client does with what the server actually sends.
 *
 * The interesting cases here are all ones where the contract
 * (`specs/041-two-factor-enrolment/contracts/enrolment.md`) is ahead of
 * `src/server/src/auth/two_factor.rs`: no `secret` field, no `qr` matrix, no
 * `/2fa/status` route. Each has a test proving the flow still works, because
 * "the QR is a convenience, the secret is the credential" (contract rule 6)
 * has to be true in the code and not only in the prose.
 */

const CREDENTIALS = { username: "wizard", password: "correct horse" };

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function stubFetch(...responses: Response[]) {
  const fetchMock = vi.fn<typeof fetch>();
  for (const response of responses) {
    fetchMock.mockResolvedValueOnce(response);
  }
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("secretFromOtpauthUri / groupSecretForTyping (FR-002)", () => {
  it("recovers the base32 secret the server put in the URI", () => {
    expect(
      secretFromOtpauthUri(
        "otpauth://totp/ThunderForge:wizard?secret=GEZDGNBVGY3TQOJQ&issuer=ThunderForge",
      ),
    ).toBe("GEZDGNBVGY3TQOJQ");
  });

  it("answers null rather than guessing when there is no secret", () => {
    expect(secretFromOtpauthUri("otpauth://totp/ThunderForge:wizard")).toBe(
      null,
    );
    expect(
      secretFromOtpauthUri("otpauth://totp/ThunderForge:wizard?issuer=TF"),
    ).toBe(null);
  });

  it("groups in fours so it can be typed off a screen", () => {
    expect(groupSecretForTyping("GEZDGNBVGY3TQOJQ")).toBe(
      "GEZD GNBV GY3T QOJQ",
    );
    // Already grouped, or lower case, or both — same answer either way.
    expect(groupSecretForTyping("gezd gnbv gy3t qojq")).toBe(
      "GEZD GNBV GY3T QOJQ",
    );
    // No trailing space on an exact multiple of four.
    expect(groupSecretForTyping("ABCD")).toBe("ABCD");
    expect(groupSecretForTyping("ABCDE")).toBe("ABCD E");
  });
});

describe("beginTwoFactorEnrolment", () => {
  it("derives the typeable secret from the URI the server sends today", async () => {
    stubFetch(
      jsonResponse({
        status: "success",
        message: "2FA secret generated.",
        otpauth_url:
          "otpauth://totp/ThunderForge:wizard?secret=GEZDGNBVGY3TQOJQ&issuer=ThunderForge",
      }),
    );

    const enrolment = await beginTwoFactorEnrolment(CREDENTIALS);

    // Both forms, always (FR-002) — even though the server sends only one.
    expect(enrolment.secret).toBe("GEZD GNBV GY3T QOJQ");
    expect(enrolment.otpauthUrl).toContain("secret=GEZDGNBVGY3TQOJQ");
    // No QR today, and that is not an error: contract rule 6.
    expect(enrolment.qr).toBe(null);
  });

  it("prefers the server's own grouped secret once it sends one", async () => {
    stubFetch(
      jsonResponse({
        status: "success",
        otpauth_url:
          "otpauth://totp/ThunderForge:wizard?secret=GEZDGNBVGY3TQOJQ&issuer=ThunderForge",
        secret: "GEZD GNBV GY3T QOJQ",
      }),
    );

    await expect(beginTwoFactorEnrolment(CREDENTIALS)).resolves.toMatchObject({
      secret: "GEZD GNBV GY3T QOJQ",
    });
  });

  it("accepts a module matrix when there is one", async () => {
    stubFetch(
      jsonResponse({
        status: "success",
        otpauth_url: "otpauth://totp/ThunderForge:wizard?secret=AAAA",
        qr: { size: 3, modules: ["101", "010", "101"] },
      }),
    );

    await expect(beginTwoFactorEnrolment(CREDENTIALS)).resolves.toMatchObject({
      qr: { size: 3, modules: ["101", "010", "101"] },
    });
  });

  it("drops a matrix that is not square rather than drawing nonsense", async () => {
    stubFetch(
      jsonResponse({
        status: "success",
        otpauth_url: "otpauth://totp/ThunderForge:wizard?secret=AAAA",
        qr: { size: 3, modules: ["101", "01"] },
      }),
    );

    // A QR that cannot be built is still an enrolment that can be completed.
    await expect(beginTwoFactorEnrolment(CREDENTIALS)).resolves.toMatchObject({
      qr: null,
      secret: "AAAA",
    });
  });

  it("keeps the server's status on a refusal", async () => {
    stubFetch(
      jsonResponse({ status: "failure", message: "Invalid credentials" }, 401),
    );

    await expect(beginTwoFactorEnrolment(CREDENTIALS)).rejects.toMatchObject({
      status: "failure",
      message: "Invalid credentials",
    });
  });
});

describe("confirmTwoFactorEnrolment", () => {
  it("returns the recovery codes the one time they exist", async () => {
    stubFetch(
      jsonResponse({
        status: "success",
        message: "2FA enabled",
        confirmed_at: "2026-09-07T12:04:11",
        recovery_codes: ["4KJH-92MX-QW3T", "8PLM-31QA-ZX9V"],
        recovery_codes_notice: "Each works once.",
      }),
    );

    await expect(
      confirmTwoFactorEnrolment(CREDENTIALS, "492013"),
    ).resolves.toEqual({
      confirmedAt: "2026-09-07T12:04:11",
      recoveryCodes: ["4KJH-92MX-QW3T", "8PLM-31QA-ZX9V"],
      recoveryCodesNotice: "Each works once.",
    });
  });

  it("strips the whitespace a password manager pastes into the code", async () => {
    const fetchMock = stubFetch(
      jsonResponse({ status: "success", recovery_codes: [] }),
    );

    await confirmTwoFactorEnrolment(CREDENTIALS, "492 013");

    const body = String((fetchMock.mock.calls[0]?.[1] as RequestInit).body);
    expect(JSON.parse(body)).toMatchObject({ code: "492013" });
  });

  it("raises a wrong code as a refusal that names itself", async () => {
    stubFetch(
      jsonResponse(
        { status: "two_factor_invalid", message: "Invalid 2FA code" },
        401,
      ),
    );

    const error = await confirmTwoFactorEnrolment(CREDENTIALS, "000000").catch(
      (thrown: unknown) => thrown,
    );

    expect(error).toBeInstanceOf(TwoFactorRequestError);
    expect(error).toMatchObject({ status: "two_factor_invalid" });
  });

  it("has no companion that could fetch the codes again (FR-009)", () => {
    // The guarantee is structural: the module exports nothing that reads
    // recovery codes, because the server has no route that returns them a
    // second time. This test fails the day somebody adds one.
    const readers = Object.keys(twoFactorApi).filter(
      (name) => /recovery/i.test(name) && /get|read|fetch|list/i.test(name),
    );
    expect(readers).toEqual([]);
  });
});

describe("readTwoFactorStatus (FR-005)", () => {
  it("reads the status route when the instance has one", async () => {
    stubFetch(
      jsonResponse({
        enabled: true,
        confirmed_at: "2026-09-07T12:04:11",
        recovery_codes_remaining: 8,
        recovery_codes_low: false,
        enrolment_pending: false,
      }),
    );

    await expect(readTwoFactorStatus()).resolves.toMatchObject({
      enabled: true,
      confirmedAt: "2026-09-07T12:04:11",
      recoveryCodesRemaining: 8,
    });
  });

  it("answers null on an instance whose server does not route it yet", async () => {
    // Which is every instance today: `/authentication/2fa/status` is in the
    // contract and not in `src/server/src/auth/mod.rs`. The panel says it
    // cannot tell rather than claiming two-factor is off.
    stubFetch(new Response("Not Found", { status: 404 }));
    await expect(readTwoFactorStatus()).resolves.toBe(null);
  });

  it("answers null rather than throwing when the request itself fails", async () => {
    const fetchMock = vi.fn<typeof fetch>();
    fetchMock.mockRejectedValueOnce(new Error("offline"));
    vi.stubGlobal("fetch", fetchMock);

    await expect(readTwoFactorStatus()).resolves.toBe(null);
  });
});
