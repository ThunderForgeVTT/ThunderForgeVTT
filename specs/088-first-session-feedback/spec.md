# Feature Specification: First Session Feedback

**Feature Branch**: `088-first-session-feedback`
**Created**: 2026-10-09
**Status**: Planned (plan.md, tasks.md)
**Input**: The owner's notes from the first real session on vtt-dev, with
three players, 2026-10-09:

1. `/admin/mail`: "we should delta if the user changed anything and save
   that".
2. The actor view (`/world/<w>/actor/<a>/view`): "has a lot of usable
   horizontal space and needs to be responsive".
3. The world page (`/world/<w>`): "needs better use of horizontal space on
   mobile and desktop and ultra wide".
4. The players page (`/world/<w>/players`): GM-only invite links "that are
   exclusive", "revocable URLs", and the invitee must "already be members of
   ThunderForge".
5. "any new world should preload one of our base maps".
6. Players launching their own hero editor and character sheet from the
   players screen.
7. **Clear rolls**, added the same day: a GM pressed something expecting it
   to clear the roll feed, and one player's feed kept the rolls. No such
   feature exists.
8. **Walls at a map's edges**, added the same day: "if someone adds a map
   like a vtt can we auto add walls to the outside of the map? like if i
   upload our ambush".

## Why

Three players and a GM used ThunderForge for a whole evening for the
first time. Each note above is a place where the table stopped playing to
work around the tool:

- the GM made a new world and got an empty grey board;
- the GM handed out a link that anyone holding it could use;
- players squinted at a character sheet held to a 672 px column on a wide
  monitor;
- a player could not find their own sheet from the screen that lists them;
- the GM had no way to give the table a clean feed between scenes;
- the ambush map has no walls, so tokens walked off its edge.

The admin form is the owner's own friction: every mail setting has its own
Save button, so changing three of them means three saves, and nothing warns
before a half-finished edit is lost.

None of this is a new subsystem. Each story is small and stands on its own,
so they share a spec only because they share a source and a proof run.

## What exists

Counted on 2026-10-09 against `main` at `caafdd6d`. research.md has the
detail, with lines.

- **Mail settings** (`apps/web/src/pages/admin/components/MailPanel.tsx`)
  are one `SettingRow` per key (`InstanceSettingsPanel.tsx:163-339`), each
  with its own Save and Clear. Save calls
  `updateInstanceSetting(key, value)`, one key per call, by design ("no
  bulk form", `settings/graphql.rs:340`). `value: null` clears a key back to
  its default. `mail.password` is write-only: the server returns
  `secretState: SET | NOT_SET` and never a value. A row's draft is set once
  on mount and not reset when its setting changes. Nothing in `apps/web`
  warns about unsaved changes (`beforeunload`, `useBlocker`, `dirty`: none).
  The app uses `react-router-dom` 7.18 in declarative mode (`BrowserRouter`),
  so `useBlocker` is not available.
- **The actor view** (`pages/world/actor/ActorDetailPage.tsx:387`) is one
  column, `Container className="max-w-2xl"`: 672 px at every width, with no
  breakpoints.
- **The world page** (`pages/world/WorldDashboardPage.tsx:135`) sits in a
  1160 px `Container`, with one `md:grid-cols-2` section. The world's
  section pages use `WorldSectionShell`, which is 1800 px wide.
- **World links** already exist:
  - `world_invites` holds a code, `max_uses`, `used_count`, `expires_at`,
    `revoked` and `rotated_from`.
  - `generateInviteCode`, `revokeInviteCode`, `rotateInviteCode` and
    `worldInvites` are Owner and GM only on the server.
  - `joinWorld` needs a signed-in account, and a world link never admits
    anyone to the instance. That is the job of `instance_invitations`,
    `/invite/<code>`, which an administrator creates.
  - The gaps:
    - Links are managed from `CampaignSettingsPanel` on the world page,
      which the client does not gate by role.
    - The players page has no link controls.
    - Both callers mint 5-use links with no expiry.
    - Codes are 20 hex characters cut from a UUIDv4, about 76 random bits.
    - `worldByInviteCode` answers a signed-out caller with the world's name
      and description.
    - Every refusal is the same `LINK_UNAVAILABLE_MESSAGE`, so a revoked
      link is not told apart from a mistyped one.
    - A signed-out visitor is sent to `/login?returnTo=/join/<code>`, and
      that page offers registration and OAuth sign-in. OAuth's first sign-in
      creates an account (`auth/oauth.rs:394`).
    - The world-link URL pages are `noindex` by `<meta>` only, and no
      `X-Robots-Tag` header is sent anywhere.
- **The example maps**: there are seven `.dd2vtt` files in `examples/maps/`,
  the owner's own work, CC BY-SA 4.0, published at
  <https://github.com/mbround18/vtt-maps>, with the catalog at
  <https://vtt-maps.dnd-apps.dev/catalog>.
  - `thunderforge-demo-maps` (`apps/thunderforge/src/bin/demo_maps.rs`)
    turns them into WebP images, thumbnails, a `maps.json` catalog with
    walls and lights, and a `NOTICE.txt`, for the demo.
  - The credit lives in `apps/demo/credit.json`.
  - The web app shows none of them. The Starting Scene that `createWorld`
    makes (`mutations_worlds.rs:50-102`) has no background, and its doc
    comment still calls the maps non-redistributable.
- **Rolls** live in `world_roll_records`, and `world_attacks` references
  them (`ON DELETE SET NULL`). The feed (`worldRolls`), the single fetch
  (`worldRoll`), the live subscription and the catch-up all decide who sees
  what through `rolls/visibility.rs`, as CONTRIBUTING requires. The world
  events in use run up to 38 (`EVENT_CODE_AUTHORING_TOOLS_CHANGED`,
  `world_events.rs:271`). There is no way to clear the feed.
- **Map import** (`crates/thunderforge-server/src/map_import/`) turns a
  UVTT file's `line_of_sight`, `objects_line_of_sight` and `portals` into
  wall rows (`geometry.rs`), all in one transaction with one
  `MAP_IMPORTED` (13) event (`mod.rs:115-410`).
  - It adds nothing at the map's edge.
  - `examples/maps/grassy-path-ambush.dd2vtt` has no line of sight, no
    portals and no lights (48 × 27 cells at 128 px). Its vtt-dev import
    recorded `walls_created: 0`, so nothing stops a token or sight at the
    edge.
  - A re-import adds its walls beside the ones already there; it removes
    none.
  - `walls.metadata` is a free JSONB column.
  - `offline.rs` (`import_offline`, :75) is the same import with no
    database. It feeds `thunderforge-demo-maps`, and so the demo's seed. The
    demo's own in-browser import (`apps/demo/src/backend/mapImport.ts`)
    mirrors the server's module.
  - A plain image set as a background goes through `uploadCanvasImage`,
    then `updateSceneLevel` (`mutations_levels.rs:116`), and gets no walls.
- **The players page** links a claimed character's name to its view and
  nothing else. The hero builder opens only from the actor page's
  imagery panel.

## Related hotfixes

These came out of the same session. Each is being fixed on its own branch,
in its own worktree, and none is part of this spec. On 2026-10-09 all four
branches still pointed at `caafdd6d`, with their work uncommitted.

| Branch | What it fixes | How 088 relates |
| --- | --- | --- |
| `hotfix-world-permissions` | Players saw GM-only world controls: settings, delete and invite links. | **US1 depends on it.** The link panel this spec moves to the players page must already be refused to players on the server and hidden from them on the client. US1's tasks start only once it is on main. |
| `hotfix-invite-uses` | A sign-up refused for a bad password or username still used up an instance invitation. | No dependency. It and US1 both touch `crates/thunderforge-server/src/auth/`, so US1's sign-in-only task (T028) merges after it to avoid a conflict. |
| `hotfix-map-load-sync` | A GM's map import did not reach the players' boards. | No dependency. US2 applies the map before anyone else is in the world. If the GM later changes it, that is a map import, and it relies on this fix. |
| `hotfix-player-hero-edit` | Players could not edit their heroes, and were not offered the builder and sheet from the Players screen. | **US8 depends on it.** US8 is only the polish on top of it. |

## Decisions already made

- **World links are for people who already have an account.** A world link
  never creates an account, whether by registration or by OAuth. To bring
  someone new to the instance, the administrator sends an instance
  invitation; that is unchanged.
- **"Exclusive" means members only, revocable, and limited if the GM
  wants.** The owner's decision, 2026-10-09. A world link admits only an
  account that already exists on the instance (above), and the GM can
  revoke it at any time. A use limit is optional: by default a link has
  none, and the GM may set one from 1 to 50. A new link expires after 7
  days; the GM may pick 1 day, 7 days, 30 days or no expiry. A link is not
  bound to one named account.
- **A use counts only when someone joins.** A use is spent only when an
  account becomes a new member of the world, in the same transaction as
  the membership row. Opening the link, viewing the join page, a crawler
  or a link preview, a refresh, a second click, an account that is
  already a member (the owner included), and a join that fails all spend
  nothing. A limited link whose last use is taken by a concurrent join
  refuses as used up.
- **A refused link says why**, to a signed-in holder: revoked, expired,
  used up, or not a link at all. This reverses the single
  `LINK_UNAVAILABLE_MESSAGE` at the owner's request. Someone who holds the
  code learns only the state of that code.
- **The GM picks the starting map at world creation**, from the seven base
  maps or **None**. The form selects the default map, Grassy Path Ambush,
  before the GM picks anything (Open item 1). The map is imported as the
  GM's own map import would import it: background, size, grid, walls, doors
  and lights.
- **Clearing rolls hides them; it deletes nothing.** The rolls stay in
  `world_roll_records`, because attacks refer to them and the record is the
  world's history.
- **An imported map is walled at its edges** (US6). Importing a UVTT map,
  or setting a plain image as a background, adds walls along the map's
  outer bounds. They go in the same transaction and event as the import,
  and are ordinary walls: they block movement and sight, and can be edited
  and deleted. The import offers **Wall the map's edges**, on by default.
  An edge a file wall already covers is not walled twice. A re-import, or a
  replaced background, does not stack a second perimeter. A preloaded base
  map (US2) takes the same path, so it arrives walled.
- **Layouts stay in the pages.** The width work changes the three pages'
  own markup and classes, not `Container` or anything under
  `components/ui/`. A change there touches every page and, through
  `e2e:which`, the full suite.
- **No runtime feature flags.** Following CONTRIBUTING's rule, a flag
  belongs to a feature that is merged unfinished, or to one an operator has
  a real reason to run without. Each story here is a fix, a layout, or a
  control that a GM already decides on (whether to send a link, which map
  to start on, whether to clear the feed). The operator's switch for base
  maps is the base-maps directory: if it is empty or missing, the picker
  offers only **None** (FR-022).

## User Scenarios & Testing

### User Story 1 - The GM sends a link only an existing account can use, and takes it back (Priority: P1)

The GM opens the players page, makes a link for a player, and copies it.
The player, already signed in to ThunderForge, opens it and joins the
world, and the link counts one join. A link the GM limited to one use is
then used up. The GM revokes a second link they shared by mistake, and the
person who opens it is told it was revoked.

**Why this priority**: the GM shared a link in a group chat during the
session. Anyone holding it could use it five times, forever, and the GM had
to find its controls on a different page.

**Independent Test**: `pnpm e2e:accounts`, which owns `world-links.spec.ts`
(new) and `invite-membership.spec.ts`.

**Depends on**: `hotfix-world-permissions` on main.

**Acceptance Scenarios**:

1. **Given** a GM on the players page, **When** they create a link with the
   defaults, **Then** it is listed with no use limit and an expiry 7 days
   away, and a Copy button puts its URL on the clipboard.
2. **Given** a signed-in account that is not a member, **When** it opens an
   active link and joins, **Then** it is a Player in the world, and the
   GM's list shows one join on the link without a reload. A link limited to
   one use shows as used up.
2a. **Given** a link limited to one use, **When** it is opened, its join
   page viewed and refreshed, and an existing member (the owner included)
   presses Join, **Then** the link still has its one use, and the next new
   member to join spends it.
3. **Given** a revoked link, **When** a signed-in account opens it, **Then**
   it reads "This invite link was revoked by the world's Game Master. Ask
   them for a new one." and no Join button is shown.
4. **Given** an expired link, a used-up link, and a code that was never
   issued, **When** each is opened, **Then** each gets its own message
   (contracts/graphql.md).
5. **Given** a signed-out visitor, **When** they open a link, **Then** they
   are asked to sign in with an existing ThunderForge account, offered no
   way to create one, and shown nothing about the world.
6. **Given** that visitor signs in with an OAuth identity that has no
   account, **When** the callback returns, **Then** no account is created,
   and they read that a world link needs an existing account.
7. **Given** a Player or Trusted Player, **When** they open the players
   page, **Then** no link controls are drawn, and the server refuses
   `generateInviteCode`, `revokeInviteCode` and `worldInvites` from them.
8. **Given** the response for `/join/<code>`, **When** its headers are read,
   **Then** it carries `X-Robots-Tag: noindex, nofollow` and
   `Referrer-Policy: no-referrer`.

---

### User Story 2 - A new world opens on one of our maps, credited (Priority: P1)

The GM creates a world. The form shows the seven base maps as thumbnails,
with Grassy Path Ambush already chosen and a **None** card at the end. The
GM enters the world and finds the map on the board, and a credit line names
MBRound18, CC BY-SA 4.0, the source and the catalog.

**Why this priority**: a new world's first impression was an empty grey
board, and there was no clue what to do next.

**Independent Test**: `pnpm e2e:worlds`, which owns `world-base-map.spec.ts`
(new), and `pnpm e2e:scenes`.

**Acceptance Scenarios**:

1. **Given** the create-world form, **When** it opens, **Then** the seven
   maps and **None** are offered, the default map is selected, and each
   card shows its name and grid size.
2. **Given** a world created with the default, **When** the GM enters it,
   **Then** the Starting Scene's background is that map, at its size and
   grid, with its walls and lights.
3. **Given** a world created with **None**, **When** the GM enters it,
   **Then** the Starting Scene is as it is today: no background, 100 × 100,
   grid 5.
4. **Given** a scene whose background is a base map, **When** anyone views
   the board, the scene list or the picker, **Then** the credit is visible
   there.
5. **Given** the GM replaces that background with their own image, **When**
   the board is viewed, **Then** the credit is gone.
6. **Given** a base map that fails to apply (for example, the bucket is
   down), **When** the world is created, **Then** the world still exists
   with a blank Starting Scene, and the GM is told the map could not be
   added.
7. **Given** a player who joins the world later, **When** they enter,
   **Then** they see the same map.

---

### User Story 3 - The actor view uses the width it has (Priority: P2)

On a phone, the sheet comes first, in one column, with nothing scrolling
sideways. On a laptop, the sheet sits beside the portrait and details. On an
ultrawide, the rolls and inventory take a third column.

**Why this priority**: players spent the session on this page, and on a
wide monitor most of it was empty.

**Independent Test**: `pnpm e2e:actors`, which owns
`actor-view-layout.spec.ts` (new), then `pnpm e2e:combat` and
`pnpm e2e:game-systems`, which also cover `ActorDetailPage`.

**Acceptance Scenarios**:

1. **Given** viewports of 375, 1280 and 2560 px wide, **When** the actor
   view loads, **Then** the page never scrolls sideways, and it shows one,
   two and three columns respectively (contracts/layouts.md).
2. **Given** 2560 px, **When** the page is measured, **Then** its content
   is 1800 px wide and centred.
3. **Given** 375 px, **When** the page loads, **Then** the character sheet
   starts above the fold, after the name and the actions.
4. **Given** any width, **When** a long lore entry is shown, **Then** its
   lines are at most about 75 characters.

---

### User Story 4 - The world page uses the width it has (Priority: P2)

The world page fills a desktop or ultrawide screen with the world's summary,
details, actions and players. On a phone, it shows **Enter world** first.

**Why this priority**: it is the first page of every session, and it sat
in a 1160 px column on every screen.

**Independent Test**: `pnpm e2e:worlds`, which owns
`world-dashboard-layout.spec.ts` (new).

**Acceptance Scenarios**:

1. **Given** viewports of 375, 1280 and 2560 px, **When** the world page
   loads, **Then** it never scrolls sideways, and it uses one, two and three
   columns respectively.
2. **Given** 375 px, **When** it loads, **Then** **Enter world** is the
   first control, at full width, and at least 44 px tall.
3. **Given** a GM, **When** the page loads, **Then** in place of the link
   management that moved to the players page, a Players card shows the
   member count, the number of active links, and a link to the players
   page.

---

### User Story 5 - The GM clears the roll feed for everyone (Priority: P2)

Between scenes, the GM clears the rolls. Every feed at the table empties at
once, the GM's and each player's, and a reload brings nothing back.
Players cannot clear the feed.

**Why this priority**: in the session the GM pressed a control expecting
this, and one player's feed kept the rolls. A clear that only some screens
obey leaves the table looking at different histories.

**Independent Test**: `pnpm e2e:rolls`, which owns `rolls-clear.spec.ts`
(new).

**Acceptance Scenarios**:

1. **Given** a GM and two players who have each rolled, one of them a
   private roll, **When** the GM clears the rolls and confirms, **Then**
   every feed is empty within 2 seconds, with no reload.
2. **Given** a cleared world, **When** any of the three reloads, or drops
   and regains its connection, **Then** no cleared roll comes back: not in
   the feed, not as a single fetch, not in the catch-up.
3. **Given** a cleared world, **When** a player rolls, **Then** only that
   new roll is in every feed.
4. **Given** a player, **When** they look at the feed, **Then** no Clear
   control is drawn, and `clearWorldRolls` from them is refused.
5. **Given** an attack whose to-hit roll was cleared, **When** it is
   resolved or viewed, **Then** it still works. Its roll exists, and is
   only hidden from the feed.
6. **Given** the demo, **When** the GM clears the rolls in one tab,
   **Then** every demo tab's feed empties.

---

### User Story 6 - An imported map is walled at its edges (Priority: P2)

The GM imports the ambush map. Its four edges are walls, so a token dragged
towards the edge stops there, and nobody sees past it. The GM can move or
delete those walls like any other. Importing again does not add a second
set.

**Why this priority**: in the session, tokens walked off the ambush map. A
map with no walls of its own is common, and the edge is the one wall every
map has.

**Independent Test**: `pnpm e2e:canvas`, which owns
`canvas-map-edge-walls.spec.ts` (new), then `pnpm e2e:scenes`. TDD unit
tests for the geometry in `map_import/perimeter_tests.rs`, and for the
demo's mirror in `apps/demo/src/backend/mapImport.test.ts`.

**Acceptance Scenarios**:

1. **Given** the ambush map imported with the defaults, **When** the scene
   loads, **Then** it has exactly four walls, on the map's four edges, each
   blocking movement and sight, and the import reports `wallsCreated: 4`.
2. **Given** that scene, **When** a token is dragged past the edge, **Then**
   the server stops it at the wall, and it stays on the map.
3. **Given** **Wall the map's edges** is unticked, **When** the map is
   imported, **Then** no edge wall is added.
4. **Given** a file whose own walls already run along part of an edge,
   **When** it is imported, **Then** only the uncovered parts of that edge
   are walled, and no two walls overlap on the edge.
5. **Given** a walled scene, **When** the same map, or another one, is
   imported over it, **Then** the scene has one perimeter, on the new map's
   bounds.
6. **Given** a plain image set as a level's background with the box ticked,
   **When** the board loads, **Then** that image's edges are walled.
   Replacing it with another image moves the perimeter, and does not add a
   second one.
7. **Given** a perimeter wall, **When** the GM deletes or moves it, **Then**
   it stays deleted or moved, and nothing puts it back until the next
   import onto that level.
8. **Given** a world created on a base map (US2), or the demo's scenes,
   **When** the board loads, **Then** the map's edges are walled.

---

### User Story 7 - The admin saves the mail settings they changed, and is warned before losing them (Priority: P3)

The administrator edits the host, port and sender, and one Save sends those
three keys and nothing else. Until then, the page says there are three
unsaved changes, Save is the only lit button, and leaving asks first.

**Why this priority**: it is one person's form, used rarely, but the owner
hit it on the day.

**Independent Test**: `pnpm e2e:instance`, which owns
`mail-settings-form.spec.ts` (new), and the web unit tests for the form
model.

**Acceptance Scenarios**:

1. **Given** the mail page as loaded, **When** nothing is changed, **Then**
   Save and Discard are disabled and no unsaved notice is shown.
2. **Given** a change to a field and then a change back to its loaded value,
   **When** the page is checked, **Then** the field is clean again.
3. **Given** three changed fields, **When** the admin saves, **Then**
   exactly three `updateInstanceSetting` calls are sent, one per changed key,
   with `mail.enabled` last if it changed, and the form is clean afterwards.
4. **Given** a password typed into the empty password box, **When** saved,
   **Then** it is sent. An empty password box is never a change. **Clear
   password** is a change that sends `null`.
5. **Given** a key fixed by an environment variable, **When** the form
   loads, **Then** that field cannot be edited and is never sent.
6. **Given** one of three saves is refused, **When** the save ends, **Then**
   the two that saved are clean, the refused one is still dirty with its
   error beside it, and the notice says "2 of 3 saved".
7. **Given** unsaved changes, **When** the admin reloads, closes the tab or
   follows an in-app link, **Then** they are asked to confirm first, and
   staying keeps their edits.

---

### User Story 8 - A player opens their own sheet and look from the players screen, on a phone too (Priority: P3)

`hotfix-player-hero-edit` offers a player their hero's builder and sheet
from the Players screen. This story makes that comfortable on a phone and
puts the player back where they started.

**Why this priority**: the hotfix carries the fix. What is left is polish.

**Independent Test**: `pnpm e2e:actors`, which owns
`players-hero-edit.spec.ts` (from the hotfix) and
`players-hero-mobile.spec.ts` (new), and `pnpm e2e:hero-builder`.

**Depends on**: `hotfix-player-hero-edit` on main.

**Acceptance Scenarios**:

1. **Given** a player with a claimed hero at 375 px, **When** the players
   page loads, **Then** their own card is first, and **Open sheet** and
   **Edit look** are full-width buttons at least 44 px tall.
2. **Given** that player opens **Edit look** at 375 px, **When** the builder
   opens, **Then** it fills the screen and never scrolls sideways, and
   closing it returns to the players page.
3. **Given** a player who opened their sheet from the players page, **When**
   they press Back on the sheet, **Then** they return to the players page,
   not to staging.
4. **Given** a player with no claimed hero, **When** the players page loads,
   **Then** their card says to ask the GM for a character.

### Edge Cases

- **A link opened in the wrong account.** Someone opens a link while
  signed in as their second account. It joins that account, and a limited
  link counts the use. The page names the account before the Join button,
  so they can sign out first. Opening the page alone spends nothing.
- **Old links.** Links made before this spec still have 5 uses and no
  expiry. They keep working, are listed, and can be revoked (Open item 6).
  Old 8- and 20-character codes stay valid. Lookup is not case-sensitive
  for new codes; old codes are uppercase hex, so they are unaffected.
- **A rotated link.** `rotateInviteCode` is unchanged. The panel shows a
  rotated link as revoked, with its replacement as a new row.
- **Brute force.** A code has 128 random bits, so guessing is not a
  practical attack. `joinWorld` keeps the existing auth rate limit.
- **The Referer leak.** A join page loads third-party avatars. Without
  `Referrer-Policy: no-referrer`, the code would travel in the `Referer`
  header.
- **A missing base-maps directory.** The server starts. The picker offers
  only **None**, and `createWorld` with a map id is refused with "That map
  is not available on this instance."
- **Base maps on a rescued world.** The character-rescue flow
  (`collections/rescue.rs:169`) also calls `insert_world_sync`. It gets no
  map: nobody chose one.
- **A world deleted later.** The map's copy is the world's own asset, so it
  goes with the world, like any background (Open item 5).
- **A clear while a roll is in flight.** A roll committed after the clear
  time is shown. One committed before it is hidden, even if its event
  arrives after the clear event.
- **A reveal of a cleared roll.** `revealRoll` on a hidden roll is refused
  with "That roll was cleared." The roll can be neither re-rolled nor
  revealed.
- **A demoted GM.** Clearing is checked per request, so a GM who loses the
  role cannot clear.
- **A perimeter on a non-rectangular map.** The perimeter is the image's
  rectangle. A map drawn as an island in a transparent image is walled at
  the rectangle, not at the shore.
- **A wall that only touches the edge.** A file wall that crosses or touches
  the boundary, without lying along it, covers nothing. Only a collinear
  overlap (within 0.5 scene px) does.
- **A moved perimeter wall.** A perimeter wall the GM has moved off the old
  bounds is no longer treated as perimeter. A re-import leaves it where it
  is, and removes only the marked walls that still lie on the old bounds.
- **A cleared background.** Clearing a background removes no walls. The
  perimeter is the GM's to delete.
- **A resized scene.** A GM who resizes a scene by hand does not get a new
  perimeter (Open item 9).
- **The secret field.** The password is never compared with a loaded
  value, because the client never has one. It is dirty if and only if
  something is typed, or **Clear password** was pressed.
- **The Back button.** In declarative routing, browser Back cannot be
  cancelled before the route changes. Open item 4.
- **A setting changed elsewhere.** If another admin saves while this form
  is open, this form's baseline is stale. Saving sends only this admin's
  changed keys, so the other admin's untouched keys survive. A key changed
  by both is last-write-wins, as it is today.

## Requirements

### Functional Requirements

**US1 World links**

- **FR-001**: The players page MUST hold the world's link panel for anyone
  who runs the world (`Role::runs_the_world`: Owner or GM): create, list,
  copy and revoke. The panel MUST NOT be drawn for anyone else.
  `CampaignSettingsPanel`'s link management MUST move there. The world page
  keeps a summary card (FR-033).
- **FR-002**: Create MUST default to no use limit and a 7-day expiry. A use
  limit, when the GM sets one, MUST be 1 to 50, and the expiry one of 1
  day, 7 days, 30 days or none. `generateInviteCode` MUST treat a missing
  `maxUses` as no limit, MUST refuse a `maxUses` outside 1 to 50, and MUST
  refuse an `expiresAt` that is not RFC 3339, instead of ignoring it.
- **FR-003**: `SessionSetupInviteLink` MUST create links with the same
  defaults as FR-002, and MUST link to the players page for management.
- **FR-004**: The list MUST show, for each link: its joins, and its uses
  left when it has a limit, its expiry,
  who created it, when, and its state. Active links come first; revoked,
  expired and used-up links are folded under **Past links**. The list MUST
  update when a link is used or revoked, through the existing world-event
  trigger on `world_invites`, with no reload.
- **FR-005**: Revoke MUST ask for confirmation, then call `revokeInviteCode`.
  A revoked link MUST NOT admit anyone after the server answers.
- **FR-006**: New codes MUST be 128 bits from the operating system's CSPRNG,
  written as 26 characters of Crockford base32. Lookup MUST upper-case
  the code before comparing. `generate_link_code` changes for instance
  invitations too, since they share it.
- **FR-007**: `worldByInviteCode` and `alreadyMember` MUST require a signed-in
  caller, and MUST return nothing to anyone else.
- **FR-008**: `joinWorld` and the join page MUST tell these cases apart, each
  with its message and error code (contracts/graphql.md): revoked, expired,
  used up, unknown, and already a member. A use MUST be counted only when
  the account becomes a new member, in the same transaction as the
  membership row. Viewing the link or its join page, an existing member
  (the world's owner included), and a join that fails MUST count nothing.
  A limited link whose last use a concurrent join took MUST refuse as used
  up.
- **FR-009**: A signed-out visitor to `/join/<code>` MUST be asked to sign in
  with an existing account. The sign-in page reached from there
  (`returnTo=/join/...`) MUST NOT offer registration. It MUST NOT show the
  world's name either.
- **FR-010**: An OAuth sign-in started from a join page MUST be sign-in
  only. If the identity has no account, the callback MUST NOT create one,
  and MUST show "World links are for existing ThunderForge accounts. Ask the
  instance's administrator for an invitation." This holds whatever the
  instance's access mode.
- **FR-011**: The server MUST send `X-Robots-Tag: noindex, nofollow` and
  `Referrer-Policy: no-referrer` on `/join/*` and `/invite/*`. The pages keep
  their `noindex` meta. `robots.txt` MUST NOT disallow these paths, so that
  a crawler can read the header.
- **FR-012**: A migration MUST make `world_invites.max_uses` nullable, with
  `NULL` meaning no limit and as the default, and replace its checks with
  `CHECK (max_uses IS NULL OR max_uses > 0)` and
  `CHECK (max_uses IS NULL OR used_count <= max_uses)`. `used_count` keeps
  counting joins on a link with no limit. Today's default of 0 is one the
  `CHECK (max_uses > 0)` refuses, and the join predicate's "0 means
  unlimited" branch can never match.

**US2 Base maps**

- **FR-020**: The server MUST read the base maps from a directory given by
  `--base-maps-dir` / `THUNDERFORGE_BASE_MAPS_DIR` (clap with an environment
  fallback), laid out as contracts/base-maps.md describes. The server image
  MUST carry them, built in their own Dockerfile stage by
  `thunderforge-demo-maps`.
- **FR-021**: A `baseMaps` query MUST list the available maps for a signed-in
  caller: id, name, width, height, grid, a thumbnail URL and the credit.
  Thumbnails and images MUST be served from `/api/base-maps/<id>.*` with a
  long cache lifetime.
- **FR-022**: If the directory is missing, empty or unreadable, the server
  MUST start, log one line saying so, and offer an empty list.
- **FR-023**: `createWorld` MUST take an optional `baseMapId`
  (`MaybeUndefined`):
  - left out: the default map (Open item 1), if it is available;
  - `null`: no map;
  - an id: that map.

  An unknown id MUST be refused before the world is created.
- **FR-024**: After the world's transaction commits, the server MUST import
  the chosen map into the Starting Scene as the map import does: a
  BACKGROUND asset in the world's own storage path, the scene's width,
  height and grid, then walls, doors and lights, with the edge walls of
  FR-094. The import MUST record its
  world events (`MAP_IMPORTED`, 13), so the circular flow reaches every
  client.
- **FR-025**: If the import fails, the world MUST stay, with a blank
  Starting Scene. `createWorld` MUST still return the world, alongside a
  non-fatal GraphQL error with `extensions.code = STARTING_MAP_FAILED`
  (contracts/graphql.md), which the form shows as a notice. The return
  type does not change.
- **FR-026**: The rescue flow MUST NOT apply a map.
- **FR-027**: The create-world form MUST offer the maps as a radio group of
  thumbnail cards, plus **None**, with the default selected. It MUST be
  usable by keyboard, and at 375 px.

**US2 Attribution** (a condition of the licence)

- **FR-028**: The credit MUST be read from a single source,
  `examples/maps/credit.json`, moved from `apps/demo/credit.json`. The demo
  reads it from its new place. The credit names:
  - the author, MBRound18;
  - the licence, CC BY-SA 4.0, with its URL;
  - the source, <https://github.com/mbround18/vtt-maps>;
  - the catalog, <https://vtt-maps.dnd-apps.dev/catalog>.
- **FR-029**: A background asset made from a base map MUST record the map's
  id (`canvas_image_assets.base_map_id`). `Scene.backgroundCredit` MUST
  resolve from it, and MUST be null for any other background.
- **FR-030**: The credit MUST be visible wherever a base map is shown:
  - each picker card, or once beside the picker;
  - the board, as a small credit line in the React chrome while the scene's
    background has a credit;
  - the scene list and the scene's settings.

  Each place MUST link the licence, the source and the catalog. ShareAlike
  MUST be stated: "The copies here are offered under the same licence."
- **FR-031**: The `NOTICE.txt` that `thunderforge-demo-maps` writes MUST ship
  in the base-maps directory.
- **FR-032**: The stale doc comment on `STARTER_SCENE_NAME` MUST be
  replaced with the licence as settled.

**US3 and US4 Layout**

- **FR-033**: The world page MUST follow contracts/layouts.md, with a
  Players card in place of the moved link management.
- **FR-034**: The actor view MUST follow contracts/layouts.md.
- **FR-035**: Neither page may scroll sideways at any width from 320 px to
  3840 px.
- **FR-036**: The changes MUST stay in the pages' own files. `Container`,
  `components/ui/**`, `styles/**` and `routes/**` MUST NOT change for US3
  or US4.

**US5 Clear rolls**

- **FR-040**: A `clearWorldRolls(worldId)` mutation MUST be allowed only to
  those who run the world (`runs_the_world`) and site admins, checked per
  request. It MUST set `worlds.rolls_cleared_at` to the transaction's time
  and record a `ROLLS_CLEARED` world event, code **39**, whose payload is
  `{clearedAt}`. It MUST be refused while play is paused, as other GM writes
  are.
- **FR-041**: It MUST NOT delete or change any row of `world_roll_records`
  or `world_attacks`.
- **FR-042**: `rolls/visibility.rs` MUST gain the clear rule, and every
  existing path to a roll MUST go through it: `worldRolls`, `worldRoll`,
  `worldRollRecords`, the live subscription and the catch-up. A roll whose
  `created_at` is at or before the world's `rolls_cleared_at` MUST be
  withheld from all of them. In the catch-up and the live stream, a
  `ROLL_MADE` or `ROLL_REVEALED` event recorded at or before the clear MUST
  NOT be delivered.
- **FR-043**: `revealRoll` and re-rolls MUST refuse a withheld roll, with
  "That roll was cleared."
- **FR-044**: `apps/web/src/engine/world/sync/rolls.ts` MUST handle event 39.
  `useWorldRolls` MUST drop every entry created at or before `clearedAt`,
  live, on every client, and MUST NOT animate any of them again.
- **FR-045**: The feed's header MUST hold **Clear rolls** for the GM, with a
  confirmation ("Clear every roll from the feed for everyone? The rolls are
  kept in the world's record."). It MUST NOT be drawn for players.
- **FR-046**: The demo MUST mirror FR-040 to FR-044 in
  `apps/demo/src/backend/handlers/dice.ts` and `events.ts`, across its tabs.

**US6 Edge walls**

- **FR-090**: `map_import/perimeter.rs` (new) MUST hold a pure function,
  `perimeter_walls(placement, existing: &[WallInsert]) -> Vec<WallInsert>`.
  It returns walls along the four edges of the placement's rectangle
  (`±width/2`, `±height/2`, centred, y up), minus every interval that an
  existing wall covers by collinear overlap within 0.5 scene px. An edge
  that is fully covered yields no wall, and a partly covered one yields
  one wall per uncovered interval. Each wall blocks movement and sight, has
  `door_state = "none"`, and is marked `metadata.perimeter = true`. It MUST
  be written test first.
- **FR-091**: The UVTT import (`import_uvtt_impl`) MUST take `wallEdges`
  (default `true`). When it is set, the import MUST:
  - delete the level's walls that are marked perimeter *and* still lie on
    the previous bounds;
  - add `perimeter_walls` over the file's own walls;
  - do both in the import's existing transaction, under its one
    `MAP_IMPORTED` event.

  `wallsCreated` counts the perimeter walls. A new `perimeterWallsCreated`
  field reports them on their own.
- **FR-092**: `updateSceneLevel`'s input MUST take `wallEdges: Boolean`
  (default `true`). When it sets a new `backgroundAssetId`, and `wallEdges`
  is true, it MUST replace the perimeter as FR-091 describes. This happens
  in the level's transaction, and records the wall events that the wall
  tool's own creation records (`WALL_CHANGED`, 10). Every other
  `updateSceneLevel` call MUST leave walls alone.
- **FR-093**: The map import tool (`MapImportTool.tsx`), and the background
  picker that sets a plain image, MUST show **Wall the map's edges**,
  ticked by default.
- **FR-094**: `import_offline` MUST add the perimeter, so that the demo's
  seed, the landing atlas and the base maps (US2, FR-024) carry it.
  `maps.json` MUST mark those walls `perimeter: true`.
- **FR-095**: The demo's `mapImport.ts` MUST mirror FR-090 and FR-091, with
  its own tests against the same fixtures.
- **FR-096**: The existing UVTT geometry tests for walls, doors and lights
  MUST be unchanged. The perimeter adds walls, and moves none.

**US7 Mail settings form**

- **FR-050**: The mail page MUST be one form, with one Save and one Discard,
  over a pure form model (`settingsForm.ts`, data-model.md). The model holds
  the loaded baseline and a change per key: `set(value)` or `clear`.
- **FR-051**: A key MUST be dirty only when its draft differs from its
  baseline after trimming, as the server compares it. A secret is dirty
  only when typed into, or cleared. A key fixed by the environment MUST NOT
  be editable or dirty.
- **FR-052**: Save MUST be enabled only while at least one key is dirty and
  every dirty key passes client-side validation. It MUST send one
  `updateInstanceSetting` per dirty key, and nothing for clean keys. Keys
  are sent in the order they appear on the form, with `mail.enabled` last.
- **FR-053**: Each saved key MUST become its new baseline. A refused key
  MUST stay dirty with its error. The page MUST say how many of how many
  were saved. After a save, the page MUST reload the settings and rebase
  the model, keeping only the keys that are still dirty.
- **FR-054**: While dirty, the page MUST show "N unsaved changes", and MUST
  ask before leaving. A `useUnsavedChanges(dirty)` hook in
  `apps/web/src/hooks/` handles this:
  - `beforeunload`, for a reload or a closed tab;
  - a confirm dialog for in-app link clicks and the settings page's own
    section switches.
- **FR-055**: `SettingRow`'s draft MUST follow its `setting` prop when the
  row is clean, so that a reload never shows a stale draft. The other
  settings panels keep their per-row Save. Moving them to the form model is
  later work.

**US8 Hero polish**

- **FR-060**: At under 640 px, the players page MUST show the viewer's own
  card first. Its **Open sheet** and **Edit look** controls (from
  `hotfix-player-hero-edit`) MUST be full width and at least 44 px tall.
- **FR-061**: At under 640 px, the hero builder dialog MUST open full screen,
  with no sideways scroll. Closing it MUST return to where it was opened.
- **FR-062**: The actor view MUST honour `?from=players`, sending its back
  control to the players page. Without it, the back control is unchanged.
- **FR-063**: A player with no claimed character MUST see "Ask your GM for a
  character" on their card.

**Telemetry** (spec 086, contracts/telemetry.md)

- **FR-070**: The browser events in contracts/telemetry.md MUST be added to
  `packages/telemetry`'s allow-list, with bounded attributes only: no code,
  id, name, address or setting value. If spec 086 has not landed when a
  story is built, its event task waits, and the wait is recorded in
  tasks.md.
- **FR-071**: The server counters in contracts/telemetry.md MUST go through
  `crates/thunderforge-server`'s telemetry API from spec 086. The GraphQL
  root-field spans of 086 FR-011 already cover each new mutation.

**Docs**

- **FR-080**: These guides MUST be written or extended:
  - `docs/guides/inviting-players.md` (new): world links, the optional use
    limit and when a use counts, revoking, and why a new person needs an
    instance invitation;
  - `docs/guides/your-first-world.md` (new): the starting map, **None**, and
    the credit;
  - `docs/guides/rolls.md`: clearing rolls, and what clearing keeps;
  - `docs/guides/doors-and-walls.md`: the edge walls an import adds, the
    **Wall the map's edges** box, and how to remove them.
- **FR-081**: `docs/INSTANCE_CONFIGURATION.md` and `.env.example` MUST
  document `THUNDERFORGE_BASE_MAPS_DIR`. `docs/CONTRIBUTING.md` MUST
  describe `useUnsavedChanges` and the settings form model.

### Key Entities

- **World link**: a `world_invites` row. It gains no column; `max_uses`
  becomes nullable (no limit), and its code gets a new generator.
- **Base map**: an entry in the base-maps directory's `maps.json`. It is
  served, not stored in the database.
- **Map credit**: the one credit in `examples/maps/credit.json`. A
  background asset made from a base map points to it through
  `base_map_id`.
- **Roll clear**: `worlds.rolls_cleared_at`, and the `ROLLS_CLEARED` world
  event (39).
- **Perimeter wall**: an ordinary `walls` row, marked
  `metadata.perimeter = true`. The mark is how a re-import finds it.
- **Settings form model**: client-only. A baseline per key, plus a change
  per dirty key.

## Success Criteria

### Measurable Outcomes

- **SC-001**: A link limited to one use admits exactly one account, however
  often it is opened. A second account opening it after the join reads the
  used-up message. A default link admits every existing account that joins
  until it expires or is revoked. A revoked link admits nobody, and
  its message names revocation.
- **SC-002**: No path through a world link creates an account: not local
  registration, and not OAuth, in `open`, `invite_only` or `closed` mode.
- **SC-003**: A newly created world shows its map on the board, with its
  credit, with no step after Create. A world created with **None** matches
  today's Starting Scene field for field.
- **SC-004**: At 375, 1280 and 2560 px, the actor view and the world page
  have `scrollWidth <= innerWidth`, and show 1, 2 and 3 columns. At 2560 px,
  their content is 1800 px wide, ±1 px.
- **SC-005**: After a clear, every connected feed is empty within 2 s.
  After a reload or reconnect, 0 cleared rolls come back through any path.
  The `world_roll_records` row count is unchanged.
- **SC-006**: A save after changing k of n mail keys sends exactly k
  `updateInstanceSetting` calls (counted in the e2e by request
  interception). A save with nothing changed is impossible.
- **SC-007**: Every Proof slice passes in one run, with
  `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and `--workers=1` where the
  external stack is used.
- **SC-008**: An imported ambush map has 4 walls, one on each edge. A
  re-import leaves 4. A token dragged 10 cells past an edge ends inside the
  map.

### Proof

On the `088-first-session-feedback` worktree, each story proves itself with
its own slice as it lands. Then, after the final phase:

- `make lint`;
- `cargo test -p thunderforge-server`, with the RustFS bucket present;
- `pnpm -F @thunderforge/web test` and `typecheck`;
- the demo's unit tests;
- the slices:
  - `pnpm e2e:accounts` (US1);
  - `pnpm e2e:worlds` and `pnpm e2e:scenes` (US2, US4);
  - `pnpm e2e:canvas`, for the board's credit line, the map import path and
    the edge walls (US6);
  - `pnpm e2e:actors`, `pnpm e2e:combat` and `pnpm e2e:game-systems` (US3,
    US8);
  - `pnpm e2e:hero-builder` (US8);
  - `pnpm e2e:rolls`, both parts, the demo included (US5);
  - the demo's unit tests for `mapImport.ts` (US6);
  - `pnpm e2e:instance` (US7);
- `pnpm e2e:which --diff`, with each slice it names run. If it names the
  full suite (`schema.graphql` and `auth/**` are cross-cutting), the slices
  above stand in for it (Open item 7).

The full suite (`e2e-parallel`) is not run.

## Assumptions

- The four hotfix branches land on main before the stories that depend on
  them start. A hotfix that changes the code a story names is rebased onto,
  not worked around.
- Spec 086 may or may not have landed. The telemetry tasks are written
  against its contract and wait for it if needed.
- The seven maps are the shipped set. A new map added to `examples/maps`
  joins the picker with no code change.
- The base-map images, at about 1 to 3 MB each as WebP, fit in the server
  image.
- Chromium only, as everywhere in the app.

## Open items

These are the owner's decisions. Each has a default that the work follows
unless the owner says otherwise.

1. **The default starting map.** Default: **Grassy Path Ambush**. It is open
   ground with no walls or lights, and `examples/maps/README.md` already
   calls it the default blank scene. The alternative is **The Proving
   Ground**, the demo's showpiece, with 33 walls and 12 lights.
2. **The default choice at world creation.** Default: the GM sees the picker
   with the default map selected. The alternatives are no picker at all, or
   **None** selected.
3. **Saving mail settings atomically.** Default: one `updateInstanceSetting`
   per changed key, in order, with `mail.enabled` last. This keeps the
   server's one-key design. The alternative is a new transactional
   `updateInstanceSettings` mutation, so that a host and a port are never
   saved apart.
4. **The Back button.** Default: no router change. The guard covers reload,
   closing the tab, in-app links and section switches, but not browser Back.
   Covering Back means moving to a data router (`createBrowserRouter`, for
   `useBlocker`). That touches `routes/**` and `main.tsx`, which is a
   cross-cutting change with its own spec.
5. **One copy of a base map per world.** Default: each world gets its own
   copy, about 1 to 3 MB, so deleting a world or replacing its background
   behaves as for any image. The alternative is one shared, de-duplicated
   asset per map, which saves storage but needs reference counting in
   deletion.
6. **Links made before this spec.** Default: they keep working with their 5
   uses and no expiry, are listed, and can be revoked. The alternative is a
   migration that expires every link without an expiry, which would break
   links already shared on vtt-dev.
7. **Slices in place of the full suite.** Default: the Proof slices stand in
   for it, as for every other change, even though `schema.graphql` and
   `auth/**` are cross-cutting paths.
8. **The ultrawide cap.** Default: 1800 px, the same as `WorldSectionShell`.
   The alternative is full width, which gives very long lines at 3840 px.
9. **Edge walls on a resize.** Default: no. The perimeter is added only at
   import, and when a background is set. A GM who resizes a scene by hand
   keeps the walls they have. The alternative is to move the perimeter on
   a resize too.

## Later

- Moving the other instance settings panels to the form model of US7.
- A data router, which would let every form guard the Back button too.
- Letting a GM pick a base map for a new scene, not only the first.
- A "restore cleared rolls" control. The rows are kept, so it is possible.
