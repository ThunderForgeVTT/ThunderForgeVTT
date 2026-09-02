import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  GraphQLRequestError,
  operationNameOf,
  postGraphQL,
  postGraphQLMultipart,
} from "@/api/graphqlClient";

/**
 * Covers the failure modes the 23 duplicated `postGraphQL` copies handled
 * badly. These exist so migrating the rest of `src/api/` onto this client is a
 * verified change rather than a hopeful one — `apps/web` has no other coverage
 * of the transport layer.
 */

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function textResponse(body: string, status = 200): Response {
  return new Response(body, {
    status,
    headers: { "content-type": "text/html" },
  });
}

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const QUERY = `query WorldAbilities($worldId: UUID!) { worldAbilities(worldId: $worldId) { id } }`;

describe("postGraphQL — happy path", () => {
  it("returns data and posts the query and variables", async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ data: { worldAbilities: [{ id: "a" }] } }),
    );

    const data = await postGraphQL<{ worldAbilities: { id: string }[] }>(
      QUERY,
      {
        worldId: "w1",
      },
    );

    expect(data.worldAbilities).toEqual([{ id: "a" }]);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/graphql");
    expect(init.method).toBe("POST");
    expect(init.credentials).toBe("same-origin");
    expect(JSON.parse(init.body as string)).toEqual({
      query: QUERY,
      variables: { worldId: "w1" },
    });
  });

  it("honours a custom endpoint (the moderation surface's one real divergence)", async () => {
    fetchMock.mockResolvedValue(jsonResponse({ data: { ok: true } }));
    await postGraphQL(QUERY, undefined, { endpoint: "/api/admin/graphql" });
    expect(fetchMock.mock.calls[0][0]).toBe("/api/admin/graphql");
  });
});

describe("postGraphQL — non-JSON responses", () => {
  it("reports the HTTP status instead of a JSON parse error", async () => {
    // The old copies did `await response.json()` unconditionally, so this
    // surfaced as `SyntaxError: Unexpected token '<'`.
    fetchMock.mockResolvedValue(
      textResponse("<html>502 Bad Gateway</html>", 502),
    );

    const err = await postGraphQL(QUERY).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(GraphQLRequestError);
    expect((err as Error).message).toContain("502");
    expect((err as Error).message).not.toMatch(/JSON|token/i);
  });

  it("handles an empty body without throwing a parse error", async () => {
    // 200 with an empty body: what a proxy or a dropped upstream produces.
    fetchMock.mockResolvedValue(new Response("", { status: 200 }));
    const err = await postGraphQL(QUERY).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(GraphQLRequestError);
    expect((err as Error).message).not.toMatch(/JSON|token/i);
  });

  it("reports an unreadable body even on a 200", async () => {
    fetchMock.mockResolvedValue(textResponse("not json at all", 200));
    await expect(postGraphQL(QUERY)).rejects.toThrow(/unreadable/i);
  });
});

describe("postGraphQL — GraphQL errors", () => {
  it("surfaces ALL error messages, not just the first", async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({
        errors: [{ message: "first problem" }, { message: "second problem" }],
      }),
    );

    const err = (await postGraphQL(QUERY).catch(
      (e: unknown) => e,
    )) as GraphQLRequestError;
    expect(err.message).toContain("first problem");
    expect(err.message).toContain("second problem");
    expect(err.errors).toEqual(["first problem", "second problem"]);
  });

  it("attaches the operation name and status for diagnosis", async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ errors: [{ message: "nope" }] }, 400),
    );
    const err = (await postGraphQL(QUERY).catch(
      (e: unknown) => e,
    )) as GraphQLRequestError;
    expect(err.operation).toBe("WorldAbilities");
    expect(err.status).toBe(400);
  });

  it("prefers a GraphQL message over the bare HTTP status", async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ errors: [{ message: "Ability not found" }] }, 500),
    );
    await expect(postGraphQL(QUERY)).rejects.toThrow("Ability not found");
  });

  it("throws rather than returning partial data when both are present", async () => {
    // Deliberate: callers are typed as receiving complete results.
    fetchMock.mockResolvedValue(
      jsonResponse({
        data: { worldAbilities: [] },
        errors: [{ message: "partial failure" }],
      }),
    );
    await expect(postGraphQL(QUERY)).rejects.toThrow("partial failure");
  });

  it("ignores blank error messages rather than throwing an empty string", async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ errors: [{ message: "   " }] }, 500),
    );
    const err = (await postGraphQL(QUERY).catch((e: unknown) => e)) as Error;
    expect(err.message.trim()).not.toBe("");
    expect(err.message).toContain("500");
  });

  it("rejects a 200 with no data and no errors", async () => {
    fetchMock.mockResolvedValue(jsonResponse({}));
    await expect(postGraphQL(QUERY)).rejects.toThrow(/did not include data/);
  });
});

describe("postGraphQL — transport failures", () => {
  it("turns a rejected fetch into a readable message", async () => {
    fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
    const err = (await postGraphQL(QUERY).catch(
      (e: unknown) => e,
    )) as GraphQLRequestError;
    expect(err).toBeInstanceOf(GraphQLRequestError);
    expect(err.message).toMatch(/could not reach the server/i);
    expect(err.operation).toBe("WorldAbilities");
  });

  it("times out rather than hanging forever", async () => {
    fetchMock.mockImplementation((_url: string, init: RequestInit) => {
      return new Promise((_resolve, reject) => {
        init.signal?.addEventListener("abort", () => {
          reject(new DOMException("Aborted", "AbortError"));
        });
      });
    });

    await expect(
      postGraphQL(QUERY, undefined, { timeoutMs: 10 }),
    ).rejects.toThrow(/timed out/i);
  });

  it("passes an abort signal so the timeout can actually cancel", async () => {
    fetchMock.mockResolvedValue(jsonResponse({ data: { ok: true } }));
    await postGraphQL(QUERY);
    expect(fetchMock.mock.calls[0][1].signal).toBeDefined();
  });

  it("omits the signal when the timeout is disabled", async () => {
    fetchMock.mockResolvedValue(jsonResponse({ data: { ok: true } }));
    await postGraphQL(QUERY, undefined, { timeoutMs: null });
    expect(fetchMock.mock.calls[0][1].signal).toBeUndefined();
  });
});

describe("postGraphQLMultipart", () => {
  const UPLOAD = `mutation UploadLoreImage($loreEntryId: UUID!, $file: Upload!) { uploadLoreImage(loreEntryId: $loreEntryId, file: $file) { id } }`;

  it("builds a spec-compliant multipart body and omits Content-Type", async () => {
    fetchMock.mockResolvedValue(
      jsonResponse({ data: { uploadLoreImage: { id: "i1" } } }),
    );

    await postGraphQLMultipart(
      UPLOAD,
      { loreEntryId: "l1" },
      new Blob(["x"]),
      "file",
    );

    const init = fetchMock.mock.calls[0][1] as RequestInit;
    const form = init.body as FormData;
    expect(JSON.parse(form.get("map") as string)).toEqual({
      "0": ["variables.file"],
    });
    const ops = JSON.parse(form.get("operations") as string);
    // The file slot must be null in `variables`; the map points at it.
    expect(ops.variables).toEqual({ loreEntryId: "l1", file: null });
    expect(form.get("0")).toBeInstanceOf(Blob);
    // The browser must set the boundary itself.
    expect(JSON.stringify(init.headers ?? {})).not.toMatch(/content-type/i);
  });

  it("has no timeout by default — a big upload on a slow link is not a fault", async () => {
    fetchMock.mockResolvedValue(jsonResponse({ data: { ok: true } }));
    await postGraphQLMultipart(UPLOAD, {}, new Blob(["x"]), "file");
    expect(fetchMock.mock.calls[0][1].signal).toBeUndefined();
  });

  it("reports upload failures with the status when the body is not JSON", async () => {
    fetchMock.mockResolvedValue(textResponse("<html>413</html>", 413));
    await expect(
      postGraphQLMultipart(UPLOAD, {}, new Blob(["x"]), "file"),
    ).rejects.toThrow(/Upload failed.*413/);
  });
});

describe("operationNameOf", () => {
  it("extracts query, mutation, and subscription names", () => {
    expect(operationNameOf("query WorldAbilities($a: ID!) { x }")).toBe(
      "WorldAbilities",
    );
    expect(operationNameOf("mutation CreateAbility($input: X!) { y }")).toBe(
      "CreateAbility",
    );
    expect(
      operationNameOf("subscription WorldEventsCreated($w: String!) { z }"),
    ).toBe("WorldEventsCreated");
  });

  it("returns undefined for an anonymous operation rather than guessing", () => {
    expect(operationNameOf("{ worldAbilities { id } }")).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// CSRF (found by the 2026-09-02 mutation audit)
// ---------------------------------------------------------------------------

/**
 * Nothing asserted that this client sends a CSRF token.
 *
 * The audit replaced **both** `withCsrf(...)` call sites in `graphqlClient.ts`
 * with plain header literals and the whole suite stayed green — every mutation
 * this app makes would have been rejected by the server, and no test would
 * have noticed. It was found from the other direction on the same day: an e2e
 * hand-rolled a `fetch` to `/api/graphql`, omitted the token, and got a 401
 * with an empty body.
 *
 * `withCsrf` reads the `csrf_token` cookie, so these set and clear one.
 */
describe("CSRF", () => {
  // This suite runs under vitest's `node` environment — there is no `document`
  // — so the cookie is stubbed rather than set. `withCsrf` reads
  // `document.cookie` and nothing else, which is exactly the seam to stub.
  const withCookie = (cookie: string) => vi.stubGlobal("document", { cookie });

  it("sends the token from the cookie on a query", async () => {
    withCookie("csrf_token=a-real-token");
    fetchMock.mockResolvedValue(jsonResponse({ data: { worldAbilities: [] } }));

    await postGraphQL(QUERY, { worldId: "w" });

    const headers = fetchMock.mock.calls[0][1].headers as Record<
      string,
      string
    >;
    expect(headers["x-csrf-token"]).toBe("a-real-token");
  });

  it("sends it on a multipart upload too", async () => {
    withCookie("csrf_token=a-real-token");
    fetchMock.mockResolvedValue(jsonResponse({ data: { upload: true } }));

    await postGraphQLMultipart(
      QUERY,
      { worldId: "w" },
      new File(["x"], "x.png", { type: "image/png" }),
      "variables.file",
    );

    const headers = fetchMock.mock.calls[0][1].headers as Record<
      string,
      string
    >;
    expect(headers["x-csrf-token"]).toBe("a-real-token");
  });

  /**
   * No cookie is not a crash. A first request before the session exists has
   * nothing to send, and the server answers that on its own terms.
   */
  it("sends no token header when there is no cookie", async () => {
    withCookie("");
    fetchMock.mockResolvedValue(jsonResponse({ data: { worldAbilities: [] } }));

    await postGraphQL(QUERY, { worldId: "w" });

    const headers = fetchMock.mock.calls[0][1].headers as Record<
      string,
      string
    >;
    expect(headers["x-csrf-token"]).toBeUndefined();
  });
});
