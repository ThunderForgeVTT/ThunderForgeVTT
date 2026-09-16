# The owner's walkthrough, 2026-09-15

A pass through the product, screen by screen, in the owner's own words. Every
item below is either **done**, **being built**, **specified**, or **queued** —
nothing rests on remembering the conversation.

Written the way `specs/031-playability/playtest-2026-09-10.md` was: the finding
first, then what was decided, then where the work lives.

## How to read the status column

| Status | Means |
|---|---|
| **Done** | On `main`, proven, pushed. |
| **Building** | An agent has it now; it lands as a signed commit on a branch and is merged after its e2e run. |
| **Spec** | Written as a specification; needs a plan and tasks before anyone builds it. |
| **Queued** | Waiting as a task, ready to start. |

---

## The admin portal

| # | Finding, in the owner's words | Decided | Status |
|---|---|---|---|
| A1 | `/admin/configuration`: "Can you tabulate this? It's kind of hard to read." | Three tables: providers, GitHub apps, manifest keys | **Done** — 8,044px → 2,607px |
| A2 | `/admin/instance`: "hard to read, a pages system might look really nice here" | One settings group at a time, the group in the URL | **Done** |
| A3 | `/admin/readiness`: "it needs a call to action, like a link on how to set those things up" | Every gap links to the setting that fixes it; an environment-fixed gap names the variable | **Done** |
| A4 | `/admin/mail`: "this email tester is a really great place to add email setup concepts" | SMTP settings live on the mail page, beside the test that proves them | **Done** — they were always stored in the database; nothing offered them |
| A5 | `/admin/legal`: "the tabs do absolutely nothing. So what is this page really?" | The tabs did work; with no enquiries every tab looked identical. Counts per tab, an empty state each, moderation hand-off promoted | **Done** |
| A6 | `/admin/security`: "I have no idea what this screen is really telling me" | Counts as the headline, a sentence saying who is affected and that nobody is locked out | **Done** |
| A7 | `/admin/security`: "this should allow me to ask people to set up two-factor" | A skippable prompt at sign-in and a profile banner, with enrolment progress. No email blast | **Queued** |
| A8 | `/admin/storage`: "worlds zero bytes, databases zero bytes, modules zero bytes. I don't think this is very accurate" | Measure database, object store and files on disk; drop the vestigial rows | **Queued** — it reported 312MB while Postgres alone held 672MB |
| A9 | `/admin/access`: "on a closed instance the user doesn't need to provide DMCA takedown information... this is a huge spec change" | Access mode decides the legal duty; new instances start closed; existing keep their mode | **Spec 052**, with the constitution amended (1.2.0) and ADR-103 |
| A10 | `/admin/moderation`: "it's just GUIDs. It's not really like anything that I can actually track" | Content and conduct in one queue, named in words, per-account history; conduct reports only within a shared table | **Spec 053** |
| A11 | `/admin/play-pauses`: "this should be an actually indexed and searchable table... search vectors to find why their play has been paused" | Server-side search and filters, paged, indexed | **Queued** |
| A12 | `/counter`: "doesn't really show what's going on in the application" | Replace with a real dashboard for the person signed in; the component gallery leaves the nav | **Spec 054** |

**Found while working, worth knowing:** `/counter` holds the only export-my-data
and delete-my-account controls, so spec 054 moves and proves them before that
page goes. And moderation has no query that lists cases at all — it could never
have been a frontend fix.

---

## Worlds, actors and the compendium

| # | Finding | Decided | Status |
|---|---|---|---|
| W1 | `/worlds`: "if somebody has more than six worlds, we should convert to a table, but allow them to flip" | Tiles to six, table beyond, toggle either way, remembered | **Building** |
| W2 | `/world/:id`: "statistics like scenes, players, tokens, but not actually showing every scene" | Figures first, a short list of recent scenes, one server query allowed | **Building** |
| W3 | `/world/:id/players`: "it should say Owner / Game Master, then playing all characters, then Set character" | A Game Master may hold a character without giving up anything | **Building** — the server already allows it |
| W4 | `/world/:id/settings/system`: "slams everything to the left... nice tabulation, nice padding" | Fluid layout, named sections | **Building** |
| W5 | Same page: "if you are Genie, it should show Genie on the select dropdown" | The picker shows the world's system | **Building** |
| W6 | Same page: "gridless, squares, and then hexagons" rather than "None" | Gridless is a choice, not an absence | **Building** |
| W7 | Same page: "put the books in the system settings, to turn on or off compendiums" | Which content a world enables is data that does not exist | **Spec 055** |
| W8 | Compendium NPCs: "really handy if we could set the image of the individual right from this page" | Set a portrait from the row, one uploader offered in three places | **Building** |
| W9 | Compendium: "game system generic" on a Genie world's actors | A player-created character is stamped `generic`, ignoring its world. Plus a backfill | **Queued** — a real defect |
| W10 | Compendium lore: "lacking the real file structure... create a folder, create a lore entry" | The tree exists in the database; no screen offers it | **Spec 055** |
| W11 | Compendium items: "a really nice indexable table based off of types... assign tags and have rich filtering" | Types, tags, filtering, one search | **Spec 055** |
| W12 | Compendium books: "I don't think this page was really built out very nicely" | Answer what the page should be, not just tidy it | **Spec 055** |
| W13 | Compendium abilities: "if I select scrolls, it should really be scroll name and scroll description" | The form speaks the selected type's words; the umbrella already does | **Building** |
| W14 | Actor edit: "doesn't show the ability to upload icons... I expected a token and/or a portrait" | Mount the existing imagery panel where a player's character lives | **Building** |
| W15 | Actor edit: "use our hero generator to generate something that makes sense" | The builder is specified and was unplanned | **Spec 044** — plan and tasks being written |
| W16 | Genie's sheet: "everything is in one very strict column... showcase how flexible our configuration system is" | Lay it out like a sheet; say what a pack must declare to get it without writing React | **Building** |

---

## The play field

| # | Finding | Decided | Status |
|---|---|---|---|
| P1 | "When we're loading in the play field, it'd be nice to have a loading symbol" | A loading state that says what is happening | **Building** |
| P2 | "When I select a world and it's loading a world image... it sort of just freezes" | Cover the whole path, including the map image | **Building** |
| P3 | "I place the token, I don't know if it places the actor's image" | It does — a token inherits its actor's art. Show what is being placed, and say when there is none | **Building** |
| P4 | "I expect WASD to also be able to move tokens" | WASD alongside the arrow keys | **Building** |
| P5 | "a little target icon... takes my viewport and scrolls me over to that specific icon", for any actor and in combat | Locate from the actors panel and the tracker, obeying what the viewer may see | **Building** |
| P6 | "if it's my turn, the viewport should scroll to me, slightly zoomed out... should be toggleable" | Follow the turn, per person, remembered | **Building** |
| P7 | "the place light source doesn't attach to cursor like I expected" | Light placement matches token placement | **Building** |
| P8 | "the shapes tools simply just do not work" | Not reproduced. The shape code is unchanged since the 2026-09-10 fix, every tool armed and drew on current main in a real browser (fresh scene, imported map, dark scene, the seeded demo scene), and the owner confirmed on 2026-09-16 that drawing works after restarting the stack | **Closed** — most likely an older build or stale world state. One lesson kept: `canvas-authoring.spec.ts` checks the stored shape count but never the canvas, so a shape saved but not drawn would still pass |
| P9 | "the tokens list shows me absolutely nothing" | What is on the board, the compendium by type, one search, and picking places it on the cursor | **Spec 055** |
| P10 | "there should be a right click context menu on a wall" | Select a wall, right-click it; the id picker becomes a fallback | **Spec 056** |
| P11 | "start a wall here if you click anywhere on the map" | One "place here" menu: token, object, light, wall, interaction point | **Spec 056** |
| P12 | "a token could be an object, like a chest... a token is interactable whereas an object is not" | An object is scenery until a Game Master makes it interactive; the `Object` kind already exists | **Spec 056** |
| P13 | "I don't know what explored areas means... we need a separate fog of war, below lights" | Fog as its own layer; explored areas stay per-browser memory | **Spec 057** |
| P14 | "I don't know what walls really does... draw walls, select doors" | A rail that says what it does in a Game Master's words | **Spec 057** |
| P15 | "doors should have snapping capabilities" | Snap to grid, to a wall's end, to a right angle; free drawing kept behind a modifier | **Spec 057** |
| P16 | "we should be able to assume the token's personality... I can't really see my line of sight" | See the board as a creature sees it, to validate walls | **Spec 057** |
| P17 | "time of day should be its own setting... take a day scene and turn it to night" | A first-class scene property, reachable while playing | **Spec 057** — scene ambient light already exists and was not discoverable |
| P18 | "the egg scene is really blown out... 100 by 100 and the grid size is 5 pixels" | Warn when grid and map disagree, with the value that would fit. Warn, never refuse | **Queued** |
| P19 | Clocks and timers: "broken because I can't select a character... NPCs should be last, players first" | Actors, characters first | **Building** — checking whether today's NPC hiding caused it |
| P20 | Genie wishes: "the asked-for list doesn't actually show anything" | A defect, traced end to end | **Building** |
| P21 | "the chat feature doesn't show somebody made a wish... should be malleable" | Which actions a table sees, declared by the pack | **Proposal being written** |

---

## What the walkthrough cost, and what it bought

Eleven screens, fifty-odd findings, and **two real defects nobody knew about**:
the shapes tools stopped working since they were fixed, and every
player-created character is stamped with the wrong game system. Both were
invisible from the inside; both came from someone using the product as a
person rather than as a test.

Six specifications came out of it (044's plan, 052, 053, 054, 055, 056, 057),
which is a fortnight of building at least. The order that makes sense is the
one the owner walked: the things that make a table's first hour work, before
the things an operator needs at scale.
