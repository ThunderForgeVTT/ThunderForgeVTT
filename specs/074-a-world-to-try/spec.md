# Feature Specification: A World To Try

**Feature Branch**: `074-a-world-to-try`
**Created**: 2026-10-05
**Status**: Draft
**Input**: The owner, 2026-10-05, while testing the dev instance: get rid of `/counter`, replace it with a real `/demo`, make it look like a world dashboard and lead into a real play field with full Game Master powers, keep every byte of its data in the browser, build it as its own app under `apps/demo`, and ship it as a static folder inside the image. "Enter demo workspace" already exists as a link; it should become a real thing.

## Why

Three things in the app are called a demo and none is one.

`/counter` is a component gallery with a click counter on it. It is linked
from the header as "Preview" and, during setup, as "Status". It is also,
by accident, the only page that holds the account's own controls: export my
data, delete my account, sign out.

"Enter demo workspace" is in the header menu and the site links. It goes to
`/world/demo-world/play`, which asks the server for a world called
`demo-world`. There is no such world. The link is a promise the app does not
keep.

And somebody who has heard of ThunderForge and wants to know what it is like
has to be invited to an instance, make an account and be handed a world
before they can move one token.

A demo answers all three: a world that anybody can open, run as its Game
Master, and wreck, because nothing they do leaves their browser.

It answers a fourth for us. A client that runs with no server is a client
whose features can be exercised with no database, no stack and no session,
which is most of what makes an end-to-end run slow.

## Is it feasible

Yes, and the reason is how few doors the client has. Counted on 2026-10-05:

- Every GraphQL request goes through one function, `postGraphQL` in
  `apps/web/src/api/graphqlClient.ts` (and its multipart sibling).
- Live events come through two `graphql-ws` clients:
  `engine/world/sync/subscriptionClient.ts` and one in
  `engine/bevy/index.ts`.
- About forty-five plain `fetch` calls, nearly all of them sign-in, second
  factor and setup, which a demo has no use for; the rest are the game-system
  and interface-pack manifests, which are already static files.
- The engine fetches its own map and token art, and has networking modules of
  its own under `crates/thunderforge-engine/src/network/` and
  `plugins/cached_assets/wasm_sync.rs`.

What it costs is behind the first door: the client speaks about 420 named
GraphQL operations, and the server that answers them is Rust over PostgreSQL
and cannot be compiled into a page. The demo needs a second, small
implementation of the part of that contract a world and a play field use.
That is the work, and it is the part to keep honest (FR-009, FR-010).

## Decisions already made

- **`/counter` goes.** The route, the page, its header entries and its
  prefetch entries are removed. Nothing redirects to it and nothing is kept
  of the gallery.
- **The account's controls live under the account.** Export, deletion and
  sign-out move to a page of the person's own settings, beside security,
  storage, standing and feedback. This part needs no demo and ships first.
- **The demo is its own app, `apps/demo`.** A thin entry point: its own
  `index.html`, router and Vite build, importing the web app's pages and
  components rather than copying them, and replacing the modules that reach a
  server. It is where anything the demo must do differently is overridden.
  Moving the web app's source into `packages/` so two apps can share it
  properly is the right shape and is not this spec; the demo imports from
  `apps/web/src` until it is done.
- **The demo is replaced at the door, not behind it.** The store, the
  mutation bridges, the event sync and the engine run as they do in the real
  app, unchanged. Only the transport is swapped: an in-page backend answers
  the GraphQL operations and emits the world events a server would have. The
  circular flow in `AGENTS.md` still holds; the authority it circles through
  is in the tab.
- **Nothing in the demo can write to an instance, by construction.** The
  demo renders the real client's pages, so the modules that call `/api` are
  in its bundle; what they call is not. Before any of that code runs, the
  demo replaces the page's `fetch`, `WebSocket`, `XMLHttpRequest` and
  `sendBeacon` with its own, which answer from the in-page backend, serve the
  demo's own static files, and refuse everything else. The built page also
  carries a content security policy that forbids loading from any other
  origin. This is a property of the bundle and is tested as one (SC-003), not
  a mode the real client is trusted to stay in.
- **The visitor is the Game Master.** One person, full powers, no account, no
  sign-in. There is nobody else in the world, so nothing that exists to stop
  one member from another applies.
- **The demo starts on a world dashboard** that looks like a real one, and
  "Enter world" opens a real play field: the same engine, the same tools.
- **It ships as static files inside the image** and is served by the server
  from a directory named at start, as the web client already is
  (`--static-dir`). It holds no data, no secrets and no server code.
- **Whether an instance offers the demo is a runtime setting**, off by
  default. An operator of a private table has no reason to show strangers
  anything. When it is off, `/demo` does not exist and no link to it is
  drawn.
- **"Enter demo workspace" points at the demo**, and is drawn only when the
  demo is offered.
- **The demo's maps are the seven example maps**, and no others from the
  owner's library. The owner confirmed
  on 2026-10-05 that the maps in `examples/maps` are the owner's own work, published
  at <https://github.com/mbround18/vtt-maps> under CC BY 4.0, licence owner
  MBRound18. Each real map there becomes a scene of the demo
  world. This reverses the earlier dev-fixtures-only note, which was written
  before the licence was confirmed. The hand-written synthetic fixture is a
  parser test and is not a map.
- **Every map says whose it is.** Attribution is a condition of the licence,
  so it is a requirement here (FR-018), not a courtesy.
- **Tokens and portraits are a separate matter.** The confirmation covers the
  maps. The real client asks a third-party avatar service for a token with
  no art of its own, which the demo may not do; the demo draws its own.
- **The demo's game system is one of the two we stand behind**: 5e, from the
  open reference content the pack already carries.

## Requirements

### The account page

- **FR-001** A signed-in person's settings include a page holding: export my
  data (both formats), delete my account with the consequence stated as it is
  today, and sign out. It is reached from the account menu.
- **FR-002** `/counter` is removed from routes, navigation, prefetch lists
  and SEO configuration. Every end-to-end spec that reached account controls
  through it reaches them through the new page.

### The demo app

- **FR-003** `apps/demo` is a pnpm workspace app that builds to a static
  directory and runs from any static file host, at a base path of `/demo/`.
- **FR-004** `/demo/` shows the demo world's dashboard: its name, scenes,
  characters, compendium and members as a real world's dashboard shows them,
  with the visitor as Game Master.
- **FR-005** "Enter world" opens the play field on the demo world's first
  scene, with the engine running and the Game Master's tools available: move
  and place tokens, draw walls, doors, lights and shapes, switch scenes, open
  a character sheet, roll.
- **FR-006** Every page of the demo carries a standing notice that this is a
  demo, that nothing is saved anywhere but this browser, and a way to start
  over.
- **FR-007** Starting over returns the demo world to the state it shipped in.

### The backend in the page

- **FR-008** The demo answers GraphQL operations from an in-page backend with
  the same request and response shapes the server uses, and delivers world
  events to the same subscribers in the same order a server would: a mutation
  is acknowledged, then its event arrives.
- **FR-009** The set of operations the demo answers is a declared list. An
  operation outside it gets one defined answer — "not part of the demo" —
  which the page shows as a notice, never as a connection error and never as
  a silent nothing.
- **FR-010** The demo's answers are checked against the server's schema, so a
  field renamed on the server fails the demo's tests rather than its
  visitors.
- **FR-011** The demo world's starting state is a seed file in the
  repository. The same seed can be loaded by a test.
- **FR-012** What the visitor changes survives a reload of the tab. It is
  kept in the browser's own storage, written by our own code. No client-side
  database library is added; `AGENTS.md` forbids reintroducing RxDB or
  anything like it without the owner's say, and nothing here needs one.
- **FR-013** Features that need another person or a server — invitations,
  sign-in providers, mail, sharing links, uploads to instance storage, lore
  sync, moderation, administration — are not offered in the demo. Their
  entry points are absent or say they are not part of the demo.
- **FR-014** A map or image the visitor adds is held in the browser only and
  is never sent anywhere.

### Serving it

- **FR-015** The server serves the demo at `/demo` from a directory named at
  start (`--demo-dir` / `DEMO_DIR`), with the same history fallback the web
  client has, and only while the instance setting that offers the demo is
  on.
- **FR-016** The image carries the built demo beside the built client.
- **FR-017** The real client's "Enter demo workspace" entries link to `/demo`
  and are drawn only when the instance offers it. No route in the real client
  refers to `demo-world`.

### Attribution

- **FR-018** The demo credits the maps where a visitor will see it: a line on
  the demo's standing notice or its about page naming MBRound18, CC BY 4.0
  and <https://github.com/mbround18/vtt-maps>, and the same credit on each
  scene made from one. A notice file carrying the same text ships in the
  demo's static directory beside the maps.
- **FR-019** `examples/maps/README.md` states the confirmed provenance and
  licence in place of the warning it carries today.

## Success Criteria

- **SC-001** Served from a plain static file server with nothing behind it,
  the demo opens its dashboard, enters the world, and a token dragged across
  the board is where it was left after a reload.
- **SC-002** In the same run a wall is drawn, a door is opened, a light is
  placed and a character sheet rolls, each through the tool a Game Master
  uses.
- **SC-003** Across the whole of that run the browser makes no request to any
  path outside the demo's own static files, and opens no socket. The test
  fails on the first one.
- **SC-004** Starting over restores the shipped world.
- **SC-005** An operation the demo does not answer produces the notice of
  FR-009.
- **SC-006** On an instance with the setting off, `/demo` is not found and no
  page draws a link to it. With it on, a signed-out stranger opens the demo
  from the sign-in page.
- **SC-007** A signed-in person exports their data and deletes their account
  from the account page.
- **SC-009** Every example map opens as a scene in the demo, and the credit
  of FR-018 is on the page.
- **SC-008** At least one existing feature's end-to-end proof runs against
  the demo with no stack started, and is faster for it.

## Open questions for the owner

1. **Does any map use a third-party DungeonDraft asset pack?** If one does,
   that pack's own terms still apply to the exported image and are worth a
   look before it ships.
2. **Does the demo world carry a Roll for Shoes table as well as 5e**, or is
   one system enough for a first cut?

## What this spec does not do

- It does not make the demo multiplayer. Two tabs are two demos.
- It does not let a demo world be exported into a real instance. Worth
  wanting; a separate decision about what a visitor's content becomes.
- It does not re-implement the server's rules in the browser. With one
  visitor who is the Game Master there is nobody for a rule to refuse, and
  where the server would compute something for the page — what a token sees,
  for one — the demo uses what the engine already computes locally.
- It does not restructure `apps/web` into packages.
- It does not replace the server-backed end-to-end suite. The demo proves the
  client; only the stack proves the circle through the server.
- It does not add an AI Game Master, a guided tour or sample players. A
  visitor gets a table and its tools.
