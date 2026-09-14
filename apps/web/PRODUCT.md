# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Game Masters.** They build a persistent world (scenes, walls and lighting,
  tokens, actors, items, lore, and books imported into their own library) and
  then run sessions in it. They do most of the authoring and most of the
  steering during play.
- **Trusted Players.** A table's co-DM for content. They manage a world's book
  material (switching the owner's books on, changing what the world inherited)
  and adopt what players bring, without the rest of a Game Master's authority.
  In everything else they see what a Player sees.
- **Players.** They join a world by invite, pick or bring a character, and play
  on the shared canvas. They see what the Game Master and the rules allow and
  nothing more. A Player changes a world's content in exactly one way: bringing
  a character sheet, which is staged under their name until someone who runs
  the table adopts it.
- **Instance operators.** People who self-host a ThunderForge instance. They
  run first-time setup, the admin portal, mail and storage, moderation and the
  notice-and-takedown programme, and they publish their own legal pages.

## Product Purpose

ThunderForge VTT is an open-source virtual tabletop. It exists so a group can
disappear into a shared fantasy for an evening, with the tool helping that
happen instead of getting in the way. In the creator's words, it should
"help facilitate the fantasy and not be an encumbrance on it", and be easy to
host, easy to use, and resilient.

Success means a table forgets the software is there. Worlds persist, upgrades
don't lose anything, and nobody is locked out of their own game by a platform,
an edition change, or a paid module that stopped being maintained.

## Positioning

- **Owned by the table, not a platform.** It is open source and self-hostable,
  and worlds belong to the people who made them. This comes from the creator
  having been burned by upgrades that broke campaigns and lost data, by one
  publisher's edition changes damaging the source material, and by paid
  modules that fell behind.
- **Any game system, as a pack.** No system is built into shared code: a
  system declares its own rules, vocabulary and content patterns in its pack,
  and a new system is a pack rather than a rewrite.
- **A human Game Master, always.** AI may assist a Game Master. It never
  replaces one, because the people at the table are the point.

## Operating Context

- **Every kind of table, equally.** In-person, online-only and mixed tables all
  count as first-class. No single scene gets priority: the UI has to work for a
  Game Master at a laptop, for a map shown on a shared screen in a room, and
  for remote and in-room players in the same session.
- **The shape of a session.** A Game Master prepares a world outside play,
  mostly in the world's staging and compendium pages. They then run a session
  on the play canvas, where the map, tokens, fog, lighting and dice live.
  Players arrive by invite link and pick an actor.
- **Their own books.** Game Masters import rulebooks they own from their own
  machine. The PDF is read locally, reviewed before anything is sent, and ends
  up on their account's library shelf, where worlds switch books on (specs 049
  and 050).
- **Browser.** Only Chromium is supported for now. That is a real constraint:
  the offline world cache depends on OPFS, WebCrypto and IndexedDB.
- **Connection.** The starting recommendation is an unmetered connection of at
  least 50 Mbps. That number is untested. Metered and mobile-data connections
  are untested too, and out of scope for the MVP.

## Capabilities and Constraints

- **The canvas belongs to the engine.** Everything drawn on, dragged across or
  spatially queried on the game canvas lives in the Bevy (WebAssembly) engine.
  React owns the chrome around it: toolbars, panels, inspectors, and every page
  that isn't the canvas. Canvas UI is engine work, not React work.
- **Authorization is server-side and authoritative.** The client may update
  optimistically, but never decides what a person is allowed to see or do.
- **No game-system vocabulary in shared code**, enforced by a build check.
  Game-specific sheets and content come from packs. A publisher's sheet shows
  what a system tracks, never how ThunderForge should look.
- **Sharing is decided by origin, not licence.** Content authored in
  ThunderForge can be shared. Content read out of an uploaded PDF (a source
  book or a character sheet) never can. Imported maps and uploaded images
  count as authored and are policed by takedown instead: a scene can be taken
  down along with its images, with gaps recorded in spec 015 T042. Uploaded
  content is not shared, published, exported or adopted into a collection, and no
  role can change that, instance operators included. This is the owner's
  decision (spec 049, decision 4), accepted as ADR-097.
- **Operators and takedowns.** The notice-and-takedown programme exists and
  gates any feature that exposes one world's content outside that world.
- **Operators can pause a world's play.** An operator, and nobody in the
  world, can pause a world's live play: everyone at the table leaves the
  canvas, nobody can start playing it again until an operator lifts the pause,
  and nothing else about the world changes. A takedown that lands on a table
  playing it asks an operator to decide; it never pauses on its own (spec 051,
  ADR-100).
- **Terminology.** Keep these apart; they are not interchangeable:
  - **World:** a persistent campaign.
  - **Scene:** a map inside a world.
  - **Token:** a piece on the canvas.
  - **Actor:** a character or NPC record.
  - **Compendium:** the world's content portal page.
  - **Collection:** content a user authored, grouped to share.
  - **Library:** an account's shelf of imported books.
  - **Pack:** a compiled directory for a game system or interface. Never a
    name for user content.
  - **Staging compendium:** what a player brought to a world, named for them,
    waiting to be adopted.
  - **Game Master** is written in full.
- **Roles, in rank order.** Owner, Game Master, Trusted Player, Player
  (ADR-099). Book material and adoption are open to Trusted Players and above;
  books always come from the world owner's shelf, whoever switches them on.

## Brand Commitments

- **Name and assets.** The product is **ThunderForge VTT**. The mark is
  `apps/web/public/brand-mark.svg`, and the social card
  (`apps/web/public/social-card.svg`) carries the line "Persistent worlds,
  secure setup, and collaborative tabletop play."
- **Styling is ThunderForge's own**, whatever a game system's publisher does.
- **Honest voice.** The project says plainly what it doesn't know rather than
  dressing up a guess as a fact. The README calls its connection figure
  untested and would "rather say so than let a guess pass for a requirement."
  Product copy should hold that standard: no inflated claims, and limits stated
  where a person will run into them.
- **A pause is told calmly, and never explained.** Most people at a paused
  table have done nothing wrong, and a reason shown to the table can tip off
  whoever is being looked into. So the notice and every member-facing trace of
  a pause say *that* play was paused by an operator and *when*, in plain,
  unaccusing words, with what a person can still do. They never say why, who,
  what triggered it, or "takedown", "report" or "violation".

## Evidence on Hand

- **The creator's note** in the repository `README.md` is the source for the
  product's purpose and the frustrations behind it.
- **Pre-release.** There are no public users, testimonials, usage numbers,
  pricing or case studies. None may be invented.
- **The owner's rulebook library** (246 commercial PDFs) is private test
  material for the import work. It must never appear in the product, in
  marketing, in screenshots, or in anything published.
- **The maps in `example/maps` are not redistributable.** Use them only as
  development and test fixtures. Never seed them into worlds or ship them.

## Product Principles

1. **Facilitate the fantasy.** Every feature is judged by whether it helps the
   table get lost in the game. Friction that pulls people out of it counts as a
   defect, however capable the feature is.
2. **Never an AI Game Master.** Tools may assist a human Game Master. Nothing
   may take their place, because the community at the table is the point.
3. **Easy to host, and never lose a world.** Self-hosting must be
   straightforward, and upgrades must never lose or break a campaign. Resilience
   outranks novelty.
4. **Every system is a pack.** Shared code learns no game's vocabulary.
   Supporting a new system means writing a pack, and ThunderForge's look stays
   its own.

## Accessibility & Inclusion

**WCAG 2.2 AA is required**, as a hard floor that all future UI work is audited
against. Because in-person, online and mixed tables are all first-class, that
floor has to hold on a Game Master's laptop and on a shared screen read from
across a room.
