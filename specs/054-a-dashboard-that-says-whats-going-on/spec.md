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

Most of the panels the owner named already have a source. Two do not, and this
spec says so rather than assuming.

| Panel | Source today |
|-------|--------------|
| Worlds and my role in each | `myWorldsWithRole` (`apps/web/src/api/world.ts:69-80`; `src/server/src/graphql/queries/user.rs:166`) |
| Characters I hold | `myActorClaim(worldId)` (`apps/web/src/api/actorClaims.ts:50-62`; `src/server/src/graphql/mutations_actor_claims.rs:241`) — **per world**, so a dashboard asks once per world or needs one query across them |
| Offers needing an answer | `pendingOffers(worldId)` (`apps/web/src/api/attacks.ts:159-168`; spec 046 FR-008) — **per world**, same shape |
| A world's play paused | `worldPlayState(worldId)` (`apps/web/src/api/playPause.ts:52-65`; `src/server/src/graphql/queries/play_pause.rs:459`) — **per world**; no "which of my worlds are paused" |
| My content under moderation | `myStanding` and `myNotices` (`apps/web/src/api/standing.ts:104-122`; `src/server/src/graphql/queries/standing.rs:154-179`), and the per-item banner `ModeratedContentBanner` |
| Recent activity in my worlds | `myWorldEvents` (`src/server/src/graphql/queries/user.rs:206`) — **owned worlds only** |
| **Invitations waiting for me** | **Nothing.** See below |
| **When a world was last played** | **Nothing.** See below |

**There are no invitations addressed to a person.** A world invite is an
anonymous *code* with `max_uses` and `used_count` and no invitee
(`src/server/src/schema.rs`, `world_invites`); it is read by
`worldInvites(worldId)`, which only an Owner or Game Master may call
(`src/server/src/graphql/queries/invite.rs:311`), and redeemed by whoever holds
the link. So "invitations waiting" is not a panel that can be populated — it is
a feature that does not exist. Q1 below asks how far this spec should go.

**There is no last-played timestamp.** The `worlds` table has `created_at` and
`updated_at` and no `last_played_at`; `world_members` has `joined_at` and no
last-visit column. Spec 051 made the server keep a throttled per-world
heartbeat while a world is in live play (ADR-100 decision 5), which is the
nearest existing fact and is about the world, not about the person. "When each
was last played" therefore needs something recorded that nothing records today
(FR-012).

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

- **FR-012**: Each world MUST show when it was last played. "Played" means a
  live session in that world, not an edit to the world's data. The product MUST
  record this; nothing records it today.
- **FR-013**: The world list MUST be ordered by when each world was last
  played, most recent first, with worlds never played after them.
- **FR-014**: Each world MUST show the role the person holds in it.
- **FR-015**: A world never played MUST say so, and MUST NOT show its creation
  or modification date in place of a session.
- **FR-016**: A world whose play an operator has paused MUST say that play is
  paused and when, and MUST NOT state or hint at why (spec 051 FR-050).
- **FR-017**: The list MUST be bounded and MUST offer a way to the full list of
  worlds.

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

### Key Entities

- **Dashboard**: what one signed-in person is shown about their own worlds and
  what awaits them.
- **Waiting item**: something addressed to this person that wants an answer —
  an offer today, and whatever later specs address to a person.
- **Last played**: when a world last had a live session. A new fact; nothing
  records it today.
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

## Assumptions

- **A dashboard reads; it does not become a second source of truth.** Every
  panel reads what the world already holds, and answering something from the
  dashboard goes through the same path as answering it in the world
  (constitution Principle I, and Principle III for who may do it).
- **"Last played" is about live play**, not about editing a world's data. That
  is why `worlds.updated_at` is not an acceptable stand-in (FR-015).
- **`/welcome` and the dashboard are the same page.** This spec does not add a
  third landing screen; whichever address survives, there is one place a person
  lands.
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
- Building targeted invitations, unless Q1 is answered that way.

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

## Questions for the owner

1. **Q1 — "Invitations waiting" has nothing behind it. How far should this spec
   go?** A world invite is an anonymous code with a use count and no invitee
   (`world_invites`), readable only by the world's Owner or Game Master. There
   is no such thing as an invitation addressed to a person.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Drop the panel from this spec and say why | Honest and small. The owner's list loses an item. |
   | B | **Show invite codes the person has been handed but not yet redeemed**, once they have pasted one | Needs somewhere to hold a code a person has not used, which is a new idea with its own questions about a code that someone else consumes first. |
   | C | Build targeted invitations — an invitation addressed to an account, which appears on their dashboard | The panel the owner described, and a feature of its own size: who may invite, what a declined invitation does, how it interacts with the anonymous code path that already exists. |

   **Recommendation: A now, C as its own spec.** The dashboard should not be
   the reason a new invitation model gets designed in passing, and an invitation
   addressed to an account is worth doing properly — it is also what would let a
   Game Master see who has not answered.

2. **Q2 — What counts as "played", exactly?** FR-012 needs a rule, and the
   candidates give different answers for a Game Master who opened a scene alone
   to prepare it.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | **Any live session in the world, by anybody** | One fact per world, matches "when was this table last played", and a solo preparation session counts. |
   | B | A live session with two or more people | Truer to "played"; a Game Master preparing alone does not disturb the ordering. Needs a definition of "at the same time". |
   | C | Per person: when *I* was last in it | Orders each person's own list by their own history; a different fact per member, and a world a person has never opened has no date. |

   **Recommendation: A.** Spec 051 already keeps a throttled per-world
   heartbeat while a world is in live play (ADR-100 decision 5), so option A is
   the fact the product is nearest to holding, and it is the one a table would
   recognise.

3. **Q3 — Which address survives, `/welcome` or the dashboard's?** They are the
   same page under this spec. Recommendation: keep `/welcome` as the address a
   person lands on, redirect `/counter` to it, and give the gallery a route of
   its own — one fewer address in the product, and no existing link to `/welcome`
   breaks.
