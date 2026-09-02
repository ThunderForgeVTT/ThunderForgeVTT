/**
 * The app's own modules, as the *browser* addresses them.
 *
 * Several specs reach into the running application from inside
 * `page.evaluate`, to ask the engine or the world store what it believes:
 *
 * ```ts
 * const bevy = (await import(
 *   "/src/engine/bevy/index.ts"
 * )) as typeof import("../../src/engine/bevy/index");
 * ```
 *
 * That string is a URL the Vite dev server serves, not a path TypeScript can
 * resolve — the code runs in the page, not in the test process. The
 * `as typeof import(...)` beside it is what actually types the result, and it
 * uses a real relative path, so the value is still checked properly.
 *
 * Without this, TypeScript reports "cannot find module" forty times over and
 * drowns every real error in the suite. That is not hypothetical: `e2e/` was
 * outside the typechecked set until spec 031, and the first thing checking it
 * found was a hand-copied `GmToolId` union that had drifted from the rail's.
 *
 * # The wildcard, and what it costs
 *
 * A per-path list would also catch a typo in one of these runtime specifiers.
 * It was tried first and is worse in practice: the list goes stale silently,
 * and a missing entry produces exactly the noise this file exists to remove.
 *
 * The trade is acceptable because these specifiers are checked another way —
 * a wrong path fails immediately and loudly in the browser, in the one test
 * that uses it, rather than corrupting anything. And the `as typeof import()`
 * cast beside each one still names a real module, so the *shape* of what comes
 * back is verified even though the URL is not.
 */

declare module "/src/*";
declare module "/packs/*";
