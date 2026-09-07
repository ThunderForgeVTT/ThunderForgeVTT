# Feature Specification: Sound at the Table — Scene Audio and Tab Sharing

**Feature Branch**: `038-scene-audio-and-tab-share`

**Created**: 2026-09-07

**Status**: Draft — **specification only, not being built yet.** Written
alongside spec 037; the two share no surface and can be built in parallel.

**Input**: User description: "the spec for the engine would be sound or audio. I'd love to be able to play music or sound effects or something over the engine… one of the things I'd like to work on would be sharing the tab, so the user could click share tab for audio and they could select a tab in their browser. This would allow them to stream from any source that may or may not allow that kind of audio streaming, but I'd like to leave that ambiguous."

## Context

**Sound is the last sense the table has not been given.** The canvas draws,
the lights occlude, tokens move, dice resolve — and all of it happens in
silence. A GM who wants music runs it in another window and asks everyone to
start their own, which is not the same room and never lines up.

### The seam has been waiting for this

Spec 030 built interactive elements and deliberately did **not** build audio.
It is recorded there as a decision rather than a gap, and ADR-054 says exactly
what happens when audio arrives:

> "Audio contributes nothing today because there is no audio, so nothing dead
> is ever offered; the day it exists it contributes `audio.play` and nothing
> else in the system changes."

So this feature has a target that was set for it before it was written: a
threshold that plays a sound when a player crosses it, or a lever that starts
music, becomes authorable **without editing the interaction core**.
`scripts/check-interaction-seam.mjs` already forbids the core naming "sound",
and it must keep passing untouched. If building audio requires changing the
core, the seam did not work and that is worth knowing.

### Two halves, one capability

1. **Audio the product owns.** Music and sound effects that belong to a world,
   played to the table by the GM or by something the GM placed.
2. **Audio the product does not own.** A GM shares a browser tab, and what is
   playing there reaches the table. This is what lets a GM use whatever they
   already use, without ThunderForge integrating with any of it.

### Two kinds of audio, and the line between them

This is the substance of the feature, and the two halves are governed
differently on purpose.

**Uploaded media is content, and it is treated as content.** A GM may upload
their own music and sound effects, and that is entirely supported. It lives in
their world like any other asset, it is theirs, and they may delete it at any
time. If they *share* it — through a collection, a share link, any of the ways
world content already travels — then it is subject to the same
notice-and-takedown program every other content type is (spec 015): if the
work turns out to be somebody else's, a notice can be filed and it comes down.
A GM using media they own, in their own game, is exercising their own
prerogative and nothing here interferes with that.

**A shared tab is not content, and never becomes any.** When a GM shares a
tab — their own Jellyfin server, a streaming service, anything — the product
carries the live audio to the table and keeps **a few seconds of buffer**, for
the single purpose of playing smoothly and together. That buffer is transient
by construction: bounded in seconds, held in memory, discarded as it is
played, never addressable, never written to the world, the device store or the
server. It exists for the same reason any player buffers, and for no other.

**Buffering is not recording**, and the spec says which is which so that a
plan cannot quietly cross the line by calling one the other. A pipe that holds
a few seconds to stay smooth is a different thing from a library, and this
feature is firmly the first.

**What a GM points a shared tab at remains the GM's business.** The product
does not inspect what passes through, does not identify it, does not transcode
it, and takes no position on whether a given source permits it. That ambiguity
is intended — and it is tenable precisely because there is nothing to file a
notice against: no copy exists to take down.

### Constitution checkpoint — DMCA guardrail

The guardrail applies to the **uploaded** half, because a world's audio can
travel the way its other content already does. Recorded here rather than
assumed either way:

- **(a) The notice-and-takedown program is operational.** Verified in the
  codebase: `src/server/src/moderation/` implements intake, disable,
  counter-notice with a waiting period and lazy auto-restoration, and
  repeat-infringer tracking with a lookback and a threshold; spec 015's task
  list is complete.
- **(b) The "centralized public repository" determination is
  OUTSTANDING** and belongs to the accountable owner before implementation
  begins. The reading this spec is written under: uploading audio adds a
  **content type to an exposure that already exists** (collections and share
  links already carry actors, items, abilities and lore) rather than creating
  a new public repository — so it is the former, not the latter. That reading
  needs to be confirmed on record, not inherited from this paragraph.

### What a person will actually notice

Three constraints that decide whether this feels good or broken, and all three
are user-facing rather than technical:

- **A browser will not play a sound until the person has interacted with the
  page.** The first sound of a session is a real problem, not a footnote.
- **Everyone must be able to make it stop.** Audio is the one medium that
  cannot be looked away from. A person who cannot silence it has had something
  done to them.
- **Together is not the same as identical.** Sample-synchronised playback
  across browsers on different networks is not achievable; arriving at roughly
  the same moment is. The spec promises the second.

## Clarifications

### Session 2026-09-07

- Q: May a GM upload their own music and media to the instance? → A: **Yes,
  and it is entirely supported.** It becomes world content, under that world's
  permissions, and the GM may delete it whenever they choose.
- Q: What happens if uploaded media turns out to be copyrighted? → A: If it is
  **shared**, the existing notice-and-takedown program applies to it exactly as
  it applies to every other content type. If it is used privately in the GM's
  own game and the GM owns it, that is their prerogative and nothing
  interferes.
- Q: Does a shared tab get cached? → A: **No.** A few seconds are buffered so
  playback is smooth and lands together for everyone; the buffer is bounded,
  held in memory, discarded as it plays, and never written anywhere. Buffering
  to play is not recording, and the spec draws that line explicitly (FR-018,
  FR-018a).
- Q: So where does the ambiguity about sources actually sit? → A: On the
  **share** path only. Uploaded media is content and is moderated as content.
  A shared tab produces no copy, so there is nothing to moderate — which is
  what makes taking no position on the source defensible rather than evasive.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The GM plays music for the table (Priority: P1)

A GM starts a track. Everyone at the table hears it, roughly together. The GM
stops it, or changes it, and everyone follows. It belongs to the scene they
are playing, and someone who joins late hears what is currently playing rather
than nothing or the beginning.

**Why this priority**: it is the feature in one sentence. Everything else is
this with a different trigger or a different source.

**Independent Test**: with two clients at one table, start a track from the GM
and confirm both hear it; stop it and confirm both stop.

**Acceptance Scenarios**:

1. **Given** a GM at a scene, **When** they start a track, **Then** every
   client at that table begins playing it within a couple of seconds of each
   other.
2. **Given** audio playing, **When** the GM stops it, **Then** every client
   stops.
3. **Given** audio playing, **When** a person joins the table, **Then** they
   hear what is currently playing rather than silence or a restart.
4. **Given** a GM moving the table to another scene, **When** the scene
   changes, **Then** what plays follows the rule the GM set for it rather than
   surprising them.
5. **Given** a player rather than a GM, **When** they try to start audio for
   the table, **Then** they cannot — playing to the table is the GM's.

---

### User Story 2 - Anyone can silence it (Priority: P1)

A person at the table sets their own volume, or mutes entirely, and nothing
the GM does overrides that. Whenever audio is playing or a tab is being
shared, everyone can see that it is — including the person sharing.

**Why this priority**: equal to US1, and non-negotiable. Audio is the one
medium a person cannot avert their eyes from, and the first person to hit an
unmutable track is the last time they trust the feature. The visible indicator
matters just as much on the sending side: nobody should broadcast without
knowing they are.

**Independent Test**: with audio playing, mute as a listener and confirm
silence persists across a track change, a scene change and a reload.

**Acceptance Scenarios**:

1. **Given** audio playing, **When** a listener mutes or lowers their volume,
   **Then** it applies immediately and no GM action restores it.
2. **Given** a listener who has muted, **When** the GM starts something new,
   **Then** it stays muted.
3. **Given** a listener's volume choice, **When** they reload or return later,
   **Then** their choice is still in effect.
4. **Given** any audio playing or tab being shared, **When** a person looks at
   the screen, **Then** something visible says so.
5. **Given** a GM sharing a tab, **When** they are sharing, **Then** they can
   see that they are and can stop it in one action.

---

### User Story 3 - The first sound of a session actually plays (Priority: P1)

Someone opens the table and audio is already playing. Their browser will not
make a sound until they have interacted with the page. Rather than silently
playing nothing, the product tells them audio is waiting and lets them turn it
on deliberately.

**Why this priority**: without it the feature's most common first impression is
"it doesn't work", and the reason is invisible. It is a small piece of design
that decides whether US1 lands.

**Independent Test**: open a table with audio already playing in a browser
that has had no interaction, and confirm the person is told and can start it
in one action.

**Acceptance Scenarios**:

1. **Given** a fresh page and audio playing at the table, **When** the browser
   refuses to play it, **Then** the person is told audio is waiting rather
   than being left with silence.
2. **Given** that prompt, **When** the person accepts, **Then** audio starts
   from where the table is, not from the beginning.
3. **Given** a person who ignores the prompt, **When** they carry on playing,
   **Then** nothing else is blocked or degraded by the absence of sound.

---

### User Story 4 - Share a tab's audio to the table (Priority: P2)

A GM presses "share tab for audio", picks a browser tab, and what is playing
there reaches the table. They stop sharing and it stops. Nothing about what
they shared is kept by the product.

**Why this priority**: the capability most likely to be used every session,
and the one that needs no content pipeline at all. P2 rather than P1 only
because US1–US3 establish the controls it inherits — the mute, the indicator,
the first-sound prompt.

**Independent Test**: share a tab producing sound and confirm a second client
at the table hears it, that the sharing GM sees an indicator, and that
stopping the share stops the audio everywhere.

**Acceptance Scenarios**:

1. **Given** a GM at a table, **When** they share a tab that is producing
   audio, **Then** the people at that table hear it.
2. **Given** an active share, **When** the GM stops it — or closes the tab,
   or the browser ends the share — **Then** the audio stops at the table and
   the indicator clears.
3. **Given** an active share, **When** anyone looks for a recording of it,
   **Then** there is none: not in the world's assets, not in the client cache,
   not on the server.
4. **Given** a person at the table, **When** a share is playing, **Then**
   their own volume and mute apply to it exactly as to any other audio.
5. **Given** a GM who tries to share from a companion window rather than the
   play field, **When** they try, **Then** they are told sharing belongs to the
   play field.

---

### User Story 5 - A trigger plays a sound (Priority: P2)

A GM authors a threshold that plays a sound when a player crosses it, or a
lever that starts music — using the same authoring they already use for doors
and lights, with sound simply present in the list where it was previously
absent.

**Why this priority**: this is the promise spec 030 recorded, and it is nearly
free once US1 exists. P2 because the GM playing audio directly is the more
common need.

**Independent Test**: author a sound onto an interactive element, trigger it,
and confirm it plays for the table — with no change to the interaction core.

**Acceptance Scenarios**:

1. **Given** audio exists, **When** a GM authors an interactive element,
   **Then** playing a sound is offered alongside the existing effects.
2. **Given** a threshold with a sound, **When** a player crosses it, **Then**
   the table hears it, subject to each listener's own volume.
3. **Given** the interaction core, **When** audio is added, **Then** the core
   is unchanged and the seam check still passes without modification.
4. **Given** a permission that would refuse the effect, **When** it is
   refused, **Then** no sound plays — audio is not a way around adjudication.

---

### User Story 6 - Sounds are world content (Priority: P2)

A GM adds sound files to their world the way they add maps and token art. The
files belong to the world, obey its permissions, and are kept on the device
like other world content so a returning table is not re-downloading them.

**Why this priority**: it is what makes US1 and US5 usable more than once, but
both can be demonstrated before it with content already present.

**Independent Test**: add a sound to a world, play it, revisit, and confirm it
came from the device rather than the network.

**Acceptance Scenarios**:

1. **Given** a world, **When** a GM adds a sound file, **Then** it becomes part
   of that world's content under that world's permissions.
2. **Given** a returning table, **When** a sound plays that was played before,
   **Then** it comes from the device rather than being downloaded again.
3. **Given** a person with no access to a world, **When** they request its
   sounds, **Then** they are refused as they would be for any other content.
4. **Given** a shared tab, **When** anyone looks at the world's content,
   **Then** the shared audio is not in it (FR-021).

---

### Edge Cases

- A listener has no audio output at all, or has the tab muted at the browser
  level. The product should not report that sound is playing to them when it
  is not reaching them, if it can tell.
- The GM's connection drops mid-share. The audio stops; the table is told why
  rather than left wondering.
- Two GMs at one table both try to play something. One table, one thing
  playing — the spec says which, rather than mixing arbitrarily.
- Audio is playing when the table moves to a different scene, or the play field
  is taken over by another window of the same account (spec 036).
- A sound file is enormous, or is not audio at all, or is an audio format the
  browser will not play.
- A player crosses a triggering threshold twenty times in ten seconds.
- Someone is running a browser where the world cache is unavailable — sound
  still plays, it is simply fetched each time.
- A share is started with no audio in the tab at all. Nothing plays and the
  GM is told, rather than an indicator suggesting it is working.
- Accessibility: a person who cannot hear must lose nothing but the sound —
  no information may exist only in audio.

## Requirements *(mandatory)*

### Functional Requirements

**Playing to the table**

- **FR-001**: A GM MUST be able to play, stop and replace audio for their
  table, and every client at that table MUST follow.
- **FR-002**: Playing audio to the table MUST be a GM capability; a player
  MUST NOT be able to play to the table.
- **FR-003**: A client joining a table where audio is playing MUST join what
  is currently playing rather than starting it over.
- **FR-004**: Audio MUST reach every listener at a table within a small,
  stated interval of each other. Sample-accurate synchronisation is NOT
  promised and MUST NOT be implied by the interface.
- **FR-005**: What plays MUST be attached to a scene or to the table
  explicitly, and the GM MUST be able to tell which — so a scene change does
  not surprise them.
- **FR-006**: One table MUST have one authority over what is playing; two GMs
  MUST NOT be able to produce two simultaneous tracks by accident.

**Listener control**

- **FR-007**: Every listener MUST have their own volume and mute, applying to
  everything this feature can play.
- **FR-008**: A listener's volume and mute MUST NOT be overridable by any GM
  action, including starting new audio.
- **FR-009**: A listener's volume and mute MUST persist across reloads, scene
  changes and sessions.
- **FR-010**: Whenever audio is playing or a tab is being shared, an indicator
  MUST be visible — to listeners, and to the person sharing.
- **FR-011**: A person sharing MUST be able to stop sharing in one action.
- **FR-012**: No information MUST exist only in audio; anything conveyed by
  sound MUST also be available to a person who cannot hear it.

**The first sound**

- **FR-013**: Where the browser refuses to play audio before a person has
  interacted with the page, the person MUST be told that audio is waiting.
- **FR-014**: Accepting that prompt MUST start audio at the table's current
  position, not from the beginning.
- **FR-015**: Nothing else in the product MUST be blocked or degraded because
  audio has not been enabled.

**Sharing a tab**

- **FR-016**: A GM MUST be able to share a browser tab's audio to their table
  and stop it again.
- **FR-017**: The product MUST NOT inspect, identify, transcode or classify
  what a shared tab is playing, and MUST take no position on whether the
  source permits it. What is shared is the GM's choice and the GM's
  responsibility.
- **FR-018**: Shared audio MUST NOT be recorded, stored or cached anywhere —
  not as world content, not on the device store, not on the server.
- **FR-018a**: A bounded live buffer of a few seconds MUST be permitted, and is
  required, so that playback is smooth and reaches everyone together. It MUST
  be bounded in seconds, held only in memory, discarded as it is played, never
  addressable and never written to any store. The bound MUST be stated. This
  is the line between buffering and recording, and it is drawn here so a plan
  cannot cross it by calling one the other.
- **FR-019**: Shared audio MUST end when the share ends, when the sharing
  client goes away, or when the browser ends the share, and the table MUST be
  told which.
- **FR-020**: Sharing MUST be available only from the play field, consistent
  with spec 036 FR-038 — a companion surface may not share.
- **FR-021**: Shared audio MUST NOT appear in any listing of a world's
  content.
- **FR-022**: A listener's own volume and mute MUST apply to shared audio
  identically to any other audio.

**Sounds as world content**

- **FR-023**: Sound files MUST be able to belong to a world, under that
  world's permissions, in the same way as other world assets.
- **FR-024**: A world's sounds MUST be kept on the device like other world
  content, so a returning table does not download them again — and MUST still
  play where that is unavailable.
- **FR-025**: A file that is not playable MUST be refused when it is added,
  with a reason, rather than failing silently at the moment a GM needs it.
- **FR-026**: Sound content MUST be bounded in size per file and per world,
  and the bounds MUST be stated.

**Uploaded media, ownership and takedown**

- **FR-026a**: A GM MUST be able to upload their own music and sound effects,
  and what they upload MUST belong to their world under that world's
  permissions.
- **FR-026b**: A GM MUST be able to delete media they uploaded at any time,
  and deletion MUST remove it from the world and stop it being served.
- **FR-026c**: Audio MUST be a moderated entity type from the day it exists —
  a notice filed against a piece of uploaded audio MUST land on that audio.
  (Scenes are not moderated today and a takedown against one lands on its
  images instead; audio MUST NOT ship with the same gap.)
- **FR-026d**: Where uploaded audio travels beyond its world by any existing
  mechanism — a collection, a share link — the notice-and-takedown program
  MUST apply to it on the same terms as every other content type.
- **FR-026e**: A takedown against uploaded audio MUST stop it playing at every
  table it reaches, not merely hide it from a listing.
- **FR-026f**: Media used privately by a GM in their own world MUST NOT be
  treated differently from any other private world content; nothing in this
  feature inspects, classifies or reports on it.
- **FR-026g**: A shared tab MUST NOT be a moderated entity, because it
  produces no copy. If a plan ever gives it one, that copy becomes content and
  FR-026c through FR-026e apply to it.

**The interaction seam**

- **FR-027**: Playing a sound MUST become an authorable effect on interactive
  elements, offered alongside the existing effects.
- **FR-028**: Adding audio MUST require no change to the interaction core, and
  the existing seam check MUST continue to pass unmodified. This is the
  acceptance criterion for ADR-054 having worked.
- **FR-029**: A refused interaction MUST play no sound; audio MUST NOT bypass
  permission or approval.
- **FR-030**: Repeated triggering MUST NOT produce unbounded overlapping
  playback.

**Cost**

- **FR-031**: The download cost added by audio MUST be measured when it lands
  and published as a generated figure, never estimated or transcribed. Spec
  031 skipped an equivalent step and the number proved unrecoverable
  afterwards; this requirement exists because of that.

### Key Entities

- **Sound**: a piece of audio belonging to a world, with a name, a file and
  the world's permissions.
- **Playback state**: what a table is currently playing, from where, and since
  when — enough for a joining client to arrive in the right place.
- **Listener preference**: one person's volume and mute, theirs alone and
  never overridden.
- **Share**: a live audio stream from a GM's browser tab to their table.
  Exists only while it is running; has no stored form by design.

## Success Criteria *(mandatory)*

- **SC-001**: A GM can start audio for their table in under 15 seconds from
  the play field, and every client is playing it within a couple of seconds of
  each other.
- **SC-002**: A listener can silence everything this feature plays with a
  single action, and it stays silent through a track change, a scene change
  and a reload.
- **SC-003**: 100% of the time audio is playing or a tab is shared, an
  indicator is visible to both the listeners and the sharer.
- **SC-004**: A person opening a table where audio is already playing is told
  so, and can start it in one action — never left with unexplained silence.
- **SC-005**: A GM can share a tab's audio to their table in under 30 seconds
  without configuring anything.
- **SC-006**: No shared audio is recoverable from the product after the share
  ends — demonstrated by looking for it in world content, on the device and on
  the server.
- **SC-007**: A returning table plays a previously-heard sound from the device
  rather than the network.
- **SC-008**: A sound can be attached to an interactive element and triggered
  in play, with the interaction core unchanged and its seam check passing
  unmodified.
- **SC-009**: The download cost added by audio is a published, generated
  figure at the moment the capability lands.
- **SC-010**: A GM can upload their own media, use it, and delete it again,
  with deletion stopping it being served everywhere within one session.
- **SC-011**: A notice filed against a piece of uploaded audio stops it
  playing at every table that reaches it, demonstrated end to end rather than
  by hiding it from a list.
- **SC-012**: Held audio from a shared tab never exceeds the stated buffer
  bound, and none of it is recoverable once played — demonstrated by looking
  in the world's content, the device store and the server.

## Assumptions

Defaults taken where the description did not settle something. Each is a
candidate for `/speckit-clarify`.

- **One table plays one thing at a time.** A music bed plus a simultaneous
  effect is a mixing model, and mixing is deliberately deferred; the simplest
  honest behaviour is that a new thing replaces the old, with short effects the
  exception.
- **The GM's client is the source of a shared tab**, and sharing therefore
  requires the GM to be present. There is no server-side stream.
- **Listener preferences are per person per device**, not synchronised between
  a person's own clients — a headset on one machine and speakers on another
  want different volumes.
- **Audio follows the scene by default**, with the GM able to say otherwise.
- **Sound files are ordinary world assets** and inherit the storage,
  permission and caching behaviour those already have — and, from FR-026c,
  the moderation behaviour too.
- **The buffer is a few seconds, not a few minutes.** The exact bound is a
  planning decision; what is fixed here is that it is small, stated, and
  measured in seconds.
- **Nobody is charged for bandwidth by this feature** beyond what world assets
  already cost; shared audio is peer-carried where peers exist and otherwise
  is not offered.
- **Chromium only**, as everywhere else in this product. Tab audio sharing in
  particular is a capability that varies between browsers, and the suite
  proves nothing about the ones it does not run.

## Out of Scope

- A mixer: layered beds, crossfades, ducking, per-sound levels, playlists.
- Voice chat between people at the table. This feature carries audio *to* the
  table, not *between* the people at it.
- Positional or directional audio tied to where a token stands.
- Any recording, capture or export of audio from a shared tab. FR-018 is a
  boundary of the feature, not a phase of it — the live buffer of FR-018a is
  the sole exception and is not a copy.
- Inspecting, fingerprinting or licence-checking what a GM uploads. The
  notice-and-takedown program is the mechanism, as it is for every other
  content type; proactive content identification is not proposed here.
- Identifying, licensing, cataloguing or policing what a GM shares.
- Bundled music or sound libraries shipped with the product. The starter map
  is already blocked on licensing; sound would be the same problem again.
- Firefox and Safari support.
