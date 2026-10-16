# ADR-116: Shared World State Is a Context, Not a Query Library

**Date:** 2026-10-16
**Status:** Proposed
**Participants:** ThunderForgeVTT Team
**Related:** spec 100 (Play names its scene on every world page), spec 101 (smooth world navigation)

---

## Problem Statement

Every world page mounts its own `WorldSectionShell` and its own hooks, so a
page change reads the world, its members, the claim gate, the play state,
the resume scene and the interface pack again from nothing. The page
blanks to a full-screen loader, GM controls arrive late and Play's label
flickers. The app has no client state library. What should hold what a
tab already knows about a world?

## Decision

1. **One React context, `WorldChromeProvider`, mounted once at each app's
   root** (`apps/web/src/main.tsx`, `apps/demo/src/main.tsx`), inside
   `AuthProvider`.
2. **Per world id, stale-while-revalidate.** A page renders from the known
   value and refreshes it on mount. Two readers of one key share one
   request. A failed refresh keeps what was known; a server refusal or a
   `null` world replaces it.
3. **Hints, never grants.** A remembered role or claim only decides what
   renders first. The server checks every action, and the refresh corrects
   the view.
4. **Per user, per tab.** The store empties when the signed-in user
   changes. Nothing is persisted.
5. **The pure parts** (patching, joining reads, telling a refusal from a
   transport failure) live in `world-chrome-store.ts`, tested without a
   DOM.

## Alternatives

- **TanStack Query.** The general tool: dedupe, retries, invalidation,
  devtools. Six kinds of data on one family of pages does not pay for a new
  dependency and a second way of fetching beside the app's own hooks.
- **Zustand or another store.** Holds state, not the fetch-and-refresh
  half, which is most of the code here.
- **A module-level cache.** Invisible to React and survives a sign-out
  unless every caller remembers to clear it.

## Consequences

- A page may show data one refresh old for the moment the refresh takes.
- Pages that save the world or its members must write the answer to the
  store, or the next page shows the old value until its refresh lands.
- **Revisit** when the store holds more than about ten kinds of data, or
  needs invalidation across worlds, retries or polling beyond the pause's
  focus refresh: adopt a query library then, with the owner's approval.
