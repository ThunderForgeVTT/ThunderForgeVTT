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
import {
  deliverEvent,
  releaseEvents,
  subscribeToEvents,
} from "../backend/events";
import { runOperation, type OperationRequest } from "../backend/execute";
import {
  NOT_IN_DEMO_CODE,
  notInDemoMessage,
  reportNotInDemo,
} from "../backend/notInDemo";
import { importUvtt } from "../backend/mapImport";
import {
  currentViewer,
  demoState,
  flushSave,
  loadState,
  reloadSaved,
} from "../backend/state";
import {
  CHANNEL_NAME,
  connectTabs,
  oneAtATime,
  type Channel,
  type Locks,
} from "../backend/tabs";
import { installImageGuard } from "./images";
import { multipartOperation } from "./multipart";
import { svgToPng } from "./rasterize";
import { answerRest } from "./rest";
import { installSocketGuard } from "./socket";
import { isTelemetryPost } from "./telemetryPass";
import { refusingXmlHttpRequest } from "./xhr";

const BASE = import.meta.env.BASE_URL;
/** Long enough that the mutation's answer is read before its event arrives. */
const EVENT_DELAY_MS = 10;

const fetchStatic = window.fetch.bind(window);

/**
 * The browser's own `fetch`, for the demo's static files: what the telemetry
 * module reads its config with (spec 086).
 */
export const staticFetch: typeof fetch = fetchStatic;

/**
 * Spec 081: an operation on the world, as the member who asked. Saved before
 * it is answered, so a tab taking over the world has every answered change;
 * its events follow the answer.
 */
const runOnWorld = oneAtATime(async (viewer, operation: OperationRequest) => {
  demoState().viewer = viewer;
  const result = await runOperation(operation);
  flushSave();
  setTimeout(releaseEvents, EVENT_DELAY_MS);
  return result;
});

/** The visitor's other tabs, sharing the one world (spec 081 R6). */
const tabs = connectTabs({
  locks: (navigator as Navigator & { locks?: Locks }).locks,
  channel:
    typeof BroadcastChannel === "function"
      ? (new BroadcastChannel(CHANNEL_NAME) as unknown as Channel)
      : undefined,
  viewer: currentViewer,
  run: runOnWorld,
  takeOver: reloadSaved,
  deliver: deliverEvent,
  onReleased: (post) => {
    subscribeToEvents(post);
  },
});

/** The world, loaded once. Every answer waits for it. */
export const worldReady = loadState(fetchStatic, BASE).then(() => tabs.ready);

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

/** What a refused request is answered with, in the demo's own words. */
function refusalBody(what: string): string {
  return JSON.stringify({
    error: "not part of the demo",
    errors: [
      {
        message: notInDemoMessage(what),
        extensions: { code: NOT_IN_DEMO_CODE },
      },
    ],
  });
}

function refuse(what: string): Response {
  reportNotInDemo(what);
  return new Response(refusalBody(what), {
    status: 404,
    headers: { "content-type": "application/json" },
  });
}

/** The client's one `XMLHttpRequest`: the book importer's upload. */
const IMPORT = "Importing a book";

async function answerGraphQL(request: Request): Promise<Response> {
  const type = request.headers.get("content-type") ?? "";
  let operation: OperationRequest;
  if (type.includes("multipart/form-data")) {
    // An upload: the GraphQL multipart request, its file kept in the page.
    operation = await multipartOperation(await request.formData());
  } else if (type.includes("application/json")) {
    operation = (await request.json()) as OperationRequest;
  } else {
    return refuse(`A ${type || "bodiless"} GraphQL request`);
  }
  return json(await tabs.ask(operation));
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
  // Spec 086 FR-021: telemetry to the origin the served config named.
  if (isTelemetryPost(request.method, url)) {
    return fetchStatic(request);
  }
  if (url.origin !== window.location.origin) {
    return refuse("Reaching another website");
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
  const mapImport = /^\/api\/scenes\/([0-9a-f-]{36})\/import\/uvtt$/.exec(
    url.pathname,
  );
  if (request.method === "POST" && mapImport) {
    // The import writes the world, which only the tab holding it may do.
    if (!tabs.holds()) return refuse("Importing a map in a second tab");
    const form = await request.formData().catch(() => null);
    const answer = await importUvtt(mapImport[1], form, currentViewer());
    setTimeout(releaseEvents, EVENT_DELAY_MS);
    return json(answer.body, answer.status);
  }
  const answer = answerRest(
    request.method,
    url.pathname,
    BASE,
    fetchStatic,
    svgToPng,
    currentViewer(),
  );
  return answer ?? refuse("That part of the server");
}

window.fetch = demoFetch;

installSocketGuard(BASE);
installImageGuard();

window.XMLHttpRequest = refusingXmlHttpRequest(
  () => refusalBody(IMPORT),
  () => reportNotInDemo(IMPORT),
);

if ("sendBeacon" in navigator) {
  navigator.sendBeacon = () => false;
}
