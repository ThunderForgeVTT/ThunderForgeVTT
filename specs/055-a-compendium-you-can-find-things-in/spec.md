# Feature Specification: A Compendium You Can Find Things In

**Feature Branch**: `055-a-compendium-you-can-find-things-in`

**Created**: 2026-09-15

**Status**: Draft

**Input**: Project owner, walking the compendium on 2026-09-15, tab by tab.

On `?tab=lore`: "This page is lacking the real file structure that we were
talking about where we could do like create a folder or create a file or create
a lore entry. We had talked about it, but I don't think we actually implemented
it on the lore screen so players and individuals could see it."

On `?tab=items`: "This page … should be a really nice indexable table based off
of types for items. Like we should be able to assign tags and have rich
filtering. It could all be under the general concept of items, but the search
really needs to be improved to show that it has different item types, et
cetera. Like imagine fifth edition or Pathfinder."

On `?tab=books`: "I'm an admin and I don't think this page was really built out
very nicely for books. We should figure out what this page needs and then fix it
up."

And on the play field's token list, the same day: "The tokens list shows me
absolutely nothing. The tokens list should actually be an ability to select and
search different things — like if I want to search for goblins or dragons — but
it should put what's on the screen, like on the screen, in a breakdown. And then
it should put compendiums, and list the compendium types, and let the person
drill down into them manually. Or search between everything to find specific
things. If I select a token that's not actually on the screen, it should attach
it to my cursor and let me place it on the map."

## The problem

The compendium is five tabs over one world's content
(`apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx:42`: npcs, lore,
items, abilities, books; `?tab=` read at `:51-55`). Four of them are a flat
list with a box on top, each searching only its own tab, each by a different
mechanism, none of them able to say what kind of thing a row is. The fifth has
no box at all.

The play field has the same shape. Its Tokens panel renders nothing at all, a
second panel of the same name lists identifiers, and the one panel that
searches — Actors — never mentions the scene.

The owner's four complaints are four symptoms of one thing: **the product
stores structure it does not show, and then offers a different search for each
place it failed to show it.**

### Lore: the tree is built, stored, moved and hidden

`world_lore_entries.parent_id` is real (`src/server/src/schema.rs:1411`). It is
writable through `moveLoreEntry`
(`src/server/src/graphql/mutations_lore_tree.rs:371`, impl `:191`, the write at
`:243`), which refuses a cycle (`would_cycle`, `:236`) and checks the parent
exists (`:226-235`). Deleting an entry reparents its children to their
grandparent rather than orphaning them (`mutations_lore.rs:351`, `:368-370`).
Spec 034 depends on all of it — "`world_lore_entries.parent_id` is the tree,
`world_lore_tags` is the tags" (`specs/034-lore-git-sync/spec.md:37`) — and
mirrors it into repository directories (`spec.md:78-79`, `:96-99`).

The client has the tree too, as a library:
`apps/web/src/pages/world/lore/loreTree.ts` holds `buildLoreTree:49`,
`flattenLoreTree:80`, `ancestorsOf:90`, `descendantIdsOf:112`,
`validMoveTargets:141`, `normaliseTag:150`, `matchesLoreQuery:162`,
`filterLoreEntries:188` and `allTagsOf:208`.

**The owner is right that no screen offers folders, and nearly right that it
was never implemented.** What exists is smaller than a file tree and is in the
wrong place:

- The compendium's Lore tab is a **flat two-column table** — Title, Actions
  (`LoreCompendiumTab.tsx:128-133`), rows rendered straight from the array
  (`:136`). It never mentions `parentId`, never imports `loreTree.ts`, and
  offers no folder, no move, no rename and no delete. Its one creation control
  is a title box and a "New entry" button (`:187-207`) calling
  `createLoreEntry({ worldId, title })` (`:68`).
- The only tree control in the product is a **parent `<select>`** on an
  entry's own detail page — `LoreOrganisationPanel.tsx:192-209`, mounted from
  `LoreEntryDetailPage.tsx:314`, with a breadcrumb at `:171-191` and tag add
  and remove at `:211-260`. `loreTree.ts` is imported by that panel and by its
  test, and by nothing else.

So an entry's position is set from inside the entry, one entry at a time, by
picking its parent from a dropdown. There is nowhere to *see* the shape, and
`CreateLoreEntryInput` does not take a parent at all — every new entry is a
root until somebody moves it.

### Lore: there is no such thing as a hidden lore entry

This matters more than the tree, because "so players and individuals could see
it" assumes there is something players do not see.

There is not. Lore's only access model is `world_lore_permissions.level`
(`src/server/src/schema.rs:1442-1452`), a Viewer/Editor/Owner ladder generated
at `src/server/src/auth/permissioned_entities.rs:264-274`. That module states
the consequence itself (`:41-46`):

> Visibility is a **separate axis** from the permission ladder: `Viewer` is
> both the ladder's floor and its default, so the ladder structurally cannot
> express "hidden".

Abilities have the separate axis — `world_abilities.gm_only`
(`schema.rs:924`). NPCs got theirs today —
`world_actors.visible_to_players` (`schema.rs:1087`, commit `375aeed`,
enforced in `src/server/src/auth/npc_visibility.rs`: `seen_by_everyone` `:81`,
`retain_visible_sync` `:180-193`, `require_actor_visible` `:255`). **Lore has
neither**, and its list query does not consult the ladder either:
`world_lore_entries_impl` checks `require_visible_world`
(`src/server/src/graphql/queries/lore.rs:226`), loads every entry in the world
(`:233-241`) and applies only the spec 015 moderation filter (`:244`). The code
says why, at `queries/lore.rs:448-454`: the ladder's Viewer minimum "is not a
real gate", because every caller defaults to Viewer when no explicit row
exists.

Spec 012's FR-001 asks for a list of "all lore entries visible to the current
user". Today that is every entry in the world, for every member. A folder tree
a player can see is therefore a question the product has never had to answer,
and Q1 below is where it is asked.

### Items: no type, no tags, nothing to index

`world_items` (`src/server/src/schema.rs:1373-1391`) is `name`, `description`,
`icon_asset_id`, the combat geometry (`reach`, `range_normal`, `range_long`,
`needs_line_of_sight`, `action_cost`, `legendary_cost`, `multiattack`) and
bookkeeping. There is **no type, kind, category, slot or rarity column, and no
`world_item_tags` table** — the name appears nowhere in the repository, and the
item family in `allow_tables_to_appear_in_same_query!` (`schema.rs:1825-1830`)
is abilities, effects, permissions, prices and shares. `CreateItemInput` is
`world_id`, `name`, `description` (`mutations_items.rs:22-27`);
`UpdateItemInput` is `item_id`, `name`, `description` (`:29-34`).

The screen matches the data. `ItemCompendiumTab.tsx` renders Name,
Description, Price, Actions (`:107-113`); its search is a server query with no
debounce (`:57`, `api/items.ts:79-93`); there is no facet, no sort, and no
column header a person can click.

**The product already knows how to do what the owner asked for — for
abilities.** `world_abilities.classification` (`schema.rs:923`) is a type, the
system pack declares the vocabulary (`src/server/src/ability_vocabulary.rs`;
`packs/systems/dnd5e/system.json` `abilityVocabulary`), and
`AbilityCompendiumTab.tsx` renders a row of type tabs built from that
vocabulary with a count each (`:220-241`, `:283-320`), an **Unrecognised**
bucket for classifications the system does not know (`:206-218`), and a
create form with a classification `<select>` and a debounced "did you mean?"
(`:148-183`, `:430-500`). Items have none of it and no pack declares an item
vocabulary: `contentPatterns` exists in `packs/systems/dnd5e/system.json:1033`
and in no other pack, and its `kind` values — `creature`, `spell`, `magicItem`,
`feat` — describe *book entries a reader extracted*, not items in a world.

### Books: mostly invisible, and unfinished where it is visible

A **book** is one imported source book: a `compendiums` row
(`schema.rs:153-177`, `book_title` at `:161`) owned by an account
(`owner_user_id`), whose contents are `compendium_entries`
(`schema.rs:136-151`: `kind` `:141`, `field_values` `:145`, `prose_text`
`:146`, `extras` `:148`). `world_books` (`schema.rs:1132-1144`) is the *book
list*: the join saying this world has switched that account-owned book on,
pinning `base_source_hash` and `base_parser_version` so a re-parse is
detectable. A world's overrides, hides and additions layer on top in
`world_entry_deltas` (`schema.rs:1254+`).

Who may read one is already decided, and decided well
(`src/server/src/library/book_list.rs`): `require_book_manager:140` is world
Owner or Game Master **and** account owner of the compendium, with **no admin
bypass**, stated deliberately at `:130-133`; `books_on:264` is readable by
every member of the world, and says so at `:258-262`.

The tab is `BookListTab.tsx`, 879 lines, gated on `managesBooks` — Owner, Game
Master or Trusted Player, from `WorldCompendiumPage.tsx:224` and
`useWorldRole.ts:39-72`. Every member sees the list of switched-on books with
title, system and entry total (`:188-214`). Everything else is manager-only:
browsing (`:247-252`, `BookEntries` at `:400`), the switch-off report
(`:257-317`), kept additions (`:321-323`) and the offer list (`:325-370`).

What is actually unfinished:

- **No search anywhere in it.** The browser has a kind filter and a cursor
  pager and no text box; the server query behind it accepts `kind`, `after`,
  `first` and `showHidden` and no search argument (`worldBooks.ts:253-297`).
  This is the one tab where a person is most likely to be looking for a
  specific thing, and it is the one tab with nothing to look with.
- **The kind filter is built from the page already loaded** (`:464`,
  `:476-495`). A kind that first appears after "Show more" never gets a button,
  and filtering re-queries the server while the button list stays stale.
- **No way in.** `WorldSidebarNav.tsx:73-99` offers NPCs, Lore, Items and
  Abilities. Books is reachable only by clicking the tab or typing
  `?tab=books`. The page's own description names "NPCs, items, and abilities"
  (`WorldCompendiumRoutePage.tsx:54`) and the overview default text omits books
  too (`compendiumOverview.ts:12-13`).
- **The editing surfaces are raw.** `AddEntryForm` takes a free-text "Kind"
  (`:848-858`) and prose; `changeEntry` is a grid of text inputs keyed by field
  name (`:684-701`). Nothing offers the kinds the book actually contains.
- **Page size is never passed** (`entriesFrom` at `:419` and `:437` omit
  `first`).

### The play field's token list is not a list

The owner is being literal. The tool rail's **Tokens** panel
(`apps/web/src/pages/world/WorldPage.tsx:2787-2788`) is `TokenTool`
(`apps/web/src/components/canvas-tools/TokenTool/TokenTool.tsx:101`), and its
first act after resolving permissions is:

```tsx
// TokenTool.tsx:192
if (!selected || !selectedToken) {
  return null;
}
```

With nothing selected it renders **nothing at all** — not an empty state, not a
prompt, nothing. With something selected it renders a property sheet for that
one token: size, facing, art, name visibility (`:219` "Selected token"). It is
not a list, it has never been a list, and it is labelled as one.

There is a second surface with the same name, and it has the defect spec 053
named on the moderation screen. `TokenPanel`
(`apps/web/src/components/TokenPanel.tsx`), opened by a button at the bottom of
the play field (`WorldPage.tsx:3037-3054`), does list a scene's tokens
(`getTokens(sceneId)` at `TokenPanel.tsx:181-196`, rows at `:354-549`). Every
row reads:

```tsx
// TokenPanel.tsx:385-388
{token.isPrimary ? "Primary — " : ""}
Token {token.tokenId.slice(0, 8)}
```

A UUID prefix, with a DiceBear avatar seeded from that same id when there is no
art (`:317-319`). The row model already carries a name — `TOKEN_FIELDS`
(`apps/web/src/api/tokens.ts:8-25`) fetches it — and no row renders it. There is
no search, no filter, no sort and no type; the panel is a modal overlay
(`:341-344`) that covers the map rather than sitting beside it; and clicking a
row opens administrative controls, including a free-text owner **user id** field
(`:465-491`), rather than selecting the token on the canvas.

So one panel called Tokens shows nothing, and another shows identifiers. Both
readings of the owner's sentence are true.

The pieces of the list the owner described exist, in two other places:

- **Searching and placing already work, for actors.** `ActorsPanel` has a search
  box (`ActorsPanel.tsx:300-303`), shows each actor's player visibility
  (`:259-270`), and has a **Place** button that calls `beginTokenPlacement`
  (`:280-288`), which hands the actor to the engine to carry on the cursor until
  a left click drops it (`apps/web/src/engine/bevy/index.ts:1234-1241`). The
  engine owns the carry — the preview, the snapping and the cancel — because
  screen-to-world is the camera's business
  (`index.ts:1225-1233`). `beginPropPlacement` (`:1256`) is deliberately the
  same machine for something that is not an actor's token.
- **What is on the scene has a breakdown waiting.** `tokens.token_type`
  (`src/server/src/schema.rs:816`) is validated on write against a closed set —
  `TokenKind::Character`, `Npc`, `Vehicle`, `Object`
  (`crates/thunderforge-canvas-core/src/token_kind.rs:40-62`;
  `mutations_tokens.rs:105-111` refuses an unknown kind because "the column
  decides how the token is drawn"). A token also carries
  `name_visible_to_players` (`schema.rs:817`), which
  `auth::npc_visibility::player_may_read_token_name`
  (`npc_visibility.rs:285-286`) already enforces.

**And the server can already answer every part of it.** `tokens(sceneId)`
(`src/server/src/graphql/queries/scene.rs:249-292`) lists a scene's tokens,
withholds names hidden from players by way of a `runs_the_world` flag
(`:283-287`) and returns nothing at all for content under takedown
(`:259-261`). Its siblings list the rest of what is on a scene: `walls`
(`scene.rs:335`), `light_sources` (`:372`), `shapes` (`:410`, already filtering
Game-Master-only shapes from players) and `interactives`
(`queries/interactives.rs:208`). Every scene-scoped list exists on the server;
no client panel renders any of them as something a person can browse.

The rest of the tool rail has the same shape. `WallTool` takes every wall as a
prop and indexes it only by the selected id (`WallTool.tsx:16-20`, `:47`);
`LightingTool` enumerates scene tokens only to fill an "attach to token"
dropdown (`LightingTool.tsx:292-314`); `InteractionTool` shows a **count** and
not a list — "N things on this scene respond" (`InteractionTool.tsx:316-321`) —
and labels its reference choices `Wall <uuid8>` and `Light <uuid8>`
(`:164-179`).

So the panel that is named for the list shows nothing, the panel that lists
shows identifiers, the panel that searches is named for actors and never
mentions the scene, and the panels for walls, lights and interactions each hold
the data for a list and render a property sheet. What is missing is one surface
that holds both halves — what is on this scene, and everything the world has
that is not — under one search.

### And there is no search across any of it

There is no cross-compendium search in the product — no global box, no command
palette, no query that spans types. Each tab searches its own collection, by a
different mechanism, and none of them keeps the query in the URL:

| Tab | How it filters |
|-----|----------------|
| Lore | Client-side, **title only**, case-insensitive `includes` (`LoreCompendiumTab.tsx:87-95`); body and tags are not searched |
| Items | Server query, refetched every keystroke, no debounce (`ItemCompendiumTab.tsx:57`, `api/items.ts:85-86`) |
| Abilities | Server query for text (`AbilityCompendiumTab.tsx:127`), client-side filter for the type tab (`:244-259`) |
| NPCs | Client-side FlexSearch index in IndexedDB (`NpcCompendiumTab.tsx:81-90`, `:118-127`); a server `searchActors` exists and this tab does not use it (`:18-23`) |
| Books | Nothing |

Four mechanisms, four answers to "is this fast", four answers to "what counts
as a match", and no way to ask one question of the whole compendium.

## What this spec is

One answer to **finding things** — across lore, items, NPCs, abilities and
books — built on the structure the product already stores, plus the two facts
it does not: an item's type and its tags.

It is not a rewrite of what those things *are*. Spec 012's lore model, spec
025's ability permissions, spec 049's import and spec 050's library all stand.
Where this spec adds an axis (lore visibility, item type, item tags) it says so
as a requirement and gives it the shape the product already uses for the same
idea elsewhere, so the compendium ends with one way of saying "this is a kind
of thing" rather than three.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A Game Master organises lore into folders (Priority: P1)

A Game Master opens the Lore tab and sees the shape of their world's lore: a
tree they can expand and collapse. They make a folder called Nations, drag
three entries into it, rename one, and make a new entry inside it without
touching a dropdown.

**Why this priority**: It is the owner's first complaint, the structure is
already stored, and every other lore requirement here reads better once there
is somewhere to see it.

**Independent Test**: In a world with a dozen flat entries, create a folder,
move three entries into it, rename it, and create a fourth entry inside it.
Reload: the tree is the same. Nothing in `world_lore_entries` needed a new
column.

**Acceptance Scenarios**:

1. **Given** a world with lore, **When** a Game Master opens the Lore tab,
   **Then** the entries are shown as a tree by their stored parent, not as a
   flat list.
2. **Given** the tree, **When** they create a folder, **Then** it appears where
   they made it and can hold entries.
3. **Given** an entry, **When** they move it under another, **Then** the move
   is saved, its children come with it, and a move that would make a cycle is
   refused with a reason.
4. **Given** an entry, **When** they rename it, **Then** the name changes
   everywhere it is shown and every link to it still resolves.
5. **Given** a folder, **When** they create an entry inside it, **Then** it is
   created there — not at the root to be moved afterwards.
6. **Given** an expanded or collapsed tree, **When** they come back to the tab,
   **Then** it is as they left it.

---

### User Story 2 - A player sees the lore they are meant to see (Priority: P1)

A player opens the Lore tab of a campaign whose Game Master has been writing
the villain's real history. They see the folders and entries meant for them,
and they see no trace of the ones that are not — not a title, not a folder that
contains only hidden things, not a count that gives one away.

**Why this priority**: The owner said "so players and individuals could see
it", and a tree is the first surface where hiding something is a structural
problem rather than a per-page one. It is also the requirement most easily got
wrong quietly.

**Independent Test**: A Game Master hides one entry and a folder containing
only hidden entries. A player's tree shows neither, no empty parent appears
where the folder was, and no count on any ancestor includes them.

**Acceptance Scenarios**:

1. **Given** an entry a Game Master has hidden, **When** a player opens the
   Lore tab, **Then** it is not in their tree and its title appears nowhere.
2. **Given** a folder all of whose descendants are hidden, **Then** the folder
   does not appear in a player's tree either.
3. **Given** a folder with some hidden and some visible descendants, **Then**
   the player sees the folder and only the visible descendants, and any count
   shown on it counts only what they can see.
4. **Given** a hidden entry, **When** a player reaches its address directly,
   **Then** they are refused in the same words any other unavailable entry
   uses, and the refusal does not confirm that the entry exists.
5. **Given** a Game Master, **Then** they see everything, and every hidden
   thing is marked as hidden so they know what a player is missing.
6. **Given** a hidden entry, **When** a player searches the compendium, **Then**
   it is not among the results (US6).

---

### User Story 3 - Items read as an index, by type (Priority: P1)

A Game Master running a fifth-edition game opens the Items tab and sees an
indexed table: a row of type tabs with counts — Weapon, Armor, Potion,
Wondrous Item, Ring, Scroll — a table they can sort by any column, filters for
tags and price, and a search box that tells them whether it matched a type or
some text.

**Why this priority**: It is the owner's second complaint, stated in full, and
it is the tab that is furthest from what a person coming from 5e or Pathfinder
expects.

**Independent Test**: In a world with two hundred items across eight types,
filter to one type, add a tag filter, sort by price, and search. The counts on
the tabs match the rows, and the URL carries the state so the view can be
handed to somebody else.

**Acceptance Scenarios**:

1. **Given** a world with items of several types, **Then** the tab shows the
   types with a count each, taken from the system the world uses.
2. **Given** a type selected, **Then** the table shows only items of that type
   and says so.
3. **Given** an item whose type the system does not recognise, **Then** it
   appears in a bucket for exactly that, as abilities already do, and is never
   silently dropped.
4. **Given** the table, **When** a person clicks a column heading, **Then** it
   sorts by that column, and the sort is visible.
5. **Given** tags on items, **When** a person filters by one or more, **Then**
   the table narrows, and clearing the filter restores it.
6. **Given** a search that matches a type name and a search that matches item
   text, **Then** the results say which happened, and a person can restrict to
   one.
7. **Given** any of the above, **Then** the state is in the address, so the
   view can be reloaded or sent to somebody.

---

### User Story 4 - Tags, and who may make one (Priority: P2)

A Game Master tags three items "cursed" and two lore entries "Nations". Later
they type "curs" into a tag box and are offered "cursed" rather than making
"cursed " a second time. A player with edit rights on their own character's
items can tag those; a player with none cannot invent a tag that everybody
sees.

**Why this priority**: Tags are the half of the owner's items request that is
not a type, and a tag vocabulary that anybody can add to without seeing what
exists becomes noise within a campaign.

**Independent Test**: Create a tag, apply it to an item and a lore entry, and
confirm the tag box suggests it from the third letter and that it is one tag,
not two, across both.

**Acceptance Scenarios**:

1. **Given** a world, **When** somebody with edit rights on a thing tags it,
   **Then** the tag is applied and becomes available to that world's other
   content.
2. **Given** an existing tag, **When** somebody types part of it, **Then** it is
   offered before a new one is made, and case and surrounding space do not make
   a second tag.
3. **Given** a tag a system pack declares, **Then** it is present in the world
   from the start and is marked as coming from the system.
4. **Given** somebody with no edit rights on a thing, **Then** they cannot tag
   it.
5. **Given** a tag nothing carries any more, **Then** it stops being offered,
   and no cleanup is asked of anybody.

---

### User Story 5 - An admin's view of books (Priority: P2)

An administrator opens the instance's view of books and can answer the
questions an operator has: how many books are on this instance, which are the
same source imported twice, which need re-parsing because the reader changed,
how much storage they hold, and which worlds have one switched on. They cannot
read the text of a book they do not own.

**Why this priority**: It is the owner's third complaint read literally — "I'm
an admin" — and it is the part of books that does not exist at all rather than
existing badly.

**Independent Test**: Import the same source book on two accounts. An
administrator sees two books and that they share a source; opening either
shows counts, versions and the worlds using it, and never an entry's text.

**Acceptance Scenarios**:

1. **Given** an instance with books on several accounts, **When** an
   administrator opens the books view, **Then** they see every book with its
   title, system, owner, entry counts, parser version and storage.
2. **Given** two books with the same source, **Then** the view says so.
3. **Given** a book parsed by an older reader than the instance now has,
   **Then** the view says it can be re-parsed and what that would change.
4. **Given** any book, **When** an administrator opens it, **Then** they see
   which worlds have switched it on, and **no** entry text, field values or
   prose from a book they do not own.
5. **Given** a book under moderation (spec 053), **Then** the view says so and
   links to the case.

---

### User Story 6 - One search across the compendium (Priority: P1)

Somebody types "raven" once and sees what the world holds by that name: a lore
entry, two NPCs, an item, a spell and three lines in a book — grouped by kind,
each saying what it is, each opening where it lives.

**Why this priority**: It is what the compendium is for, it is the thing none
of the five tabs can do, and it is the requirement that makes the other four
stories worth having.

**Independent Test**: Seed one name across five content kinds and search it
once. Every kind appears, grouped, and opening a result lands on that thing.

**Acceptance Scenarios**:

1. **Given** a search term, **Then** results come from lore, items, NPCs,
   abilities and books, grouped by kind with a count each.
2. **Given** a result, **Then** it says what kind of thing it is before it is
   opened, and opening it lands on that thing in its own tab or page.
3. **Given** a player searching, **Then** they see only what they may see: no
   hidden NPC, no hidden lore entry, no Game-Master-only ability, and nothing
   from a book the world has not switched on.
4. **Given** a Game Master searching the same term, **Then** they see more, and
   each result a player could not see is marked.
5. **Given** a term that matches a type or a tag rather than text, **Then** the
   search says so and offers to filter by it instead.
6. **Given** a search, **Then** its term and filters are in the address.

---

### User Story 7 - A book's page is a book (Priority: P2)

A Game Master opens a switched-on book and gets something worth reading: what
it is, what it contains by kind, what this world has changed or hidden in it,
and a search across its entries that lands on the page it came from.

**Why this priority**: It is the tab the owner called not built out, and it is
the one place a person is looking for a specific rule rather than browsing.

**Independent Test**: Switch on a book of several thousand entries, search a
phrase, filter by a kind that only appears deep in it, and open a changed entry
to see what this world changed.

**Acceptance Scenarios**:

1. **Given** a switched-on book, **Then** its page names the book, its system,
   its page count and its entry count by kind — every kind in the book, not
   only those on the page being shown.
2. **Given** the book's entries, **When** somebody searches within it, **Then**
   matching entries are returned by the server, across the whole book, with the
   kind filter applying to the search.
3. **Given** an entry this world has changed or hidden, **Then** the page says
   so and shows what was changed.
4. **Given** a book whose system no longer matches the world's, **Then** the
   page says so plainly and still works.
5. **Given** a player at the table, **Then** they can read the book's entries
   the world has switched on, and cannot switch a book on or off, add an entry
   or change one.

---

### User Story 8 - The token list answers "what is on my board, and what else have I got" (Priority: P1)

A Game Master mid-fight opens the token list. The top of it is this scene: four
characters, six NPCs, a vehicle, nine objects, each group counted and
expandable, each row saying what it is and whether players can see it. Below it
is everything the world has that is not here, by type, drillable. They type
"goblin" and get both: the three goblins already on the board and the goblin in
the compendium. They pick the compendium one, it attaches to the cursor, and
they click to drop it in the doorway.

**Why this priority**: It is the same defect as the compendium tabs — structure
stored, not shown — and it is the one that bites during play rather than during
preparation. The panel renders literally nothing today.

**Independent Test**: In a scene with tokens of three kinds, open the token
list: every placed token is there, grouped and counted. Search a name that
exists both on the scene and in the compendium: both halves answer. Choose the
compendium result and place it.

**Acceptance Scenarios**:

1. **Given** a scene with tokens, **When** a person opens the token list,
   **Then** it shows what is on the scene, grouped by kind with a count each,
   each row named in words — never nothing, never only a property sheet for a
   selection, and never a row that reads `Token 3f2a1b9c`.
2. **Given** nothing selected, **Then** the list is still a list. Selecting a
   token MAY add its properties; it MUST NOT be the condition of anything being
   shown.
3. **Given** the list, **Then** below the scene's contents it offers the world's
   content by type — creatures, items, abilities — drillable without a search.
4. **Given** one search box, **When** a person types a name, **Then** both
   halves answer: things already on the scene and things in the world that are
   not, each half labelled so the two are never confused.
5. **Given** a result already on the scene, **When** a person chooses it,
   **Then** it is selected and brought into view on the canvas — not placed a
   second time.
6. **Given** a result not on the scene, **When** a person chooses it, **Then**
   it attaches to the cursor and a click places it, which is the gesture
   `beginTokenPlacement` already implements from the actors panel.
7. **Given** a player using the list, **Then** they find only what they may
   see: no hidden NPC, no token whose name is hidden from them, and nothing
   from the world's content they could not find in the compendium.
8. **Given** a Game Master, **Then** every row says whether players can see it,
   as the actors panel already does.

---

### User Story 9 - A world with thousands of entries stays usable (Priority: P2)

A long campaign has three thousand lore entries, two thousand items and a
switched-on book of eight thousand entries. Every tab opens, every filter
responds, and the tree expands without the page stopping.

**Why this priority**: Every requirement above is about a list, and a list that
is only correct is not a feature. It is second because correctness comes first.

**Independent Test**: Seed the volumes above, then measure the bounds in SC-020
to SC-024 on each tab.

**Acceptance Scenarios**:

1. **Given** the seeded world, **When** a person opens any compendium tab,
   **Then** it is usable within the bound in SC-020.
2. **Given** the lore tree, **When** a person expands a folder of five hundred
   children, **Then** it expands within the bound in SC-022.
3. **Given** the item table, **When** a person types in the search box, **Then**
   results settle within the bound in SC-021 and intermediate keystrokes do not
   each cost a round trip.
4. **Given** any list, **Then** it is paged or virtualised, and the page never
   holds every row of a world at once.

---

### Edge Cases

- **An entry that is both a folder and a page.** A folder is a lore entry with
  children; nothing stops it having content of its own. A tree must show both
  and must not hide an entry's body because it has children.
- **A folder moved under its own descendant.** Refused, as `would_cycle`
  already refuses it (`mutations_lore_tree.rs:236`), with a reason a person can
  read.
- **A folder deleted with children in it.** Today the children are reparented
  to the grandparent (`mutations_lore.rs:368-370`). The tree must say that is
  what will happen before it happens, because in a file manager it would not
  be.
- **A hidden entry with visible children.** The tree must not leak the hidden
  parent's title as a path segment. Its visible children are shown under the
  nearest visible ancestor.
- **Hiding a folder.** Hiding a parent hides its descendants for a player,
  whatever each descendant's own flag says, and un-hiding the parent restores
  each descendant to its own flag rather than making them all visible.
- **An item with no type**, because it was made before this feature or by an
  import. It lands in the unrecognised bucket, is never dropped, and a Game
  Master can set its type.
- **A world whose system pack declares no item vocabulary.** Every item is
  unrecognised, the type tabs collapse to one, and nothing else about the tab
  breaks. Only one pack declares even a content vocabulary today
  (`packs/systems/dnd5e/system.json:1033`).
- **A world whose system is changed under it.** Item types that the new system
  does not know become unrecognised rather than being deleted or rewritten, the
  way an ability's classification already does
  (`AbilityCompendiumTab.tsx:206-218`).
- **A tag that differs only by case or trailing space.** One tag.
  `normaliseTag` (`loreTree.ts:150`) already decides this for lore and must
  decide it for items too, rather than a second rule being written.
- **A search term matching a moderated-disabled thing.** It is not returned, as
  the lore list already applies the spec 015 filter
  (`queries/lore.rs:244`).
- **A search term matching content in a book the world has not switched on.**
  Not returned. A book's contents reach a world through `world_books` or not at
  all.
- **A person who is a Game Master in one world searching in another.** The
  search is world-scoped and their role in another world means nothing here.
- **A book switched off while somebody has its page open.** The page says the
  book has left the shelf; kept additions stay, as `KeptAdditions` already
  shows.
- **A token on the scene whose actor has been deleted.** It is still on the
  board, so it is still in the scene half of the list, named as the token is
  named; it simply has nothing in the world half to correspond to.
- **A token placed from a compendium thing that is later changed.** The list
  shows what is on the scene as it is on the scene, not as its source now
  reads. The two halves are two facts, not one fact twice.
- **A scene with no tokens at all.** The list says the scene is empty and still
  offers the world's content below it. "Nothing placed yet" is a list; nothing
  at all is the defect being fixed.
- **A player opening the token list.** They get it, under their own visibility:
  no hidden NPC, no token whose name is hidden from them, and no placement
  control they would be refused (FR-079).
- **The scene changes while the list is open** — somebody else places or
  removes a token. The scene half follows, as the canvas already does.
- **A placement armed and then abandoned.** The engine owns the carry and the
  cancel (`index.ts:1225-1233`); the list does not track it and must not offer
  its own way to abandon one.
- **An administrator who is also a Game Master of a world.** They see what
  their role in that world allows, plus the instance-wide facts of US5.
  Administrator is not a way into a book's text (`book_list.rs:130-133`).

## Requirements *(mandatory)*

### Functional Requirements

**The compendium as a whole**

- **FR-001**: Every compendium tab MUST keep its filter, sort, search term and
  selected type in the address, so a view can be reloaded, bookmarked and sent
  to somebody at the same table.
- **FR-002**: Every compendium list MUST say how many things it is showing and
  of how many, and MUST never present a filtered count as a total.
- **FR-003**: Every compendium surface MUST show only what its reader may see.
  A count, a breadcrumb, a folder, a tag suggestion and a search result are all
  surfaces under this requirement.
- **FR-004**: The world navigation MUST offer every compendium tab, Books
  included. Books is reachable today only by clicking the tab or typing
  `?tab=books` (`WorldSidebarNav.tsx:73-99`).
- **FR-005**: Where two tabs do the same thing — a text box, a type filter, a
  tag filter, a sorted table — they MUST behave the same way, use the same
  words and honour the same address parameters.

**The lore tree in the product**

- **FR-010**: The Lore tab MUST present entries as a tree built from their
  stored parent (`world_lore_entries.parent_id`), expandable and collapsible,
  in place of today's flat table (`LoreCompendiumTab.tsx:128-136`).
- **FR-011**: The tree MUST be built from the tree library the product already
  has (`apps/web/src/pages/world/lore/loreTree.ts`), not from a second
  implementation. Two tree builders will disagree, and the entry detail page's
  breadcrumb is where that would first show.
- **FR-012**: A person with rights to create lore MUST be able to create a
  **folder** from the tree, at a chosen position.
- **FR-013**: A folder MUST be a lore entry with children — no new entity, no
  new table. An entry MAY be both a folder and a page, and a tree MUST show a
  folder's own content when it has some.
- **FR-014**: A person MUST be able to create an entry **inside a chosen
  folder** in one step. Creation MUST accept a parent; today
  `CreateLoreEntryInput` does not, so every new entry is a root until moved.
- **FR-015**: A person MUST be able to move an entry or a folder within the
  tree from the tree itself, and a move MUST carry the moved thing's
  descendants with it.
- **FR-016**: A move that would make a cycle MUST be refused with a reason a
  person can read, as `would_cycle` already refuses it
  (`mutations_lore_tree.rs:236`).
- **FR-017**: A person MUST be able to rename an entry or folder from the tree,
  and a rename MUST NOT break any link to it. Spec 012's FR-012a already makes
  an entry's address opaque and therefore rename-proof.
- **FR-018**: Deleting a folder MUST state, before it happens, what will happen
  to what is inside it — today the children are reparented to the grandparent
  (`mutations_lore.rs:368-370`).
- **FR-019**: The tree's expansion state MUST survive leaving and returning to
  the tab.
- **FR-020**: Who may create, move, rename and delete MUST be exactly who may
  today: creation by the Game Master (spec 012 FR-002,
  `mutations_lore.rs:121`), move and tag by an Editor
  (`mutations_lore_tree.rs:197`), delete by an Owner
  (`mutations_lore.rs:332`). This spec adds no permission and removes none.

**A folder in the app and a directory in the repository**

- **FR-021**: A folder in the app and a directory in the synchronised
  repository (spec 034) MUST be the same thing, because both are
  `world_lore_entries.parent_id` and neither may be derived from anything else.
  Specifically:
  - an entry's directory path MUST be its chain of ancestors, in order, and
    nothing else;
  - creating a folder in the app MUST produce a directory in the repository on
    the next synchronisation, and a move in the app MUST produce a move in the
    repository, as spec 034's FR set already requires
    (`specs/034-lore-git-sync/spec.md:96-99`, `:105`);
  - an entry that is both folder and page MUST have one settled representation
    in the repository, and spec 034 MUST state which — the product side of this
    requirement is only that the app never invents a second parent.
- **FR-022**: The app MUST NOT let a person build a tree that spec 034 has
  already declared unrepresentable — excessive nesting depth or path length
  (`specs/034-lore-git-sync/spec.md:236-238`) — without saying so at the moment
  of the move, rather than at the next synchronisation.
- **FR-023**: A rename in the app MUST NOT be mistaken for a delete and a
  create by the repository. Spec 034 already keys an entry to its file
  independently of that file's path (`spec.md:560`); this requirement only
  forbids the app from breaking that key.

**Lore a player may see**

- **FR-024**: Lore MUST gain a visibility axis separate from its permission
  ladder, of the shape the product already uses twice —
  `world_abilities.gm_only` (`schema.rs:924`) and
  `world_actors.visible_to_players` (`schema.rs:1087`). The ladder cannot
  express it: `Viewer` is both floor and default
  (`permissioned_entities.rs:41-46`), which is why every member sees every
  entry today. **The default for an existing entry is decided in Q1.**
- **FR-025**: A hidden entry MUST NOT appear in a player's tree, in a player's
  search results (FR-060), in any count shown to a player, or as a segment of
  any breadcrumb a player can see.
- **FR-026**: Hiding a folder MUST hide its descendants from players regardless
  of each descendant's own setting, and un-hiding it MUST restore each
  descendant to its own setting rather than revealing all of them.
- **FR-027**: A folder with no descendant a player may see MUST NOT appear in
  that player's tree.
- **FR-028**: A Game Master MUST see the whole tree with every hidden thing
  marked as hidden, and MUST be able to hide and show an entry or folder from
  the tree and from the entry's own page — as
  `setActorVisibleToPlayers` already allows for an NPC
  (`mutations_actors.rs:351`).
- **FR-029**: Refusing a player the address of a hidden entry MUST NOT confirm
  that the entry exists.

**Items as an index**

- **FR-030**: An item MUST have a **type**, from a vocabulary the world's
  system pack declares, in the shape `world_abilities.classification`
  (`schema.rs:923`) and the ability vocabulary
  (`src/server/src/ability_vocabulary.rs`) already establish. One way of saying
  "this is a kind of thing", not a second.
- **FR-031**: A system pack MUST be able to declare its item vocabulary
  alongside the ability vocabulary it declares today. A pack that declares none
  MUST work: every item is unrecognised and nothing else changes.
- **FR-032**: An item whose type the world's system does not recognise MUST
  appear in an **unrecognised** bucket, named as such, and MUST NEVER be
  hidden, dropped or silently retyped — the behaviour
  `AbilityCompendiumTab.tsx:206-218` already has.
- **FR-033**: The Items tab MUST show the system's item types with a count
  each, and selecting one MUST filter the table to it — as the abilities tab
  does (`AbilityCompendiumTab.tsx:220-241`, `:283-320`).
- **FR-034**: The item table MUST be sortable by name, type and price, and the
  active sort MUST be visible. Sorting MUST apply to the whole filtered set,
  not to the page in view.
- **FR-035**: The item table MUST offer filters for type, tag and price range,
  combinable, each clearable on its own and all clearable at once.
- **FR-036**: Item search MUST distinguish **what an item is** from **what its
  text says**. A term matching a type or a tag MUST be offered as a filter
  rather than silently searched as text, and results MUST say which field
  matched.
- **FR-037**: Item search MUST be served by the server over the world's whole
  item set — never over the page in view — and MUST be debounced so that typing
  does not cost one round trip per character (`ItemCompendiumTab.tsx:57`,
  `:75`).
- **FR-038**: Existing items MUST survive this change with their type
  unrecognised, and a Game Master MUST be able to set the type of one or of
  many at once.
- **FR-039**: The table MUST read well for a system with many item types. Its
  proof is SC-011: a fifth-edition or Pathfinder world with at least eight
  types and two hundred items.

**Tags**

- **FR-050**: A tag MUST be **per world**, not per instance and not per system.
  A world's vocabulary is its campaign's; a system's is a starting point, not a
  cage. `world_lore_tags` (`schema.rs:1465-1473`) is already per entry within a
  world, and items MUST gain the same, on the same terms.
- **FR-051**: A system pack MAY declare tags. A declared tag MUST be present in
  a world of that system from the start and MUST be marked as coming from the
  system, so a person can tell a shared word from one their table invented.
- **FR-052**: A declared tag MUST NOT be removable from the vocabulary and MUST
  NOT constrain what else a world may add. A pack proposes; a world disposes.
- **FR-053**: Anybody who may edit a thing MAY tag that thing, and tagging it
  MAY introduce a new tag to the world. Nobody may tag a thing they may not
  edit. This deliberately gives a world no separate tag-administration
  permission: the existing ladder decides it.
- **FR-054**: Tags MUST be normalised identically wherever they are applied —
  one normalisation function for lore and items, `normaliseTag`
  (`loreTree.ts:150`), not two. Case and surrounding space MUST NOT produce a
  second tag.
- **FR-055**: A tag box MUST offer the world's existing tags as a person types,
  before it offers to create one.
- **FR-056**: A tag no thing carries MUST stop being offered. No person is ever
  asked to tidy a tag list.
- **FR-057**: A tag MUST be filterable on every tab that shows tagged things,
  with the same control and the same address parameter (FR-005).

**Search across the compendium**

- **FR-060**: The compendium MUST offer **one search** across lore, items,
  NPCs, abilities and the entries of books the world has switched on, from one
  box, returning results grouped by kind with a count each.
- **FR-061**: Every result MUST say what kind of thing it is before it is
  opened, and MUST open at that thing.
- **FR-062**: Search MUST apply every visibility rule the product holds, at the
  server, and MUST NOT rely on a client filtering results it was given:
  - a hidden NPC MUST NOT be findable by a player —
    `world_actors.visible_to_players`, enforced by
    `auth::npc_visibility::retain_visible_sync`
    (`npc_visibility.rs:180-193`);
  - a Game-Master-only ability MUST NOT be findable by a player —
    `world_abilities.gm_only`;
  - a hidden lore entry MUST NOT be findable by a player (FR-024, FR-025);
  - content disabled by moderation MUST NOT be returned to anybody, as the lore
    list already refuses it (`queries/lore.rs:244`);
  - a book the world has not switched on MUST contribute nothing.
- **FR-063**: A Game Master's results MUST mark each thing a player could not
  find, so that a Game Master can tell what they are showing.
- **FR-064**: A term matching a type or a tag MUST be offered as a filter, and
  the search MUST say when it did so.
- **FR-065**: Search MUST be one mechanism. Four tabs search four ways today —
  a client title filter, two server queries and an IndexedDB FlexSearch index
  (`NpcCompendiumTab.tsx:81-127`) — and a compendium search that agrees with
  none of its tabs is worse than none.
- **FR-066**: Search MUST cover a lore entry's **body and tags**, not its title
  alone. Today lore is filtered client-side on title only
  (`LoreCompendiumTab.tsx:87-95`).
- **FR-067**: The search term and filters MUST be in the address (FR-001).

**The play field's token list**

- **FR-070**: The play field's token list MUST be a list. It MUST show
  something useful when nothing is selected; today the tool-rail panel returns
  nothing at all in that case (`TokenTool.tsx:192`).
- **FR-070a**: Every row MUST name the thing **in words** — the token's name,
  or its actor's — and MUST NOT render an identifier as the only name. Today a
  row reads `Token 3f2a1b9c` (`TokenPanel.tsx:385-388`) while the row model
  already carries a name (`apps/web/src/api/tokens.ts:8-25`). This is the same
  defect spec 053 named on the moderation screen and it gets the same answer.
- **FR-070b**: The product MUST end with **one** token list. Two surfaces named
  Tokens — a tool-rail property sheet and a modal overlay that covers the map
  (`TokenPanel.tsx:341-344`) — is how "the tokens list" came to mean two
  different things to the same person. Whichever survives, it sits beside the
  map rather than over it.
- **FR-070c**: Choosing a row MUST select the thing on the canvas and bring it
  into view. Administrative controls MAY be reachable from a row; they MUST NOT
  be what a click does, and an owner MUST NOT be set by typing a user identifier
  into a box (`TokenPanel.tsx:465-491`).
- **FR-071**: Its first half MUST be **what is on this scene**, grouped by kind
  with a count each, from the kinds the product already stores and validates —
  character, NPC, vehicle, object (`tokens.token_type`, `schema.rs:816`;
  `TokenKind`, `token_kind.rs:40-62`). A group MUST be expandable and
  collapsible, and the breakdown MUST cover everything on the scene, not only
  tokens: where the scene carries lights, walls or interaction points, they are
  named in their own groups. Every one of those already has a server query that
  answers for the scene — `walls` (`queries/scene.rs:335`), `light_sources`
  (`:372`), `shapes` (`:410`), `interactives`
  (`queries/interactives.rs:208`) — and the list MUST use them rather than a
  count. `InteractionTool` shows "N things on this scene respond"
  (`InteractionTool.tsx:316-321`); a count is what you write when you have the
  rows and nowhere to put them.
- **FR-072**: Its second half MUST be **the world's content that is not on this
  scene**, offered by type and drillable without typing anything — the same
  types the compendium tabs show (FR-033), so a person learns one vocabulary.
- **FR-073**: One search box MUST cover both halves. A term MUST return what is
  on the scene and what is in the world, in two labelled groups, so that "the
  three goblins here" and "the goblin in the compendium" are both findable and
  never confused for each other.
- **FR-074**: That search MUST be the compendium search of FR-060, with the
  scene added — not a second search with its own rules. FR-062's visibility
  rules apply unchanged, and the scene half MUST honour what the server already
  enforces: a name hidden from players is withheld by `tokens(sceneId)` itself
  (`queries/scene.rs:283-287`,
  `npc_visibility::player_may_read_token_name`, `npc_visibility.rs:285-286`),
  content under takedown returns nothing (`scene.rs:259-261`), and
  Game-Master-only shapes are already filtered (`scene.rs:410`). The list MUST
  read those answers rather than asking its own question.
- **FR-075**: Choosing a result **already on the scene** MUST select it and
  bring it into view. It MUST NOT place a second copy.
- **FR-076**: Choosing a result **not on the scene** MUST arm placement — the
  thing attaches to the cursor and a click drops it. This MUST be the gesture
  the product already has (`beginTokenPlacement`,
  `apps/web/src/engine/bevy/index.ts:1234`; `beginPropPlacement`, `:1256` for
  something that is not an actor), and this spec MUST NOT add a second way to
  carry something onto a map.
- **FR-077**: Every row MUST say, to a Game Master, whether players can see the
  thing — as `ActorsPanel` already does (`ActorsPanel.tsx:259-270`).
- **FR-078**: The panel MUST NOT require a selection to be useful, and a
  selected token's properties MAY continue to be shown alongside the list. The
  property sheet is not deleted by this requirement; it stops being the whole
  panel.
- **FR-079**: Who may place, move or change anything MUST be decided by the
  same facet that decides it today. Finding a thing in a list never grants a
  person the right to place it, and a control that would be refused MUST NOT be
  offered.

**Where this spec stops, and the placing spec starts**

- **FR-079a**: Spec 056, **placing and reaching things on a map** — walls,
  objects, interaction points, the single "place here" menu, and the decision
  that an object is scenery until it is made interactive — owns **the gesture
  and what happens on the map**: arming a placement, the carried preview,
  snapping, the drop, what is created, whether the created thing is scenery or
  interactive, and the menu a person opens by pointing at the map. This spec
  MUST NOT restate any of it and MUST NOT specify a second placement gesture.
- **FR-079b**: This spec owns **finding the thing**: the list, its breakdown of
  what is on the scene, its drill-down into the world's content by type, the
  one search across both, and which results a given viewer may see. It hands a
  chosen thing to the placing spec's gesture and stops there.
- **FR-079c**: The seam between them is exactly one handover — *this thing,
  now, on the cursor*. Where the two specs must agree, they agree on that
  handover and on nothing else. Neither may define what the other owns; if a
  question is about what appears in the list or who may see it, it is this
  spec's, and if it is about what happens after the click, it is the placing
  spec's.

**Books**

- **FR-080**: A book's page MUST name the book, its game system, its page
  count and its entry count **by kind for the whole book** — not for the page
  of results in view, which is what the kind filter is built from today
  (`BookListTab.tsx:464`, `:476-495`).
- **FR-081**: A book's entries MUST be searchable by text, at the server,
  across the whole book, with the kind filter applying to the search. The query
  behind the browser accepts `kind`, `after`, `first` and `showHidden` and no
  search argument today (`worldBooks.ts:253-297`).
- **FR-082**: A book's page MUST show what this world has changed, hidden or
  added in it, and MUST show what a changed entry looked like before.
- **FR-083**: A book's page MUST say when the world's system no longer matches
  the book's, and MUST keep working.
- **FR-084**: Every member of a world MUST be able to read the entries of a
  book the world has switched on, and MUST NOT be able to switch a book on or
  off, add an entry, change one or hide one. That is today's rule —
  `books_on` is readable by every member (`book_list.rs:258-264`) and
  `require_book_manager` gates the rest (`:140`) — and this spec keeps it while
  giving players a page that is worth having.
- **FR-085**: Adding an entry to a book MUST offer the kinds the book contains
  rather than a free-text box (`AddEntryForm`, `BookListTab.tsx:848-858`).
- **FR-086**: The book browser MUST pass a page size rather than leaving it to
  a default (`entriesFrom`, `:419`, `:437`).
- **FR-087**: An administrator MUST have an instance-wide view of books that
  answers: how many books exist, on which accounts, in which systems, their
  entry counts, their parser versions, their storage, which are the same source
  imported more than once, and which worlds have each switched on
  (`worlds_with_book`, `book_list.rs:503`).
- **FR-088**: That view MUST say which books were parsed by an older reader
  than the instance now has, and what re-parsing would change.
- **FR-089**: An administrator MUST NOT be able to read the entry text, field
  values or prose of a book they do not own, through this view or any other.
  `require_book_manager` has no admin bypass today and says so deliberately
  (`book_list.rs:130-133`); **an operator's instance-wide view is about books,
  not about their contents.** Where an operator legitimately needs a book's
  contents — a takedown naming an entry — that is spec 053's moderation path,
  which records who read what (spec 053 FR-042).
- **FR-090**: A book under moderation MUST be marked as such in both views,
  linking to the case (spec 053).

**Performance**

- **FR-100**: Every compendium list MUST be paged or virtualised. No tab may
  hold every row of a world in the page at once.
- **FR-101**: Every filter, sort and search MUST be applied by the server over
  the whole set, never by the client over the page in view. A count that
  describes only what was downloaded is a wrong count (FR-002).
- **FR-102**: The lore tree MUST render without loading every descendant of
  every collapsed folder.
- **FR-103**: The bounds are SC-020 to SC-024, measured on the world SC-020
  describes. They are requirements, not aspirations: a tab that misses one is
  not done.

**Proof**

- **FR-110**: An end-to-end test MUST prove that a folder can be created, an
  entry created inside it, an entry moved into it and renamed, and that the
  tree is unchanged after a reload.
- **FR-111**: An end-to-end test MUST prove that a player's tree omits a hidden
  entry, omits a folder whose descendants are all hidden, shows a folder with
  one visible child, and that no count or breadcrumb shown to that player names
  a hidden thing.
- **FR-112**: An end-to-end test MUST prove that a player cannot reach a hidden
  entry by its address, and that the refusal does not confirm it exists.
- **FR-113**: An end-to-end test MUST prove that a hidden NPC, a
  Game-Master-only ability and a hidden lore entry are all absent from a
  player's compendium search and all present, marked, in a Game Master's.
- **FR-114**: An end-to-end test MUST prove that the items table filters by
  type, filters by tag, sorts by price, and that the counts on the type tabs
  match the rows.
- **FR-115**: A test MUST prove that an item with an unrecognised type appears
  in the unrecognised bucket and is never dropped, including after the world's
  system is changed.
- **FR-116**: A test MUST prove that a tag differing only by case or trailing
  space is the same tag, on an item and on a lore entry.
- **FR-117**: An end-to-end test MUST prove that a phrase deep in a book of
  several thousand entries is found by searching within the book, and that the
  kind filter narrows that search.
- **FR-118**: A test MUST prove that an administrator's books view lists a book
  they do not own and returns **no** entry text for it.
- **FR-117a**: An end-to-end test MUST prove that the token list shows the
  scene's contents with nothing selected, that one search returns both a token
  on the scene and a compendium thing of the same name in labelled groups, and
  that choosing the compendium one arms placement and a click places it.
- **FR-117b**: An end-to-end test MUST prove that a player's token list omits a
  hidden NPC and a token whose name is hidden from them, while the Game
  Master's shows both, marked.
- **FR-117c**: A test MUST prove that every row in the token list names its
  thing in words, and that choosing a row selects that thing on the canvas.
- **FR-119**: A performance test MUST measure SC-020 to SC-024 on a seeded
  world of the stated size, and MUST fail the build when a bound is missed.

### Key Entities

- **Lore tree**: the parent-child shape of a world's lore entries, already
  stored as `world_lore_entries.parent_id` and mirrored into repository
  directories by spec 034.
- **Folder**: a lore entry that has children. Not a new kind of thing.
- **Lore visibility**: a new axis, separate from the permission ladder, saying
  whether players may see an entry. The shape abilities and NPCs already have.
- **Item type**: what kind of thing an item is, from a vocabulary the system
  pack declares. New; the ability classification is its model.
- **Item vocabulary**: a system pack's declared item types, alongside the
  ability vocabulary it declares today.
- **Tag**: a word a world applies to its own content. Per world, shared between
  lore and items, seeded by the pack, extended by anybody who may edit the
  thing they are tagging.
- **Compendium search**: one query across a world's lore, items, NPCs,
  abilities and switched-on book entries, answered by the server under the
  reader's own visibility.
- **Book**: one imported source book — a `compendiums` row owned by an account,
  its `compendium_entries`, and the `world_books` row that switches it on for a
  world.
- **Token list**: the play field panel that says what is on this scene and what
  else the world has, under one search. Today it is two surfaces: a tool-rail
  property sheet that renders nothing without a selection, and a modal overlay
  that lists identifiers.
- **Token kind**: character, NPC, vehicle or object — the closed set the product
  already validates on write and draws from.
- **Placement handover**: choosing a thing in the list and handing it to the
  engine's carry. This spec's last act; the placing spec's first.
- **Instance books view**: an administrator's view of the books on an instance:
  what they are, not what they say.

## Success Criteria *(mandatory)*

### Measurable Outcomes

**Finding things**

- **SC-001**: A Game Master can create a folder and put a new entry inside it
  in **two steps** from the Lore tab, with **zero** visits to an entry's detail
  page.
- **SC-002**: **100%** of lore entries appear under their stored parent in the
  tree, and **0** appear at the root because creation could not accept one.
- **SC-003**: **0** hidden entries, and **0** titles of hidden entries, appear
  anywhere a player can reach — tree, counts, breadcrumbs, search results and
  direct addresses, each checked.
- **SC-004**: A folder in the app and its directory in a synchronised
  repository agree in **100%** of comparisons after a move, a rename and a
  create.
- **SC-010**: An item's type is shown on **100%** of item rows, and **0** items
  are absent from the table because their type is unrecognised.
- **SC-011**: In a world of at least **eight** item types and **200** items, a
  person can narrow to one type, add a tag filter and sort by price in **three**
  interactions, and the type counts match the rows in **100%** of checks.
- **SC-012**: **0** tags in a world differ only by case or surrounding space.
- **SC-020**: In a world of **3,000** lore entries, **2,000** items, **500**
  NPCs, **500** abilities and one switched-on book of **8,000** entries, every
  compendium tab is interactive within **1.5 seconds** at the 95th percentile,
  measured from navigation on a cold cache.
- **SC-021**: In that world, a search or filter returns within **400
  milliseconds** at the 95th percentile, and typing a ten-character term costs
  at most **three** requests.
- **SC-022**: In that world, expanding a folder of **500** children renders
  within **200 milliseconds** at the 95th percentile.
- **SC-023**: In that world, **0** compendium responses carry more than
  **200** rows, and **0** tabs hold the world's whole set in the page.
- **SC-024**: A compendium search across all five kinds returns within **800
  milliseconds** at the 95th percentile in that world.
- **SC-030**: A player and a Game Master searching the same term in the same
  world see result sets that differ by exactly the things the Game Master may
  see and the player may not — **0** unexplained differences either way.
- **SC-031**: **100%** of visibility exclusions are applied by the server:
  **0** results are returned to a client and then hidden by it.

**The token list**

- **SC-060**: With nothing selected, the token list shows the scene's contents
  in **100%** of runs, and renders an empty panel in **0%**.
- **SC-061**: **100%** of tokens on a scene appear in the list's scene half,
  under a group naming their kind, with counts that match the rows, and **0**
  rows render an identifier as the only name.
- **SC-061a**: The product offers **one** token list, and choosing a row
  selects that thing on the canvas in **100%** of runs.
- **SC-062**: A name present both on the scene and in the world's content is
  returned in **both** halves from **one** search, labelled, in **100%** of
  runs.
- **SC-063**: Choosing a result not on the scene arms placement in **one**
  interaction and places the thing on the next click, in **100%** of runs, with
  **0** placement gestures other than the one the placing spec owns.
- **SC-064**: Choosing a result already on the scene selects it and places
  **0** additional copies.
- **SC-065**: A player's token list contains **0** hidden NPCs and **0** names
  hidden from them, verified against a Game Master's view of the same scene.

**Books**

- **SC-040**: A phrase appearing once in a book of **8,000** entries is found
  by searching within that book in **one step**, in **100%** of runs.
- **SC-041**: A book's kind counts are complete for the whole book in **100%**
  of checks, including kinds that appear only after paging.
- **SC-042**: An administrator can answer, in **one step** each, how many books
  the instance holds, which share a source, and which need re-parsing.
- **SC-043**: **0** entry text, field values or prose from a book an
  administrator does not own is reachable through the instance books view.
- **SC-044**: Every member of a world can open a switched-on book and read its
  entries, and **0** of them can switch a book on or off.

**Consistency**

- **SC-050**: Every compendium tab carries its filter, sort and search in the
  address, in **100%** of tabs, using the same parameter names.
- **SC-051**: Every compendium tab is reachable from the world navigation, in
  **100%** of tabs — Books included.
- **SC-052**: The compendium meets WCAG 2.2 AA under an automated audit, and
  the lore tree is fully operable by keyboard: expand, collapse, move, rename.

## Assumptions

- **The tree is stored; this spec shows it.** `parent_id`, `moveLoreEntry`, the
  cycle guard, the reparenting delete and `loreTree.ts` all exist. The missing
  half is a screen, creation that accepts a parent, and a visibility axis.
- **A folder is an entry with children.** No new table, no new entity, no
  second parent. This is what keeps FR-021 possible: spec 034 mirrors one tree,
  and a second notion of a folder would give it two to reconcile.
- **Lore's permission ladder is not a visibility model, and never was.** The
  product says so in the code that generates it
  (`permissioned_entities.rs:41-46`), and the two content types that needed
  hiding each grew a separate flag. Lore is the third.
- **Item type and item tags are new facts, and they are modelled on what
  exists.** An ability's `classification` plus a pack-declared vocabulary plus
  an unrecognised bucket is a working answer to exactly this problem, shipped,
  in this product. Copying it is cheaper than designing it and leaves one
  pattern rather than two.
- **A system pack proposes; a world disposes.** A pack's declared types and
  tags are a starting vocabulary. A world may add, and no world is refused a
  word because its pack did not think of it.
- **Search belongs on the server.** Every visibility rule this spec relies on
  is enforced on the server today, and a client that filters results it was
  already given has already been given them.
- **Nothing here changes who may do what.** Creation, move, delete, tagging,
  switching a book on and reading a book all keep the permissions they have.
  Where this spec says "MUST be able to", it means the person who may already.
- **The token list is the same problem as a compendium tab.** Structure the
  product stores — a token's kind, an NPC's visibility, a name's visibility —
  and does not show. It is in this spec because a second search, with its own
  rules about what a player may find, is exactly what FR-065 exists to prevent.
- **Placement is not this spec's.** The gesture exists
  (`beginTokenPlacement`, `beginPropPlacement`) and the spec on reaching and
  placing things on a map owns it and everything after the drop. This spec hands
  it a thing and stops (FR-079a to FR-079c).
- **An operator's interest in books is administrative.** Counts, versions,
  duplicates, storage and reach — not text. The code already refuses an admin
  bypass into a book, and that refusal is a feature.

## Out of Scope

- Changing what a lore entry, an item, an ability, an NPC or a book **is**.
  Spec 012's revisions and links, spec 025's ability permissions, spec 049's
  import and spec 050's library are read here, not redefined.
- Cross-world search. The compendium is a world's.
- Searching outside a world's content: another account's library, an
  unswitched book, the instance at large.
- Item rarity, attunement, weight, encumbrance or any other system-specific
  property beyond type and tags. A pack may declare a type vocabulary; making
  the product understand what "attunement" means is not this feature.
- Full-text search of a book's PDF beyond the entries the reader extracted.
- A tag hierarchy, tag synonyms or tag merging. One flat per-world vocabulary.
- Bulk editing beyond FR-038's setting of an item's type.
- Redesigning the entry detail page, the item editor or the NPC editor.
- The placement gesture and everything after it: the carried preview,
  snapping, the drop, what gets created, whether it is scenery or interactive,
  and the "place here" menu. The spec on reaching and placing things on a map
  owns all of it (FR-079a).
- Changing what a token is, how it is drawn, or the set of token kinds.
- The selected-token property sheet's own controls — size, facing, art, name
  visibility. FR-078 keeps them; it does not redesign them.
- Moderating a book's contents — spec 053 owns that, and FR-090 only links to
  it.

## Dependencies

- **Spec 012**: the lore wiki — its entries, permissions, revisions, links and
  its FR-012a opaque address, which is what makes FR-017's rename safe.
- **Spec 025**: world abilities, whose `classification`, pack vocabulary and
  unrecognised bucket are the model for an item's type.
- **Spec 026**: content collections, which a compendium search must not confuse
  with a book.
- **Spec 032 / 033**: pack architecture and the ability vocabulary — where a
  system pack's declared vocabulary lives, and where an item vocabulary would.
- **Spec 034**: lore git sync, whose directories are this spec's folders
  (FR-021 to FR-023).
- **Spec 047**: the bestiary, and today's NPC visibility flag that FR-062 must
  respect.
- **Spec 049**: importing a source book — what a book is.
- **Spec 050**: the account library — where a book lives before a world
  switches it on.
- **Spec 030**: interactive elements, whose interaction points appear in the
  scene half of the token list (FR-071).
- **Spec 045**: token movement and vision, and the scene the token list
  describes.
- **Spec 056**: placing and reaching things on a map — owns the placement
  gesture this spec hands to, and the boundary FR-079a to FR-079c states.
- **Spec 053**: moderation across the instance — FR-090's link, and the path by
  which an operator legitimately reads a book's contents.

## Decisions (owner, 2026-09-15)

1. **The lore screen gets the file structure.** Folders, entries, create, move,
   rename — in the tree, not in a dropdown on an entry's own page. The owner's
   words: "create a folder or create a file or create a lore entry… I don't
   think we actually implemented it on the lore screen."

2. **Items get a type and tags, and the table gets built around them.** "A
   really nice indexable table based off of types for items… we should be able
   to assign tags and have rich filtering… like imagine fifth edition or
   Pathfinder."

3. **The search must know what kind of thing it found.** "The search really
   needs to be improved to show that it has different item types." This spec
   reads that as the general requirement it is: one search, across the whole
   compendium, that says what each result is.

4. **The books page is answered, not merely fixed.** The owner asked what the
   page needs. It needs to be a book: what it is, what it holds by kind, what
   this world changed in it, and a way to find a rule inside it — plus an
   instance-wide view for an administrator that is about books and never about
   their contents.

5. **One pattern, not three.** Where the product already has a working answer —
   an ability's classification and pack vocabulary, an NPC's visibility flag,
   `normaliseTag`, `loreTree.ts` — this spec uses it rather than inventing a
   parallel one.

## Questions for the owner

1. **Q1 — What happens to the lore that exists when lore gains a visibility
   axis?** FR-024 adds the axis. Today every member of a world can read every
   lore entry in it (`queries/lore.rs:226-244`), so whatever default is chosen
   changes what somebody already sees. The two live content types chose
   opposite defaults, on purpose: an ability is `gm_only` false by default, and
   an NPC is `visible_to_players` **false** by default — new NPCs are inserted
   hidden (`mutations_actors.rs:117-118`).

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | **Existing entries stay visible; new entries are visible unless hidden** | Nothing a player can read today disappears. A Game Master who has been writing secrets in lore keeps leaking them until they go and hide each one — but they are leaking them today, so nothing gets worse. |
   | B | Existing entries stay visible; new entries are **hidden** unless shown, as NPCs are | Safer for what is written from now on, and matches the newest precedent. Costs a Game Master a "show this" step on every entry they write for players, which on a wiki is most of them. |
   | C | Everything becomes hidden, and a Game Master shows what players should see | Safest, and it takes a running campaign's lore away from its players at an upgrade. |

   **Recommendation: A**, with a Game Master's tree marking every entry's state
   so the hiding is one pass rather than a hunt. Lore is mostly written *for*
   the table — that is what a wiki is — and the NPC precedent comes from the
   opposite case, where a creature the players have not met yet is the norm.
   Option C is the one to refuse outright: taking content away from players at
   an upgrade is a worse failure than continuing to show what is already shown.

2. **Q2 — Does an item have one type or several?** FR-030 assumes one, the way
   an ability has one `classification`. Fifth edition mostly agrees — an item is
   a weapon or armour or a wondrous item — but a magic sword is a weapon *and*
   a magic item, and Pathfinder's categories overlap more.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | **One type, plus tags for everything else** | Type tabs with counts that add up, one filter that is exclusive, and the exact shape the abilities tab already ships. "Magic" becomes a tag. |
   | B | Several types per item | Truer to some systems; type tabs stop being a partition, counts overlap, and the tab row becomes a second tag filter with a different name. |

   **Recommendation: A.** The value the owner asked for — a table indexed by
   type, with counts, that reads like a rulebook's index — comes from type
   being a partition. Anything that is not a partition is a tag, and FR-050
   gives tags everything they need.

3. **Q3 — Should a pack's declared item types be per system only, or may a
   world add its own?** FR-031 has the pack declare the vocabulary; FR-032 puts
   anything else in an unrecognised bucket. A world that invents "Relic" would
   have every Relic in that bucket forever.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Pack only. Unrecognised is where anything else lives | Simplest, matches abilities exactly, and makes a world's own category a second-class thing. |
   | B | **A world may add a type to its own vocabulary; the pack's are marked as the system's** | Same rule tags get under FR-051 and FR-052 — the pack proposes, the world disposes — and one rule for both is easier to explain than two. |

   **Recommendation: B**, for consistency with tags rather than with abilities.
   If this is taken, the abilities tab should get the same eventually, so the
   two stay one pattern; that is its own change and not this spec's.
