# Feature Specification: A Board That Says It Stopped

**Feature Branch**: `069-a-board-you-can-touch`
**Created**: 2026-10-04
**Status**: Draft
**Input**: The engine readiness review of 2026-10-04: a crash or a lost graphics context in the middle of a session is silent.

## Why

Two things end a running board. The engine panics — in wasm a trap, after
which the frame loop never runs and every call into the module throws. Or the
browser takes the graphics context: a driver reset, a laptop switching GPUs,
a phone reclaiming memory from a background tab. Either way the canvas keeps
its last frame. The page showed a live-looking map that ignored every click,
and the only evidence was in a console nobody at a table has open.

A failed *load* already had an answer (spec 028: an error and a retry). A
board that dies after it started had none.

## Decisions already made

- **The page tells the person, the engine tells the page.** The panic hook
  sends the window one DOM event; a notice is chrome.
- **Reload is the only action offered.** The engine module starts once per
  page, and a restored graphics context is one the engine cannot pick up.
  Offering "try again" in place would be offering something that does not
  work.
- **The notice says nothing is lost**, because that is true — the table is
  the server's, not the page's — and it is what somebody mid-fight needs to
  hear before they press anything.
- **The rest of the page stays up.** Chat, sheets and the dock do not depend
  on the engine and are not covered.
- **A real crash is the proof.** Debug engine builds export `debug_panic`;
  release builds do not, so no shipped board can be asked to crash.

## Requirements

- **FR-001** On panic the engine logs as before and dispatches
  `thunderforge:engine-stopped` on `window`.
- **FR-002** The web app records why the engine stopped — `crashed` on that
  event, `context-lost` on `webglcontextlost` — keeps the first reason, and
  keeps it whether or not the board's page was mounted at the time.
- **FR-003** While stopped, the board carries an alert: the board stopped,
  why in a sentence, nothing is lost, and a Reload button that reloads the
  page.

## Success Criteria

- **SC-001** Losing the engine's real graphics context shows the notice;
  pressing Reload brings the board back with the token that was on it where
  it was.
- **SC-002** A real engine panic shows the notice.

## What this spec does not do

- It does not recover in place, and does not auto-reload: a reload nobody
  asked for, mid-typing in chat, is worse than a notice.
- It does not report the crash anywhere. Telemetry is deferred by the owner;
  the feedback form still captures the console.
- It does not make the SDK calls that are swallowed by a dead engine fail
  loudly. With the notice up, they have nothing left to say.
