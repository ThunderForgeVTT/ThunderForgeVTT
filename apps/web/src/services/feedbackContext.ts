/**
 * The context a feedback submission carries because the app already knows it
 * (spec 037, FR-009).
 *
 * # Why this is assembled here and not read from a store
 *
 * There is no `WorldContext` and no `useWorld` hook in this app: `worldId`
 * comes from `useParams()` inside a route and is threaded as a prop, and the
 * world record is local `useState` inside `WorldPage`. The feedback launcher
 * is mounted *above* the router (FR-001 — "any screen"), so it can reach
 * neither. research.md § R13 settles it: the context is whatever a component
 * above every route can honestly see — the path, the world id that path
 * names, the build, and a coarse description of the browser.
 *
 * The **game system is deliberately absent**. `contracts/feedback.md` rule 3:
 * the server resolves it from `worldId`, because a client that could name the
 * system could name a different world's.
 */

/**
 * What the app knows about where the person was standing.
 *
 * Every field is nullable except the two the contract declares non-null;
 * a screen with no world simply carries no world, which spec.md's first Edge
 * Case requires to keep working.
 */
export interface FeedbackContext {
  /** The route the person was on. `null` is valid — an error state may have none. */
  screenPath: string | null;
  /** Absent on any screen with no world. */
  worldId: string | null;
  /** The client build. `"unknown build"` when nothing defined one. */
  clientVersion: string;
  /** Coarse family and platform. Never the full User-Agent, never an address. */
  browser: string;
}

/** What a removed field is replaced by, rather than a lie or an empty string. */
export const WITHHELD = "withheld by submitter";

/** Rendered when no build identifier was compiled in, rather than omitted. */
export const UNKNOWN_BUILD = "unknown build";

/**
 * The world id the current path names, or `null`.
 *
 * A regular expression rather than `useParams()` on purpose: a component
 * mounted above `<Routes>` has matched no route, so `useParams()` there is
 * always empty. The path is the only thing that is true at that height.
 */
export function worldIdFromPath(pathname: string): string | null {
  const match = /^\/world\/([0-9a-fA-F-]{36})(?:\/|$)/.exec(pathname);
  return match ? match[1] : null;
}

interface BrowserFamily {
  name: string;
  /** Matched in order; the first hit wins, so the impostors come first. */
  pattern: RegExp;
}

/**
 * Ordered because every Chromium browser claims to be Chrome and Safari
 * claims to be everything. Edge and Opera are tested before Chrome, and
 * Chrome before Safari, for that reason alone.
 */
const FAMILIES: BrowserFamily[] = [
  { name: "Edge", pattern: /Edg[A-Z]?\/(\d+)/ },
  { name: "Opera", pattern: /OPR\/(\d+)/ },
  { name: "Firefox", pattern: /Firefox\/(\d+)/ },
  { name: "Chrome", pattern: /Chrome\/(\d+)/ },
  { name: "Safari", pattern: /Version\/(\d+).*Safari/ },
];

const PLATFORMS: BrowserFamily[] = [
  { name: "Windows", pattern: /Windows/ },
  { name: "Android", pattern: /Android/ },
  { name: "iOS", pattern: /iPhone|iPad|iPod/ },
  { name: "macOS", pattern: /Mac OS X|Macintosh/ },
  { name: "Linux", pattern: /Linux|X11/ },
];

/**
 * A coarse description — "Chrome 141 on Linux".
 *
 * The full User-Agent is not sent, and this is the reason it is a function
 * rather than a field: a User-Agent carries a build, a device model and often
 * an OEM string, and a maintainer reproducing a bug needs the family and the
 * major version, which is all this returns. Anything unrecognised becomes
 * "an unrecognised browser" rather than the raw string, so a novel agent
 * cannot leak itself through the default case.
 */
export function describeBrowser(userAgent: string): string {
  const family = FAMILIES.find((entry) => entry.pattern.test(userAgent));
  const platform = PLATFORMS.find((entry) => entry.pattern.test(userAgent));

  const name = family
    ? `${family.name} ${family.pattern.exec(userAgent)?.[1] ?? ""}`.trim()
    : "an unrecognised browser";

  return platform ? `${name} on ${platform.name}` : name;
}

/**
 * The build this bundle was compiled from.
 *
 * `VITE_APP_VERSION` is read rather than a `define`d global so that no build
 * configuration change is required for the value to be *readable*; until
 * something sets it, this honestly says the build is unknown rather than
 * inventing one. research.md § R12 wants the server's own version recorded
 * alongside it, which is the server's half of the same field.
 */
export function clientVersion(): string {
  const declared = import.meta.env.VITE_APP_VERSION;
  return declared !== undefined && declared !== "" ? declared : UNKNOWN_BUILD;
}

/** Everything above, gathered at the moment the form is opened. */
export function collectFeedbackContext(pathname: string): FeedbackContext {
  const userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent;

  return {
    screenPath: pathname === "" ? null : pathname,
    worldId: worldIdFromPath(pathname),
    clientVersion: clientVersion(),
    browser: describeBrowser(userAgent),
  };
}
