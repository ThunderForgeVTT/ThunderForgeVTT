/**
 * Spec 074: the door, replaced.
 *
 * This module is imported before any of the app's, and by the time it has
 * run the page has no way to speak to a server: `fetch`, `WebSocket` and
 * `XMLHttpRequest` each end here. What they are asked for is answered in the
 * page (`backend/`), read from the demo's own static files, or refused with
 * the one "not part of the demo" answer. Nothing is forwarded.
 *
 * The app above this is the real one, unchanged: the store, the mutation
 * bridges, the event sync and the engine do not know. That is the point: a
 * guard at the door covers code that is written next year, and a list of
 * replaced modules would not.
 */
import { releaseEvents } from "../backend/events";
import { runOperation, type OperationRequest } from "../backend/execute";
import { NOT_IN_DEMO_CODE, reportNotInDemo } from "../backend/notInDemo";
import { loadState } from "../backend/state";
import { installImageGuard } from "./images";
import { svgToPng } from "./rasterize";
import { answerRest } from "./rest";
import { installSocketGuard } from "./socket";

const BASE = import.meta.env.BASE_URL;
/** Long enough that the mutation's answer is read before its event arrives. */
const EVENT_DELAY_MS = 10;

const fetchStatic = window.fetch.bind(window);

/** The world, loaded once. Every answer waits for it. */
export const worldReady = loadState(fetchStatic, BASE);

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function refuse(what: string): Response {
  reportNotInDemo(what);
  return json(
    {
      error: "not part of the demo",
      errors: [
        {
          message: `${what} is not part of the demo.`,
          extensions: { code: NOT_IN_DEMO_CODE },
        },
      ],
    },
    404,
  );
}

async function answerGraphQL(request: Request): Promise<Response> {
  const type = request.headers.get("content-type") ?? "";
  if (!type.includes("application/json")) {
    // The multipart form: an upload to instance storage (FR-013).
    return refuse("Uploading to an instance");
  }
  const operation = (await request.json()) as OperationRequest;
  const result = await runOperation(operation);
  setTimeout(releaseEvents, EVENT_DELAY_MS);
  return json(result);
}

async function demoFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  const request = new Request(input, init);
  const url = new URL(request.url);

  // Bytes already in the page.
  if (url.protocol === "blob:" || url.protocol === "data:") {
    return fetchStatic(request);
  }
  if (url.origin !== window.location.origin) {
    return refuse(url.host);
  }
  // The demo's own static files: the bundle, the engine, the maps.
  if (url.pathname.startsWith(BASE)) {
    return fetchStatic(request);
  }

  await worldReady;
  if (
    request.method === "POST" &&
    (url.pathname === "/api/graphql" || url.pathname === "/api/graphql/public")
  ) {
    return answerGraphQL(request);
  }
  const answer = answerRest(
    request.method,
    url.pathname,
    BASE,
    fetchStatic,
    svgToPng,
  );
  return answer ?? refuse(`${request.method} ${url.pathname}`);
}

window.fetch = demoFetch;

installSocketGuard(BASE);
installImageGuard();

/**
 * The client's one `XMLHttpRequest` is an upload with a progress bar. There
 * is nowhere for it to go.
 */
window.XMLHttpRequest = class {
  constructor() {
    reportNotInDemo("Uploading to an instance");
    throw new DOMException("not part of the demo", "NotSupportedError");
  }
} as unknown as typeof XMLHttpRequest;

if ("sendBeacon" in navigator) {
  navigator.sendBeacon = () => false;
}
