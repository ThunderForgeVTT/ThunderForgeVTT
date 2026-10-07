# Feature Specification: Resumable Downloads

**Feature Branch**: `080-resumable-downloads`
**Created**: 2026-10-06
**Status**: Draft
**Input**: The owner, 2026-10-06, about the "Waking the engine" stage: "when we download the engine during the waking the engine phase is it possible to multi part it?" Then: "it might not buy us something now but its worth while for future adaptations to support large downloads right? like image a scene thats chunkyyyyy we could support multi part", and "multi part the engine though absolutely but dont shrink the dev wasm".

## Why

Some of what a table downloads is big. The engine is the largest thing on
a first visit, and a scene's background map or animated backdrop can be
larger still. Today each of those arrives as one unbroken download. If a
player's Wi-Fi drops 90% of the way through, the download starts over from
the first byte. On a weak connection that can mean never finishing at all,
and a player who cannot load the scene cannot play.

The fix is to download large files in parts. A part that fails is fetched
again on its own, so a drop costs one part, not the whole file. Several
parts can travel at once, which helps on connections that slow each single
download down. As for the engine, it is the same problem in the same place.
Each part is checked as one version of the file, so a file changed on the
server midway is never stitched together from two versions.

Who this is for, in the owner's words: players on a weak computer or a weak
connection should still get a decent experience. The two are helped
differently. A weak connection gains the most: a drop costs one part, and
parallel parts get past a link that throttles each single download. A weak
computer gains less, and the spec does not pretend otherwise. Parts do not
make the engine smaller or quicker to run. What they must not do is make
things worse: the bytes go to the engine in order as they arrive, so it
compiles while the rest downloads, and no file is held whole in memory
twice over on a machine that has little to spare.

The owner decided the engine is in scope, not just scene assets. The owner
also decided the size of the local development build of the engine is not
a problem to solve here: a development build is large by nature.

## What exists

Counted on 2026-10-06:

- **Asset routes.** `crates/thunderforge-server/src/assets_serve/` serves
  the bytes of canvas assets, scene previews, lore images and actor
  portraits. Each route checks the caller's permission, then calls
  `storage::rustfs::read_object`. That function reads the *whole object
  into server memory* before a byte is sent. None of these routes accepts
  a request for part of a file, and none says a file can be fetched in
  parts.
- **Engine and other built files.** These are served from disk under
  `/assets/entry`, `/assets/chunks` and `/assets/static` with a one-year
  cache (`static_files/mod.rs`). They are compressed on every request by a
  server-wide layer (`apps/thunderforge/src/main.rs`), not ahead of time.
  The release engine is about 25 MB raw and about 4–5 MB compressed.
- **Engine loader.** `apps/web/src/engine/bevy/index.ts`
  (`fetchWasmWithProgress`) downloads the engine as one request. It counts
  bytes for the "Waking the engine" progress bar (spec 028 FR-030) and
  passes the stream on, so compiling overlaps downloading.
- **Scene preload.** `apps/web/src/services/scenePreload.ts` (spec 031)
  warms a scene's background in the browser with one request.
- **The engine's world cache.** `crates/thunderforge-engine/src/plugins/cached_assets/wasm.rs`
  fetches each canvas asset whole when no peer has it (spec 028), then
  verifies it by fingerprint before keeping it.
- **Upload cap.** Map imports are capped at 50 MB today
  (`map_import/mod.rs`). Raising it is not part of this spec; this spec is
  what makes a later raise safe to download.

## User Scenarios & Testing

### User Story 1 - A dropped connection does not restart a scene (Priority: P1)

A player joins a session whose scene has a large background map. Partway
through the download their connection drops for a few seconds. When it
comes back, the download carries on from where it was, and the scene
opens. The progress bar never goes back to zero.

**Why this priority**: This is the failure a player actually meets, and
on a weak connection it is the difference between playing and not.

**Independent Test**: Serve a large scene asset, cut the connection
partway through, restore it, and check that the scene opens and that the
bytes fetched after the drop are only what was still missing.

**Acceptance Scenarios**:

1. **Given** a scene with a large background and a player loading it,
   **When** the connection drops after half the file has arrived and comes
   back,
   **Then** the scene opens, and the total bytes downloaded are no more
   than the file's size plus the parts in flight when it dropped.
2. **Given** a download in progress,
   **When** one part fails,
   **Then** only that part is fetched again, and the parts already
   received are kept.
3. **Given** a download that has failed again and again,
   **When** its retries run out,
   **Then** the player sees the same error and retry action the scene
   load offers today, and nothing half-downloaded is shown as the map.

---

### User Story 2 - The engine wakes in parts (Priority: P1)

A visitor opens a board for the first time. The engine downloads in
several parts at once. The "Waking the engine" progress bar fills
smoothly as bytes arrive, compiling starts before the download ends, and
a dropped connection mid-download resumes rather than restarting.

**Why this priority**: The owner named the engine explicitly. It is the
largest download on a first visit, and every new player pays it.

**Independent Test**: Load a board with the network throttled and
interrupted once, and check that the engine starts, that the progress bar
only ever moves forward, and that the bytes fetched match the engine's
size plus at most the parts in flight.

**Acceptance Scenarios**:

1. **Given** a first visit with nothing cached,
   **When** the board loads,
   **Then** the engine arrives in more than one part, and the progress bar
   reports the true bytes received against the true total.
2. **Given** the engine is downloading,
   **When** the early parts have arrived but later ones have not,
   **Then** compiling has already begun on the early parts.
3. **Given** a return visit with the engine already in the browser's
   cache,
   **When** the board loads,
   **Then** nothing is downloaded again, and the engine starts as quickly
   as it does today.

---

### User Story 3 - A file changed on the server is never spliced (Priority: P1)

A Game Master replaces a scene's background while a player is still
downloading the old one. The player ends up with either the old file
whole or the new file whole. They never get a file made of parts of both.

**Why this priority**: A spliced file is corrupt in a way nobody can see
until it fails to draw. A resume that can do this is worse than none.

**Independent Test**: Start a download, replace the file on the server
before it finishes, and check that the result is exactly one version.

**Acceptance Scenarios**:

1. **Given** a partial download of version A,
   **When** the file becomes version B and the download resumes,
   **Then** the downloader notices the change, discards the parts of A,
   and downloads B from the start.
2. **Given** any completed download of a world's canvas asset,
   **When** the engine's cache checks it against its fingerprint,
   **Then** it passes, as it must today.

---

### User Story 4 - Permission is checked on every part (Priority: P1)

A player who may not see a hidden scene's assets cannot fetch any part of
them. Asking for a part is checked exactly as asking for the whole file
is.

**Why this priority**: Splitting a file into parts must not open a side
door. Spec 028 already closed one (T045c).

**Independent Test**: As a player without access, request a byte range of
a hidden scene's asset and check that it is refused just as the whole file
would be.

**Acceptance Scenarios**:

1. **Given** a player who may not see an asset,
   **When** they ask for any part of it,
   **Then** they get the same refusal as for the whole file, and no bytes.
2. **Given** a player who may see an asset,
   **When** they ask for a part of it,
   **Then** they get exactly those bytes and nothing else.

---

### User Story 5 - Small files stay simple (Priority: P2)

Token art, a portrait or a small lore image downloads as one plain
request, as it does today. So does any file from a server that does not
offer parts.

**Why this priority**: Parts only help large files. For a small file they
add requests and slow it down.

**Independent Test**: Load a small asset and check that it arrives in one
request, then load a large one from a server that does not offer parts
and check that it still arrives whole in one request.

**Acceptance Scenarios**:

1. **Given** a file under the size threshold,
   **When** it is downloaded,
   **Then** it arrives in one request.
2. **Given** a server or route that does not say it offers parts,
   **When** a large file is downloaded,
   **Then** it arrives in one request, and nothing fails.

### Edge Cases

- **The server ignores a part request** and sends the whole file instead.
  The downloader uses the whole file and stops asking for parts.
- **The requested part lies outside the file** (the file shrank). The
  server says so, and the downloader starts over against the current
  version.
- **The file's size is not known up front.** The downloader falls back to
  one plain request. It never guesses a total for the progress bar
  (spec 028 FR-030).
- **The browser already holds the file in its cache.** It is used as is,
  with no part requests at all.
- **Compressed delivery.** Built files are compressed ahead of time, so a
  part refers to the same bytes on every request. A part is never taken
  from a copy compressed differently from the copy the other parts came
  from.
- **The player leaves the scene mid-download.** Parts still in flight are
  cancelled. The downloader does not keep fetching a scene nobody is
  opening.
- **Many players load one large scene at once.** The server reads each
  part from storage as it is sent, so memory use per request is bounded by
  the part, not by the file.
- **The demo** (spec 074), which runs with no server, is unaffected. Its
  assets come from its own build.

## Requirements

### Functional Requirements

**Server: serving parts**

- **FR-001**: Every authorized asset route (canvas assets, scene previews,
  lore images, actor portraits) MUST accept a request for a single byte
  range of the file and answer with exactly those bytes, marked as a
  partial answer.
- **FR-002**: Those routes MUST say, on every answer, that the file can be
  fetched in parts, and MUST give the file a version tag that changes
  whenever its content changes.
- **FR-003**: A part request MAY name the version it expects. If the file
  is no longer that version, the server MUST answer with the whole current
  file instead of a part.
- **FR-004**: A request for a range outside the file MUST be refused with
  the file's real size, and no bytes.
- **FR-005**: Permission MUST be checked on every request, part or whole,
  by the same rule as today. A refused caller MUST get no bytes and no
  size.
- **FR-006**: The server MUST read a part from storage as it sends it. It
  MUST NOT load the whole file into memory to serve one part, and it
  SHOULD stop loading whole files to serve whole requests too.
- **FR-007**: Requests for several ranges at once MAY be answered with the
  whole file. The downloader never sends them.

**Server: built files**

- **FR-008**: The engine and the other built files MUST be compressed when
  they are built, and the server MUST serve those stored compressed copies
  rather than compressing on every request. Part requests then refer to
  the same bytes every time.
- **FR-009**: Built files MUST also accept part requests and carry a
  version tag, under the same rules as FR-001 to FR-004, with no
  permission check (they are public today and stay so).
- **FR-010**: A browser that accepts no compression MUST still get the
  file uncompressed, as today.

**Client: the downloader**

- **FR-011**: One downloader MUST serve every large download in the web
  app. It MUST NOT depend on the interface or the engine, so either can
  use it and it can be tested alone.
- **FR-012**: For a file at or above a size threshold, from a server that
  offers parts, the downloader MUST fetch it as several parts, a few at a
  time.
- **FR-013**: The downloader MUST hand the bytes on in file order as they
  become available, as one continuous stream. A part that arrives early
  waits for the parts before it. Compiling the engine MUST keep
  overlapping the download, as it does today. What the downloader holds
  at once MUST be bounded by the parts in flight, not by the file size,
  so a computer with little memory is not made worse off.
- **FR-014**: A part that fails MUST be retried on its own, with a growing
  pause between tries, up to a limit. Parts already received MUST be kept.
- **FR-015**: Every part MUST be fetched against the same version of the
  file. If the version changes midway, the downloader MUST discard what it
  has and start again on the new version.
- **FR-016**: Below the threshold, or when the server offers no parts, or
  when the size is unknown, the downloader MUST make one plain request,
  exactly as today.
- **FR-017**: The downloader MUST report bytes received and the total
  size. The count MUST never go down, retried parts MUST NOT be counted
  twice, and an unknown total MUST be reported as unknown.
- **FR-018**: A cancelled download MUST stop every part still in flight.
- **FR-019**: A download that cannot finish MUST fail with one clear
  error. It MUST NOT deliver a partial file as if it were whole.

**Client: who uses it**

- **FR-020**: The engine loader MUST download the engine through the
  downloader, keeping the "Waking the engine" progress bar (spec 028
  FR-030 to FR-033) and its no-delay rule for return visits.
- **FR-021**: Scene preload (spec 031) MUST warm a large background
  through the downloader, so that a later scene open finds it in the
  browser's cache.
- **FR-022**: The engine's world cache (spec 028) MUST fetch large canvas
  assets with the same part, retry and version rules. Fingerprint
  verification before storing MUST stay exactly as it is.

**Settings**

- **FR-023**: The size threshold, the part size, how many parts travel at
  once and the retry limit MUST be settings with sensible defaults. They
  MUST NOT be scattered constants.

### Key Entities

- **Version tag**: what the server says identifies one exact content of a
  file. It changes when the content changes. Every part of a download is
  checked against it.
- **Part**: a contiguous byte range of one version of a file. It is
  fetched, retried and handed on as a unit.
- **Download**: one file, one version, its parts, the bytes received so
  far and the total. It ends as one whole file or one error.

## Success Criteria

### Measurable Outcomes

- **SC-001**: A large scene download interrupted once at its halfway point
  completes after the connection returns. The extra bytes are no more than
  the parts in flight at the drop, against today's full restart.
- **SC-002**: On a connection that slows each single download, a large
  file arrives at least 1.5× faster in parts than in one request.
- **SC-003**: A first-visit engine load on an ordinary connection takes no
  longer than it does today, and its progress bar never moves backwards.
- **SC-004**: A return visit loads the engine with zero bytes downloaded,
  as today.
- **SC-005**: A file replaced mid-download is never delivered as a mix of
  two versions, in every run of the test that forces it.
- **SC-006**: A caller refused the whole file is refused every part of it:
  zero bytes, every time.
- **SC-007**: Serving a large file to ten players at once keeps the
  server's memory growth bounded by the part size, not by the file size.

### Proof

- **Server tests** cover:
  - partial answers;
  - a range outside the file;
  - a version mismatch answered with the whole file;
  - refusal of a part for a caller without access, on each of the four
    asset routes;
  - built files served precompressed with parts.
- **Downloader unit tests**, with no browser and no server:
  - bytes come out in order when parts arrive out of order;
  - a failed part is retried alone;
  - a changed version restarts the download;
  - plain fetch is used below the threshold and when parts are not
    offered;
  - progress never goes backwards;
  - cancellation stops all parts.
- **An e2e slice, `pnpm e2e:resumable-downloads`** (constitution
  principle VI):
  - It cuts the connection partway through a large scene download and
    shows it resumes rather than restarts.
  - It loads the board and shows the engine arriving in parts and
    starting.
  - Its standalone half runs the downloader against a served test file
    with no stack.

## Assumptions

- The defaults are a 16 MB threshold, 8 MB parts and 4 parts at a time.
  They are tuned during planning against the release engine and a large
  map. Any file below the threshold, the release engine included at its
  present ~4–5 MB compressed, behaves exactly as today until it grows.
  The engine still uses the downloader, so that path is proven now.
- One version tag per stored object is enough. Asset content is replaced,
  not edited in place.
- The storage service can return a byte range of an object. RustFS, being
  S3-compatible, does.
- The development build of the engine stays as large as it is (owner
  decision). The downloader handles it like any large file.
- Raising the 50 MB map upload cap, changing uploads, and serving through
  a CDN or other hosted storage are out of scope (self-hosted first).
- Downloading parts from peers (spec 028's peer transport) is out of
  scope. Peers keep serving whole, fingerprint-checked files.
