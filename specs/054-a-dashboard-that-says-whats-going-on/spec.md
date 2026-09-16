# Feature Specification: A Dashboard That Says What Is Going On

**Feature Branch**: `054-a-dashboard-that-says-whats-going-on`

**Created**: 2026-09-15

**Status**: Draft

**Input**: Project owner, walking the app on 2026-09-15: `/counter` is a
component preview page — "Dashboard preview", a counter demo, avatar swatches —
sitting in the navigation. Replace it with a real dashboard for the person
signed in: their worlds and when each was last played, invitations waiting,
characters they hold, offers and requests needing an answer, and anything paused
or under moderation that affects them. The component gallery moves to a
developer-only route and leaves the navigation.

## The problem

### The page in the navigation is a showcase

`/counter` (`apps/web/src/pages/counter/CounterPage.tsx`) is titled "Dashboard
preview" (`:24-36`) and says what it is in its own header: "This page acts as
the UI showcase: shadcn primitives, themed panels, Dicebear identity previews"
(`:161-171`). What it renders:

- a hero with links to `/world/demo-world/play` — a world that does not exist
  (`:172-213`);
- a **counter demo**, two buttons and a number, described in the page as "a
  regression harness" (`:221-258`);
- an avatar panel that generates Dicebear swatches and exports them (`:260-310`);
- tabs whose three panels hold placeholder prose (`:47-103`, `:388-390`);
- three static cards of prose about worlds, scenes and permissions
  (`:392-403`), and a dialog of five hardcoded to-do strings (`:405-433`).

It is behind `RequireAuthenticated` (`apps/web/src/routes/AppRoutes.tsx:587-594`)
and it is in the navigation: as **Status** while setup is required
(`AppRoutes.tsx:218`) and as **Preview** for every signed-in person and every
administrator (`:227-232`, `:247-253`). The header's user menu sends a
non-administrator there (`apps/web/src/components/navigation/AppHeader.tsx:116`),
and a sign-out returns there by default (`apps/web/src/api/auth.ts:243`).

### One real thing lives on it

The page holds the **only** user-facing path to export-my-data and
delete-my-data (`CounterPage.tsx:107-155`, controls at `:312-386`) — the
contracts of ADR-004, ADR-005, ADR-011 and ADR-012. Nothing else in the app
calls `exportUserData` or `deleteUserData`. Deleting this page without moving
those controls would remove a person's only way to exercise those rights.

### And there is nowhere that answers "what needs me"

`/welcome` (`apps/web/src/pages/user/WelcomePage.tsx`) is the closest thing.
It reads one query — `getMyWorldsWithRole()` (`:44-70`) — and shows world
cards sorted Game-Master-first (`:76-80`, `:110-149`), a create-a-world link, an
invite-code box and two account links (`:156-238`). It says nothing about
anything waiting for the person reading it. A person with three worlds, a
pending damage offer, a character somebody wants them to approve and a paused
table learns none of that until they open each world in turn.

**A new account never sees it at all**: zero worlds redirects straight to
`/worlds/create` (`WelcomePage.tsx:96-98`), and a failure to load worlds is
treated the same way, so a transient error also sends somebody to a creation
form they did not ask for.

## What a dashboard can read today, and what it cannot

Most of the panels the owner named already have a source. One does not, and one
has a source under a name nobody would look under. This spec says so rather
than assuming.

| Panel | Source today |
|-------|--------------|
| Worlds and my role in each | `myWorldsWithRole` (`apps/web/src/api/world.ts:69-80`; `src/server/src/graphql/queries/user.rs:166`) |
| Characters I hold | `myActorClaim(worldId)` (`apps/web/src/api/actorClaims.ts:50-62`; `src/server/src/graphql/mutations_actor_claims.rs:241`) — **per world**, so a dashboard asks once per world or needs one query across them |
| Offers needing an answer | `pendingOffers(worldId)` (`apps/web/src/api/attacks.ts:159-168`; spec 046 FR-008) — **per world**, same shape |
| A world's play paused | `worldPlayState(worldId)` (`apps/web/src/api/playPause.ts:52-65`; `src/server/src/graphql/queries/play_pause.rs:459`) — **per world**; no "which of my worlds are paused" |
| My content under moderation | `myStanding` and `myNotices` (`apps/web/src/api/standing.ts:104-122`; `src/server/src/graphql/queries/standing.rs:154-179`), and the per-item banner `ModeratedContentBanner` |
| Recent activity in my worlds | `myWorldEvents` (`src/server/src/graphql/queries/user.rs:206`) — **owned worlds only** |
| **Invitations waiting for me** | **Nothing.** See below |
| **When a world was last played** | `world_live_play.last_beat_at` — a stored per-world timestamp, already written whenever a session runs. See below |

**There are no invitations addressed to a person.** A world invite is an
anonymous *code* with `max_uses` and `used_count` and no invitee
(`src/server/src/schema.rs`, `world_invites`); it is read by
`worldInvites(worldId)`, which only an Owner or Game Master may call
(`src/server/src/graphql/queries/invite.rs:311`), and redeemed by whoever holds
the link. So "invitations waiting" is not a panel that can be populated — it is
a feature that does not exist. FR-035 drops the panel and FR-037 sends targeted
invitations to a spec of their own.

**The last-played timestamp already exists, under another name.** It is true
that the `worlds` table has `created_at` and `updated_at` and no
`last_played_at`, and that `world_members` has `joined_at` and no last-visit
column. But spec 051 did not only keep a live/not-live flag: it keeps a
**durable row per world holding the time of the last beat** —
`world_live_play (world_id, last_beat_at)`
(`src/server/src/schema.rs:1394-1398`), written by `mark_live`
(`src/server/src/play_pause/live_play.rs:71-91`) from the heartbeat mutation
(`src/server/src/graphql/mutations_heartbeat.rs:108`).

That row is exactly a last-played record, and it has the four properties the
dashboard needs:

- **It is written when a session actually runs**, not when a world is edited.
  The heartbeat comes from a client present in a world, and only from a member:
  membership is checked on every beat
  (`mutations_heartbeat.rs:85-86`).
- **It is durable and never pruned.** `mark_live` upserts one row per world
  (`live_play.rs:79-87`); nothing deletes it. The in-memory map it throttles
  against is pruned (`live_play.rs:60-62`), the row is not. So a world played
  once in March still carries March.
- **Absence means never played.** A world with no row has never had a beat, and
  the module's own test asserts it in those words
  (`a_beat_marks_the_world_and_the_next_is_throttled`, `live_play.rs:126-146`),
  which is precisely what FR-015 needs.
- **It is accurate to thirty seconds**, because the write is throttled in the
  process and conditional in the database (`live_play.rs:9-20`). Thirty seconds
  is far finer than "two hours ago".

So FR-012 is a requirement to *read and display* a fact the product already
records, under the conditions FR-012a to FR-012e set, rather than a new
column. `live_among` (`live_play.rs:99-110`) already answers the same table for
a set of world ids in one query, which is the shape a dashboard wants.

One thing the row is not: it is not per person. It says when *the table* last
played, not when *this member* last sat at it. FR-012 asks for the former,
which is the fact a table would recognise.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The first screen says what needs me (Priority: P1)

Somebody signs in and lands on their dashboard. Above everything else is what is
waiting for them: an offer to accept or decline, a world whose play an operator
has paused, content of theirs that has been disabled. Below that, their worlds,
each with what they play there and when it was last played.

**Why this priority**: It is the point. Everything else on the page is a list;
this is the part that changes what somebody does next.

**Independent Test**: A player with one pending damage offer signs in. The offer
is the first thing on the dashboard, naming the world and what it is, and
answering it from the dashboard removes it.

**Acceptance Scenarios**:

1. **Given** a signed-in person with something waiting, **When** they open the
   dashboard, **Then** those items appear first, each naming the world it
   belongs to and what it asks of them.
2. **Given** an item that can be answered, **When** they answer it from the
   dashboard, **Then** it is answered exactly as it would be in the world, and
   it leaves the dashboard.
3. **Given** a person with nothing waiting, **Then** the dashboard says so
   plainly rather than showing an empty region.
4. **Given** an item that arrives while the dashboard is open, **Then** it
   appears without the page being reloaded.

---

### User Story 2 - My worlds, and when each was last played (Priority: P1)

The dashboard lists the worlds the person is in, with the role they hold in
each, and when each was last played — ordered by that, so the table they are
actually running is at the top.

**Why this priority**: It is the list people come back for, and ordering it by
recency is what makes the page worth opening rather than bookmarking one world.

**Independent Test**: A person in three worlds plays one. It moves to the top of
their dashboard with a last-played time that matches the session, and the other
two follow in their own order.

**Acceptance Scenarios**:

1. **Given** a person in several worlds, **Then** each is listed with the role
   they hold there.
2. **Given** a world that has been played, **Then** the list shows when, in
   words a person reads rather than a timestamp.
3. **Given** a world nobody has ever played, **Then** it says so rather than
   showing a blank or the world's creation date dressed as a session.
4. **Given** a world whose play an operator has paused, **Then** the card says
   play is paused and when, and says nothing about why (spec 051 FR-050).
5. **Given** the list, **Then** it is ordered by when each world was last
   played, most recent first.

---

### User Story 3 - A Game Master's dashboard is not a player's (Priority: P2)

What each person sees is what that person may see. A Game Master's dashboard
shows what their table needs from them; a player's shows what is theirs. Neither
shows the other's business.

**Why this priority**: A dashboard is a place where facts from several worlds
meet, which is exactly where a leak happens. It is second because it constrains
the panels rather than adding one.

**Independent Test**: A Game Master and a player in the same world open their
dashboards side by side. Each sees their own characters and their own waiting
items; neither sees anything the world would not show them.

**Acceptance Scenarios**:

1. **Given** a player, **Then** their dashboard shows characters they hold and
   offers addressed to them, and nothing about other players' characters or
   offers.
2. **Given** a Game Master, **Then** their dashboard shows what their table
   needs from them, within what their role in that world already allows.
3. **Given** a person who is a site operator, **Then** their dashboard is their
   own, as a person — operator work stays in the admin portal.
4. **Given** anything a world hides from a person, **Then** the dashboard
   hides it too. A dashboard never widens what somebody may see.

---

### User Story 4 - Content of mine that is under moderation (Priority: P2)

If something a person made has been disabled, or their account's standing has
changed, the dashboard says so once, plainly, with a way to read the detail.

**Why this priority**: Today a person finds out by opening the thing and meeting
a banner. Being told once, where they start, is the difference between a process
and an ambush.

**Independent Test**: Disable a person's lore entry through the takedown flow.
Their dashboard says something of theirs has been disabled and links to where
they can read about it and dispute it.

**Acceptance Scenarios**:

1. **Given** content of a person's that has been disabled, **Then** their
   dashboard says so and links to the detail and the dispute path.
2. **Given** an account whose standing has changed, **Then** the dashboard says
   so in the words spec 039's standing surfaces already use, and does not
   restate them differently.
3. **Given** an account in good standing with nothing disabled, **Then** the
   dashboard says nothing about moderation at all.

---

### User Story 5 - A new account is told what to do next (Priority: P2)

Somebody who has just made an account opens the dashboard and is told, in one
screen, the two things they can do: make a world, or join one with a code.
Nothing is empty, and nothing redirects them somewhere they did not choose.

**Why this priority**: It is the first impression of the product, and today it
is a redirect to a form.

**Independent Test**: Sign in as a brand-new account. The dashboard explains the
two routes and offers both, and no redirect happens.

**Acceptance Scenarios**:

1. **Given** an account in no worlds, **When** they open the dashboard, **Then**
   they see it — they are not redirected away from it.
2. **Given** that account, **Then** the page explains creating a world and
   joining one with a code, and offers both.
3. **Given** an account whose worlds failed to load, **Then** they are told the
   list could not be loaded and offered a retry — not shown the new-account
   text, which would read as "your worlds are gone".

---

### User Story 6 - The gallery becomes a developer's page (Priority: P3)

The component showcase stops being part of the product: it leaves the
navigation, leaves the user menu and leaves the sign-out destination, and lives
at a route meant for the people building the app. Nothing it uniquely offered is
lost.

**Why this priority**: It is a removal, and removals come after the thing that
replaces them. It is listed as its own story because of what is buried on that
page.

**Independent Test**: No navigation, menu or redirect points at the gallery, and
export-my-data and delete-my-data are reachable from the person's account
settings.

**Acceptance Scenarios**:

1. **Given** any signed-in person, **Then** no navigation item, user-menu entry
   or post-sign-out redirect takes them to the component gallery.
2. **Given** the account controls that live on the gallery today — export my
   data and delete my account — **Then** they are reachable from account
   settings, and behave as they do now.
3. **Given** somebody following an old link to the gallery, **Then** they reach
   the dashboard rather than an error.
4. **Given** a developer, **Then** the gallery is still reachable by its own
   route, and is not in anybody's way.

---

### Edge Cases

- **A person in fifty worlds.** The list is bounded and ordered by recency, with
  a way to reach the rest. The dashboard is not the worlds page.
- **A world with no scenes, or no active scene.** It still lists; it says it has
  not been played.
- **An offer in a world whose play is paused.** The world is paused, so the
  offer cannot be answered. The dashboard says the world is paused and does not
  offer a control that would be refused.
- **A person whose account is disabled** (spec 039 US7). They are already sent
  to their standing page by `RequireAuthenticated`; the dashboard is not a way
  around that.
- **Setup is still required.** The navigation's "Status" item during setup
  points at the gallery today. Setup has its own screens; the dashboard is for
  a signed-in person with worlds, and the setup navigation must point at
  something that is about setup.
- **A panel's source fails.** One panel failing must not take the page with it:
  the panel says it could not load, the rest of the dashboard renders.
- **Two browsers open.** Answering something in one removes it from the other
  without a reload, the same way the world's own panels already follow events.
- **A world deleted while the dashboard is open.** It leaves the list; nothing
  errors.

## Requirements *(mandatory)*

### Functional Requirements

**What the dashboard is**

- **FR-001**: The route a signed-in person lands on MUST show their own worlds
  and what needs their attention. It MUST NOT be a component showcase.
- **FR-002**: Every panel MUST be about the person reading it. The dashboard
  MUST NOT show anything that person could not already see in the world it
  came from.
- **FR-003**: A panel whose source fails MUST fail alone, saying so, without
  preventing the rest of the page from rendering.
- **FR-004**: Anything the dashboard can answer MUST arrive and disappear
  without a reload, following the same events the world's own panels follow.

**What needs me**

- **FR-010**: The dashboard MUST show items awaiting the person's answer, each
  naming its world and what it asks. At minimum: damage and healing offers
  addressed to them (spec 046 FR-008), and any request for a decision that a
  later spec addresses to a person.
- **FR-011**: An item that can be answered MUST be answerable from the
  dashboard, with the same effect and the same refusals as answering it in the
  world. An item that cannot be answered — because the world's play is paused,
  for example — MUST be shown without a control that would be refused.

**My worlds**

- **FR-012**: Each world MUST show when it was last played. **"Played" means
  any live session in that world, by anybody** — one member present and beating
  is a session, so a Game Master who opened a scene alone to prepare it counts.
  It is one fact per world, not one per member, and it is the fact a table
  would recognise when asked "when did we last play?". It is never an edit to
  the world's data.
  - **FR-012a**: The product MUST read this from the record it already keeps —
    `world_live_play.last_beat_at`, the durable per-world mark spec 051's
    heartbeat writes (`src/server/src/schema.rs:1394-1398`,
    `src/server/src/play_pause/live_play.rs:71-91`). It MUST NOT introduce a
    second timestamp for the same fact: two records of when a world was last
    played will disagree, and the dashboard would be the surface where that
    shows.
  - **FR-012b**: That record MUST be readable for a set of worlds in one
    query, as `live_among` already reads the same table for liveness
    (`live_play.rs:99-110`). A dashboard MUST NOT ask once per world for it.
  - **FR-012c**: The last-played time MUST be shown to **every** member of the
    world, not only to those who were present for that session. It is a fact
    about the table.
  - **FR-012d**: A world whose play is paused MUST show the last-played time it
    had when the pause began and MUST NOT present it as freshness. A paused
    world's beats are refused (`mutations_heartbeat.rs:89`), so its mark stops
    advancing; the card says play is paused (FR-016), and the two together must
    not read as "this table went quiet on its own".
  - **FR-012e**: The last-played time MUST be accurate to within a minute of
    the session it describes. The existing mark is written at most once per
    thirty seconds per world (`live_play.rs:9-20`), which satisfies this; no
    finer accuracy is required, and nothing may be added to obtain one.
- **FR-013**: The world list MUST be ordered by when each world was last
  played, most recent first, with worlds never played after them.
- **FR-014**: Each world MUST show the role the person holds in it.
- **FR-015**: A world never played MUST say so, and MUST NOT show its creation
  or modification date in place of a session.
- **FR-016**: A world whose play an operator has paused MUST say that play is
  paused and when, and MUST NOT state or hint at why (spec 051 FR-050).
- **FR-017**: The list MUST be bounded and MUST offer a way to the full list of
  worlds.
- **FR-018**: Last-played MUST be a property of a world, available to **every**
  surface that lists worlds — the dashboard, the full worlds list, and the
  world's own staging page — and read from the one record FR-012a names. It
  does not deserve a home of its own: `world_live_play` already is that home,
  and the gap is that nothing outside `play_pause` reads it. The fix is to
  carry it on the world as `myWorldsWithRole` already carries the role
  (`src/server/src/graphql/queries/user.rs:166`), so that a second surface
  wanting it needs no second query and cannot show a different answer.
- **FR-019**: The full worlds list MUST offer the same ordering the dashboard
  uses — most recently played first — and MUST say, for a world never played,
  that it has not been played, on the same terms as FR-015.

**Characters I hold**

- **FR-020**: The dashboard MUST show the characters the person holds, each
  naming its world.
- **FR-021**: It MUST show only their own; another player's character in the
  same world MUST NOT appear.

**Moderation and standing**

- **FR-030**: Where content of the person's has been disabled, the dashboard
  MUST say so once and link to the detail and the dispute path.
- **FR-031**: Where the account's standing has changed, the dashboard MUST say
  so in the same words spec 039's standing surfaces use.
- **FR-032**: An account in good standing with nothing disabled MUST be shown
  nothing about moderation.

**Invitations**

- **FR-035**: This feature MUST NOT carry an "invitations waiting" panel, and
  the dashboard MUST NOT imply that one is coming. There is nothing behind it:
  a world invite is an anonymous code with `max_uses` and `used_count` and no
  invitee (`world_invites`, `src/server/src/schema.rs`), readable only by the
  world's Owner or Game Master
  (`src/server/src/graphql/queries/invite.rs:311`), and redeemed by whoever
  holds the link. A panel listing invitations addressed to a person can only
  list nothing.
- **FR-036**: The route into a world by code MUST stay on the dashboard for a
  person who has been handed one — the invite-code box `/welcome` has today
  (`WelcomePage.tsx:156-238`) — because that is the whole of what the product
  can offer a person holding an invitation.
- **FR-037**: Targeted invitations — an invitation addressed to an account,
  which it could then appear on their dashboard, and which would let a Game
  Master see who has not answered — are a feature of their own and MUST be
  specified separately. They carry their own questions: who may invite, what a
  declined invitation does, and how they sit alongside the anonymous code path
  that already exists. This spec MUST NOT be the reason a new invitation model
  is designed in passing.

**A new account**

- **FR-040**: A person in no worlds MUST see the dashboard and MUST NOT be
  redirected away from it.
- **FR-041**: That dashboard MUST explain and offer both routes into a world:
  creating one, and joining one with a code.
- **FR-042**: A failure to load the world list MUST be distinguished from
  having no worlds, and MUST offer a retry.

**Moving the gallery out**

- **FR-050**: No navigation item, user-menu entry or post-sign-out redirect may
  point at the component gallery.
- **FR-051**: Export-my-data and delete-my-account MUST be reachable from
  account settings and MUST behave exactly as they do today. They are the
  contracts of ADR-004, ADR-005, ADR-011 and ADR-012 and are, today, reachable
  only from the gallery page.
- **FR-052**: The gallery's existing address MUST take a person to the
  dashboard rather than to an error.
- **FR-053**: The gallery MUST remain reachable to the people building the app,
  at a route that is not offered to anybody else.
- **FR-054**: The navigation item shown while setup is required MUST point at
  something about setup.
- **FR-055**: `/welcome` MUST be the address a signed-in person lands on, and
  it MUST be the dashboard. `/counter` MUST redirect there (FR-052), and the
  gallery MUST take a route of its own (FR-053). The product ends this feature
  with **one** landing address, not two, and no existing link to `/welcome`
  breaks.

**Proof**

- **FR-060**: An end-to-end test MUST prove that a pending offer appears on its
  recipient's dashboard, can be answered there, and disappears from a second
  browser without a reload.
- **FR-061**: An end-to-end test MUST prove that a world played in one browser
  moves to the top of that person's dashboard with a last-played time.
- **FR-062**: An end-to-end test MUST prove that a Game Master's and a player's
  dashboards differ as US3 requires, and that neither shows the other's items.
- **FR-063**: An end-to-end test MUST prove that a brand-new account sees the
  dashboard, is not redirected, and is offered both routes into a world.
- **FR-064**: An end-to-end test MUST prove that export-my-data and
  delete-my-account are reachable and working after the gallery leaves the
  navigation.
- **FR-065**: A test MUST prove that a world never played says so, that editing
  a world does not give it a last-played time, and that a Game Master alone in
  a scene does.
- **FR-066**: A test MUST prove that a world's last-played time is the same on
  the dashboard and on the full worlds list, read from one record.
- **FR-067**: A test MUST prove that a paused world's last-played time is the
  one it had when the pause began, and that the card says play is paused
  without hinting why.

### Key Entities

- **Dashboard**: what one signed-in person is shown about their own worlds and
  what awaits them.
- **Waiting item**: something addressed to this person that wants an answer —
  an offer today, and whatever later specs address to a person.
- **Last played**: when a world last had a live session, by anybody. Not a new
  fact — `world_live_play.last_beat_at`, which spec 051's heartbeat already
  writes, and which nothing outside `play_pause` reads yet.
- **World card**: one world, the role held in it, when it was last played, and
  whether its play is paused.
- **Component gallery**: the showcase that `/counter` is today, moved out of the
  product's navigation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A signed-in person can name everything waiting for them across
  every world, in **one step** from signing in.
- **SC-002**: A pending offer appears on its recipient's dashboard within
  **one second** of being created, without a reload, measured the way spec 046
  measures its own one-second bounds.
- **SC-003**: A world played in one browser is at the top of that person's
  dashboard, with a last-played time, in **100%** of runs.
- **SC-004**: **0** items belonging to another person appear on any dashboard,
  verified for a Game Master and a player in the same world.
- **SC-005**: A brand-new account sees the dashboard with **0** redirects and is
  offered **2** routes into a world.
- **SC-006**: **0** navigation items, user-menu entries or redirects point at
  the component gallery.
- **SC-007**: Export-my-data and delete-my-account are reachable in **two
  steps** from the dashboard.
- **SC-008**: With one panel's source made to fail, the remaining panels still
  render, in **100%** of runs.
- **SC-009**: The dashboard meets WCAG 2.2 AA under an automated audit and is
  fully operable by keyboard.
- **SC-010**: A world's last-played time on the dashboard, on the full worlds
  list and on the world's own page agree in **100%** of comparisons, because
  all three read one record.
- **SC-011**: A world played for one minute shows a last-played time within
  **one minute** of that session ending.
- **SC-012**: The product ends this feature with **one** address a signed-in
  person lands on.

## Assumptions

- **A dashboard reads; it does not become a second source of truth.** Every
  panel reads what the world already holds, and answering something from the
  dashboard goes through the same path as answering it in the world
  (constitution Principle I, and Principle III for who may do it).
- **"Last played" is about live play**, not about editing a world's data. That
  is why `worlds.updated_at` is not an acceptable stand-in (FR-015).
- **The heartbeat is the definition, not merely the source.** A world was
  played when a member's client was present in it. This accepts that a Game
  Master preparing a scene alone counts as play, which is the price of having
  one fact per world rather than a rule about how many people were present at
  the same time — and it is the answer a table would give.
- **`/welcome` and the dashboard are the same page, at `/welcome`.** This spec
  does not add a third landing screen, and it removes one address rather than
  adding one (FR-055).
- **There is no such thing as an invitation addressed to a person**, and this
  spec does not invent one (FR-035 to FR-037).
- **Per-world queries are a shape question, not a requirement.** Several sources
  are per world today (claims, offers, pause state). Whether the dashboard asks
  once per world or a single query answers across worlds belongs to the plan;
  the requirement is only that the page loads within its budget.
- **Operator work stays in the admin portal.** An operator's personal dashboard
  is a person's dashboard.

## Out of Scope

- A cross-world activity feed or notification centre. The dashboard shows what
  **awaits an answer**, not everything that happened.
- E-mail or push notification of anything on the dashboard.
- Redesigning the worlds page, the world staging page, or the play dock.
- Any change to what a takedown, a pause or an offer *does*. This spec surfaces
  them.
- Building targeted invitations. FR-037 sends them to their own spec.
- A per-person "when was I last in this world" fact. FR-012 is about the table,
  not the reader.

## Dependencies

- **Spec 017**: actor claims — the characters a person holds.
- **Spec 039**: standing and the moderated-content banner.
- **Spec 042**: the admin portal, which is where operator work stays.
- **Spec 046**: pending offers, the first waiting item.
- **Spec 051**: pause state, and its rule that a table is told *that* and
  *when*, never *why*.

## Decisions (owner, 2026-09-15)

1. **`/counter` is replaced, not improved.** It is a component preview page
   sitting in the product's navigation, and what belongs there is a real
   dashboard for the person signed in.

2. **The gallery survives as a developer's page.** It is useful to the people
   building the app and it is not a product surface. It leaves the navigation.

3. **The dashboard is personal.** Worlds and when each was last played,
   invitations waiting, characters held, offers and requests needing an answer,
   and anything paused or under moderation that affects the reader.

4. **A new account is told what to do next**, on the dashboard, rather than
   being redirected to a form.

5. **"Invitations waiting" is dropped, and targeted invitations become their
   own spec.** Decided 2026-09-15 by the owner, taking option A of the question
   this spec asked, with option C as a feature of its own (FR-035 to FR-037).
   A world invite is an anonymous code with no invitee, so the panel could only
   list nothing; and an invitation addressed to an account is worth doing
   properly — it is also what would let a Game Master see who has not answered.
   The dashboard must not be the reason a new invitation model gets designed in
   passing.

6. **"Played" means any live session in the world, by anybody.** FR-012.
   Decided 2026-09-15 by the owner, taking option A of the question this spec
   asked. It is one fact per world, it is the fact a table would recognise, and
   — as the corrected reading above shows — it is the fact the product already
   records in `world_live_play.last_beat_at`. A solo preparation session counts;
   that is the price of not needing a rule about how many people were present
   at the same time.

7. **Last played is read, not newly recorded.** The spec as first written said
   nothing records it today. That is true of `worlds` and of `world_members`,
   and false of `world_live_play`, which spec 051 added and which holds exactly
   this: a durable per-world timestamp written only while a session is running.
   FR-012a requires reading that record and forbids a second one.

8. **`/welcome` is the address that survives.** FR-055. Decided 2026-09-15 by
   the owner, taking the recommendation the question carried: `/counter`
   redirects to it, the gallery takes a route of its own, one fewer address
   exists in the product, and no existing link to `/welcome` breaks.
