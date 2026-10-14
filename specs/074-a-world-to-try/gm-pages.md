# Every page a Game Master can reach, in the demo (T023, FR-013)

The router is `apps/web/src/routes/AppRoutes.tsx` (and `/status`, mounted in
`App.tsx`); every route there is a row below, with real ids where the demo
seeds them and a made-up uuid (`00000000-0000-4000-8000-000000000000`) where
it does not. The demo renders the real client, so the decision for each page
is one of three:

- **absent**: the page is never shown. The client sends the visitor on to
  `/welcome`, as it would on a real instance that needs no setup and has the
  visitor signed in. There is no entry point to it anywhere in the demo.
- **notice**: the page opens, what it needs is refused by the demo's backend,
  and the one notice (FR-009) says which part of ThunderForge is not part of
  the demo. The page's own empty or error state shows beneath it.
- **works**: the page opens and nothing is refused. The feature runs in the
  browser.

Proof is `apps/demo/e2e/gm-pages.spec.ts` unless another file is named; each
row names the test that asserts it. The sidebar's pages and the board are
also walked in `demo-pages.spec.ts` and `demo-board.spec.ts`.

## Setup, sign-in, administration

| Route | Decision | Proven by |
| --- | --- | --- |
| `/setup`, `/setup/:code`, `/setup/callback` | absent (setup not required; lands on `/welcome`) | "pages that need setup, sign-in or an administrator are never shown" |
| `/login`, `/register`, `/signup` | absent (already signed in; lands on `/welcome`) | same |
| `/admin` and its 15 children (`welcome`, `settings`, `configuration`, `instance`, `readiness`, `mail`, `legal`, `analytics`, `storage`, `oauth`, `system`, `access`, `security`, `moderation`, `play-pauses`) | absent (the demo's session is not an administrator's; lands on `/welcome`) | same |
| `/oauth/callback/:providerKey` | absent as an entry point: no provider is offered, so only the address reaches it; it reports the missing exchange and its only ways out are the absent sign-in pages (`/welcome`) | "a sign-in provider's return and an instance invitation lead nowhere but Welcome" |
| `/invite/:code` | absent as an entry point: nothing in the demo issues one; "Create an account" lands on `/welcome` | same |
| `/join/:code` | notice: "Invite links", "World members"; the page says the link could not be opened | "pages that reach for a server say which part is not part of the demo" |

## Sharing

| Route | Decision | Proven by |
| --- | --- | --- |
| `/collection/:shareCode` | notice: "Share links" | "pages that reach for a server say which part …" |
| `/shared/actor/:code` | notice: "Share links" | same |
| `/shared/ability/:code` | notice: "Share links" | same |
| `/shared/item/:code` | notice: "Share links" | same |
| The sheet's own **Share** (`/world/:id/actor/:actorId/view`) | notice: "Share links", and no Agree button is offered | "sharing a character is refused by name, from the sheet's own Share" |
| Lore **Copy link** | works: the address is copied, nothing is sent | `demo-pages.spec.ts` (lore test) |

## Account and instance settings

| Route | Decision | Proven by |
| --- | --- | --- |
| `/settings/account` | works: the `account-demo` card says what the demo keeps and where; no export or deletion offered | "every other page opens with nothing refused"; `demo-pages.spec.ts` (controls) |
| `/settings/storage` | notice: "World settings" (the list of worlds is not read) | "pages that reach for a server say which part …" |
| `/settings/feedback` | notice: "That feature" | same |
| `/settings/security` | notice: "Your account", and the password and session endpoints | same |
| `/settings/standing` | notice: "That feature" | same |
| `/status` | works: `status-demo`, "Running in your browser" | "every other page opens with nothing refused" |
| `/legal/dmca`, `/legal/terms`, `/legal/privacy`, `/legal/operator` | works | same |

## Worlds and the library

| Route | Decision | Proven by |
| --- | --- | --- |
| `/` | works: lands on the demo world's dashboard | "the root lands on the demo world, and play opens the board" |
| `/welcome` | works | "every other page opens with nothing refused" |
| `/worlds` | works: one world listed | same |
| `/worlds/create` | notice: the starting maps are not read ("That feature"); **Create world** is refused as "Creating another world" | "pages that reach for a server say which part …"; "creating another world is refused by name, from the page that offers it" |
| `/library` | works: empty | "every other page opens with nothing refused" |
| `/library/:compendiumId` | notice: "The compendium library" | "pages that reach for a server say which part …" |

## Inside the world

| Route | Decision | Proven by |
| --- | --- | --- |
| `/world/:id` | works; no Delete world, Create another world, Generate Join Link or Sign out offered | "every other page opens with nothing refused"; `demo-pages.spec.ts` (controls) |
| `/world/:id/paused` | works: "Play has resumed" | "every other page opens with nothing refused" |
| `/world/:id/staging` | works | same; `demo-pages.spec.ts` (sidebar walk) |
| `/world/:id/actor-select` | works | "every other page opens with nothing refused" |
| `/world/:id/collections` | works | same |
| `/world/:id/compendium` (NPCs, Lore, Items, Books, Abilities tabs) | works; the Books tab says the world holds no books; **Import a book** is refused as "Importing a book" | same; `demo-pages.spec.ts` (Books tab); `demo-limits.spec.ts` |
| `/world/:id/compendium/npc/new` | works | "every other page opens with nothing refused" |
| `/world/:id/compendium/npc/:actorId/edit` | works | "the pages of the seeded fighter and the first scene open with nothing refused" |
| `/world/:id/compendium/item/new` | works | "every other page opens with nothing refused" |
| `/world/:id/compendium/item/:itemId/edit` | notice: the world ships no items ("That feature"; "Item not found") | "pages that reach for a server say which part …" |
| `/world/:id/scenes` | works: seven scenes | "every other page opens with nothing refused"; `demo-pages.spec.ts` (maps) |
| `/world/:id/scenes/:sceneId` | works, each credited | "the pages of the seeded fighter …"; `demo-pages.spec.ts` (maps) |
| `/world/:id/players` | works: no Generate Join Link | "every other page opens with nothing refused"; `demo-pages.spec.ts` (controls) |
| `/world/:id/actor/:actorId/view`, `/edit`, `/import` | works | "the pages of the seeded fighter …" |
| `/world/:id/lore/:slug/view`, `/edit`, `/history` | works; uploading an image to lore is refused as "Uploading images to lore" (`refusalNames.ts`) | "every other page opens with nothing refused"; `demo-pages.spec.ts` (lore) |
| `/world/:id/ability/:abilityId/view`, `/edit` | notice: the world ships no abilities ("That feature"; "Ability not found") | "pages that reach for a server say which part …" |
| `/world/:id/item/:itemId/view`, `/edit` | notice: as above ("Item not found") | same |
| `/world/:id/settings/system` | works; no Invite players card, no GIF from another site; the lore repository card says in its own words "Repository sync is not part of the demo." (`instanceRepositoryIntegration` is answered, not refused) | "every other page opens with nothing refused"; `demo-pages.spec.ts` (sidebar walk) |
| `/world/:id/play` | works: the board | "the root lands on the demo world, and play opens the board"; `demo-board.spec.ts` |
| `*` (any other address) | works: "That page does not exist." | "every other page opens with nothing refused" |

## Totals

Counting each route of the router once (the 15 admin children and the three
setup routes each as their own row):

- **absent**: 24 (3 setup, 3 sign-in, 16 admin, the provider return, the
  instance invitation)
- **not part of the demo** (notice): 16 routes, plus 2 actions on pages
  that otherwise work (Create world, Share)
- **works**: 31 routes
- **fixed** in T023: 1. A share link, and the sharing terms the dialog reads
  first, were refused as "Characters", "The compendium library" or "That
  feature"; `refusalNames.ts` now names them "Share links" (FR-013 lists
  sharing links by name). No page broke or dead-ended with neither an absence
  nor a notice.

What the walk did not change: pages with a made-up id say "not found" beneath
the notice, which is the client's own empty state and the right one, since
the demo world ships no items or abilities.
