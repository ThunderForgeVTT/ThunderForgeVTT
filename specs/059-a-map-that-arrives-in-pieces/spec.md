# Feature Specification: A Map That Arrives in Pieces

**Feature Branch**: `059-a-map-that-arrives-in-pieces`

**Created**: 2026-09-16

**Status**: Draft

**Input**: Project owner, 2026-09-16: "Since I host on Cloudflare and Cloudflare
has a 100MB limit upload, could we realistically convert to multipart uploads?
Especially for scenes." Asked the same day an import failed with "Failed to
upload the map file. Check your connection and try again."

## The problem

A map reaches the server as one HTTP request carrying the whole file. That
single shape causes four separate failures, and only the first is the one the
owner asked about.

### One request cannot pass a proxy's body limit

Map import is `POST /api/scenes/{scene_id}/import/uvtt`
(`src/server/src/map_import/mod.rs:76`), a single `multipart/form-data`
request read whole into memory (`:425-435`). Everything the browser sends is
one request body. Cloudflare refuses a proxied request body over its plan's
limit, and a Cloudflare Tunnel's public hostname is a proxied hostname, so the
same refusal applies behind `make dev-tunnel`'s `cloudflared`.

The limits, as Cloudflare's own documentation states them on 2026-09-16
(Workers platform limits, "Request body size", and the Error 413 support page):

| Plan | Maximum request body |
|---|---|
| Free | 100 MB |
| Pro | 100 MB |
| Business | 200 MB |
| Enterprise | 500 MB by default; self-serve up to 5 GB (changelog, 2026-09-04) |

Cloudflare's own 413 page lists "break up requests into smaller chunks" as the
first remedy. Its documentation does not mention tunnels on that page; the
limit is a property of the proxied zone, and the owner's hostname is one.

A `.dd2vtt` file is JSON with the map image embedded as base64, which makes the
image about a third larger in transit than it is on disk. The example
maps bear that out exactly: `demo.dd2vtt` is 4,237,625 bytes holding a
3,175,260-byte WebP (1.33x). An exported battlemap that is 75 MB as an image is
over 100 MB as a `.dd2vtt`.

A second, less visible limit sits on the same path. Cloudflare's proxy read
timeout is **125 seconds** (Connection limits, "Proxy Read Timeout", error
524), configurable only on Enterprise. Today the browser's one request stays
open while the server decodes, resizes, encodes and stores the art, so a slow
import can fail at the edge after the bytes have all arrived.

### A map that is too large is reported as a network fault

The cap is `MAX_UPLOAD_BYTES = 50 MB` (`map_import/mod.rs:64`), applied twice:
as axum's `DefaultBodyLimit` on the route (`:77`) and as a check on the file
field after it is read (`:432`). They are the same number, and the body limit
counts the whole multipart body, which is always larger than the file inside
it. So the body limit always trips first, inside `field.bytes()`, and is mapped
to **400** with `"Failed to read file bytes: Error parsing
multipart/form-data request"` (`:426-431`). The clean **413** at `:432-433` is
unreachable for any file over 50 MB. The owner's 60 MB test showed exactly this.

The browser then makes it worse. `MapImportTool` has a "too large" message for
a 413 (`apps/web/src/components/canvas-tools/MapImportTool/MapImportTool.tsx:93-95`)
that the server never sends, and a `catch` for anything that throws
(`:115-120`). A server that refuses a body while the browser is still sending
it commonly closes the connection, `fetch` rejects, and the refusal arrives as
"Check your connection and try again." An oversize map reads as a network
fault, and nothing the user can do about their connection will fix it.

GraphQL image uploads have the same shape: `DefaultBodyLimit::max(MAX_UPLOAD_BYTES)`
on `/graphql` (`src/app/src/main.rs:636-639`) against the same 50 MB checked in
the transcoder (`src/server/src/storage/transcode.rs:17`, `:56-62`), so their
"too large" branch is equally out of reach for a file over the ceiling.

### A dropped connection costs the whole file

There is no resume. A connection that drops at 95% of a 90 MB upload starts
again at zero, from the file picker.

### The 18 seconds is mostly not the upload

The owner measured `demo.dd2vtt` (4.2 MB) at **18 s** through the Vite proxy on
the dev stack, with no progress shown, against 5.8 s for a 388 KB map. Moving
4.2 MB over loopback takes milliseconds. The time is spent after the bytes
arrive, in work this spec's protocol does not by itself remove:

- The base64 image is decoded **twice** — once for the background
  (`src/server/src/map_import/image.rs:123-125`) and again for the preview
  (`image.rs:85-87`).
- The image is decoded to pixels **twice** — `transcode_map_background`
  (`transcode.rs:124-126`) and `transcode_scene_preview` (`transcode.rs:333-335`)
  — resized with Lanczos3 (`:151`), and WebP-encoded twice.
- All of that runs **directly inside an `async fn`** (`image.rs:133`), on a
  Tokio worker thread, not in `spawn_blocking`.
- Each storage write mints an STS credential and then writes
  (`src/server/src/storage/rustfs.rs:265-300`).

`demo.dd2vtt` is 4480x2560 at 128 px per cell (35x20 cells), over the 4096 px
texture ceiling (`transcode.rs:323`), so it takes the resize path. The dev
profile already builds `image`, `webp` and `png` at `opt-level = 3`
(`Cargo.toml:83-90`) — the fix `canvas-authoring.spec.ts:518-527` recorded as
the cause of a 28-34 s import — and the owner still measured 18 s. The e2e
budget for this import is 30 s (`canvas-authoring.spec.ts:528`).

This spec therefore treats the 18 s as its own finding: measured stage by
stage first, then removed where it is waste (FR-080 to FR-084).

### The art a 500 MB map would carry is mostly discarded

Every stored background is capped at **4096 px** on its long side
(`transcode.rs:323`, a hardware ceiling explained there). A 500 MB map is
stored as a WebP of a few megabytes. That is correct and stays — tiling is how
larger art will be served, and that is not this spec — but it means the upload
path exists to carry bytes the server will mostly throw away, which bears on
Question 3.

And one limit the owner has not met yet: the `image` crate refuses to decode an
image whose pixel buffer exceeds **512 MiB** (`Limits::default().max_alloc`,
image 0.25), which is roughly 134 megapixels of RGBA. `transcode_map_background`
uses those defaults. A large enough map is refused at decode, after its whole
upload, with the crate's error text. This spec makes that ceiling explicit,
checks it from the image header before a byte of the image is sent, and says it
in words (FR-043, FR-044).

### The storage client cannot reach R2, and cannot reach S3 as written

The server depends on `aws-sdk-s3` (`src/server/Cargo.toml:143`) and uses only
`PutObject`, `GetObject`, `HeadBucket`, `CreateBucket` and one `DeleteObject`.
There are no multipart or presigned calls. More importantly for "works against
R2 and S3":

- **Every read and write first calls STS `AssumeRole`** with an inline policy
  (ADR-039; `rustfs.rs:216-236`, `:265-289`). Cloudflare R2 implements the S3
  multipart operations but documents no STS endpoint and does not implement
  bucket policies (R2 S3 API compatibility page). The storage client as it
  stands cannot write a single object to R2.
- The role ARNs are hard-coded as `arn:aws:iam::000000000000:role/thunderforge-canvas-asset-{reader,writer}`
  (`rustfs.rs:222`, `:276`) — RustFS's placeholder form. Against real S3 they
  name a role that does not exist.
- The scoped write policy allows only `s3:PutObject` (`rustfs.rs:181-192`).
  `AbortMultipartUpload` and `ListMultipartUploadParts` are separate actions.
- The only delete refuses every key outside `feedback/` (`rustfs.rs:327-330`),
  because nothing that might be deduplicated may be deleted
  (`src/server/src/storage/dedupe.rs:24-34`).

The first two are pre-existing, and not this spec's to fix (Question 1); the
last two this spec must change.

### What there is not

- **A plain-image map path.** `MapImportTool` accepts `.dd2vtt` only
  (`MapImportTool.tsx:130`). `uploadCanvasImage` has a `BACKGROUND` kind
  (`src/server/src/graphql/mutations_assets.rs:29-32`), but nothing in the web
  sends it, and the only code that sets `scenes.background_asset_id` from an
  upload is the importer (`map_import/mod.rs:336`). A GM with a PNG and no
  `.dd2vtt` cannot make it a scene's map today.
- **Progress.** The tool shows `Loader label="Uploading map..."`
  (`MapImportTool.tsx:147`) from click to result, including the server's
  processing.
- **Spec 043** ("worker blob store") is unrelated: it is the browser cache's
  OPFS writes in a Web Worker, and it stopped at its measurement gate.

## What exists, and what this spec adds

| Concern | Today | This spec |
|---|---|---|
| Shape of an upload | One request, whole file in server memory | An upload session: declared up front, sent in numbered parts, completed explicitly |
| Size ceiling | 50 MB, compiled in, enforced by a body limit that masks its own message | 500 MB default instance setting, refused from the declared size before any bytes move, and again on bytes received |
| Too large | 400 "error parsing multipart", shown as "check your connection" | A sentence naming the file's size and the instance's ceiling |
| Dropped connection | Start again | The failed part retries alone; a reload continues from the last confirmed part |
| `.dd2vtt` in transit | Base64 inside JSON, +33% | Parsed in the browser; the image as binary parts, the walls, doors, lights and grid as one small declaration |
| Progress | A spinner | Bytes confirmed by the server, then named processing steps, announced |
| Processing | Inside the upload request, on an async worker, duplicate decodes | After completion, off the request, one decode, bounded memory |
| Plain image as a map | Not possible | The same upload, without the declaration |
| Portraits, tokens, pasted art, lore images | GraphQL multipart, 50 MB / 25 MB | Unchanged path; the "too large" message is fixed (FR-060 to FR-062) |
| Abandoned uploads | n/a | Expire and are aborted in storage |

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A large map imports through a proxy that limits requests (Priority: P1)

A Game Master on an instance behind Cloudflare picks a 180 MB `.dd2vtt`. It
imports: the map, its walls, its doors and its lights, exactly as a small map
does today.

**Why this priority**: It is the owner's question. Every hosted instance on
Cloudflare's Free or Pro plan cannot import any map over 100 MB today, and no
setting fixes it.

**Independent Test**: Behind the harness's body-limit proxy set below the file's
size, import a generated map larger than that limit. The scene gets the
background and the walls, doors and lights the file declares, and no request
the proxy saw exceeded its limit.

**Acceptance Scenarios**:

1. **Given** a proxy that refuses any request body over its limit, and a map
   larger than that limit but under the instance's ceiling, **When** a GM
   imports it, **Then** it imports, and every request is under the limit.
2. **Given** the imported scene, **Then** its walls, doors and lights have the
   counts and coordinates the old importer produces for the same file.
3. **Given** `examples/maps/demo.dd2vtt`, **When** imported through the new
   path, **Then** the result is 31 walls, 2 doors and 12 lights — the counts
   `canvas-authoring.spec.ts` asserts today.
4. **Given** a map whose processing takes longer than a proxy's read timeout,
   **Then** the import still completes, because no request stays open while
   the server processes.

---

### User Story 2 - A dropped connection costs one part (Priority: P1)

A Game Master's connection drops two-thirds of the way through a large map.
The upload pauses, says so, and carries on from where it stopped when the
connection returns. If they reload the page, they pick the same file again and
it continues rather than restarting.

**Why this priority**: A 500 MB ceiling without resume is a ceiling nobody on a
home connection reaches.

**Independent Test**: Sever the connection mid-part, restore it, and observe
that the upload completes with no part confirmed before the cut sent again.
Separately, reload mid-upload, re-pick the file, and observe the same.

**Acceptance Scenarios**:

1. **Given** an upload in progress, **When** one part's request fails, **Then**
   that part alone is retried, and no confirmed part is re-sent.
2. **Given** the connection is lost, **Then** the tool says the upload is
   waiting for the connection, keeps the progress it had, and resumes by itself
   when requests succeed again.
3. **Given** a reload mid-upload, **When** the GM opens the import tool on the
   same scene, **Then** it offers to continue the unfinished upload, and
   picking the same file continues from the last confirmed part.
4. **Given** the GM picks a *different* file to continue, **Then** the tool
   says it is not the same file and offers to start it fresh instead.
5. **Given** an upload left unfinished beyond its expiry, **Then** it is no
   longer offered, and its parts are gone from storage.
6. **Given** a GM who cancels, **Then** the upload stops and its parts are
   removed, without waiting for expiry.

---

### User Story 3 - "Too large" says too large (Priority: P1)

A Game Master picks a 612 MB map on an instance with a 500 MB ceiling. Before
any bytes are sent, the tool says: this map is 612 MB, and this server accepts
maps up to 500 MB.

**Why this priority**: The misreport is a defect today, and it is the one the
owner hit.

**Independent Test**: Declare an upload over the ceiling; the refusal arrives
before any part is sent and the words name both sizes. Separately, lower the
ceiling in instance settings and see the new number in the refusal without a
restart.

**Acceptance Scenarios**:

1. **Given** a file over the ceiling, **Then** the refusal names the file's
   size and the ceiling, and does not mention the connection.
2. **Given** a client that declares a size under the ceiling and then sends
   more, **Then** the server refuses the excess, and the tool says the file
   changed or was larger than declared, not that the connection failed.
3. **Given** an image whose pixel dimensions exceed what the server can decode,
   **Then** the refusal says so in pixels, before its bytes are sent.
4. **Given** an operator who lowers the ceiling, **Then** the next declaration
   is judged against the new value, and uploads already started keep the
   ceiling they were accepted under.
5. **Given** a proxy between browser and server that refuses a part as too
   large, **Then** the tool says something between the browser and the server
   refused a piece of the upload as too large, that this is the server
   operator's to fix, and does not say "check your connection".

---

### User Story 4 - Progress that tells the truth (Priority: P2)

A Game Master sees how much of the map has arrived, then what the server is
doing with it — reading the map, preparing the art, placing walls and lights —
and a screen reader hears the same at a pace a person can follow.

**Why this priority**: The 18 s spinner is the owner's second complaint, and
without it a long upload is indistinguishable from a hung one.

**Independent Test**: Import a multi-part map and record the progress values
and the live-region announcements. Progress only rises, reaches 100% of the
bytes only when the server has confirmed all of them, and the processing steps
are named.

**Acceptance Scenarios**:

1. **Given** an upload, **Then** a progress bar shows the share of bytes sent,
   and a separate figure of bytes confirmed never exceeds what the server has
   acknowledged.
2. **Given** all bytes confirmed, **Then** the tool leaves the byte bar and
   shows the named step the server reports, without a percentage it does not
   have.
3. **Given** a screen reader, **Then** it hears when the upload starts, at each
   quarter, when it pauses for the connection, when it resumes, each processing
   step, and the result — and nothing more often than that.
4. **Given** a keyboard user, **Then** Cancel is reachable and named.

---

### User Story 5 - A plain image becomes a scene's map (Priority: P3)

A Game Master with a PNG battlemap and no `.dd2vtt` picks it in the same tool.
It becomes the scene's background at the scene's current grid, with no walls
or lights added.

**Why this priority**: It costs one branch once the upload exists, and a
Game Master who has only an image cannot make a map today.

**Independent Test**: Import a PNG; the scene's background is set, its grid is
unchanged, and it has no new walls, doors or lights.

**Acceptance Scenarios**:

1. **Given** a PNG, JPEG or WebP, **When** imported, **Then** it becomes the
   scene's background, stored as the importer stores one today.
2. **Given** the scene's walls, doors and lights before, **Then** they are
   unchanged after.
3. **Given** a file that is neither a supported image nor a `.dd2vtt`, **Then**
   the tool refuses it by type before uploading.

---

### User Story 6 - Only a Game Master of that scene, and never into a paused world (Priority: P1)

A player cannot start a map upload. A Game Master who loses that role, or whose
world an operator pauses, during a long upload cannot complete it.

**Why this priority**: Principle III. A long-lived upload is a long-lived
authority, and it must not outlast the authority it was granted under.

**Independent Test**: Start an upload as a GM, demote them mid-upload, and
attempt to complete: refused, nothing written. Repeat with a pause.

**Acceptance Scenarios**:

1. **Given** a Player or a non-member, **When** they declare an upload for a
   scene, **Then** it is refused before anything is stored.
2. **Given** a world that is paused, **When** a GM declares an upload, **Then**
   it is refused with the pause code (spec 051).
3. **Given** a GM demoted mid-upload, **When** the upload completes, **Then**
   completion is refused, and no background, wall, door or light is written.
4. **Given** a world paused mid-upload, **When** the upload completes, **Then**
   completion is refused with the pause code, and the upload stays resumable
   until its expiry (Question 2).
5. **Given** another user who learns an upload's identifier, **When** they send
   a part, read its state, complete or cancel it, **Then** each is refused as
   though the upload did not exist.

---

### Edge Cases

- **The file changes on disk mid-upload.** Each part carries a checksum; a part
  whose bytes differ from the one already confirmed at that number is refused
  unless the client is deliberately replacing it, and completion verifies every
  part. A file edited under an upload fails completion, it is not assembled
  from two versions.
- **The same part is sent twice** (a retry whose first attempt did in fact
  arrive). Idempotent: the second is acknowledged without being stored again.
- **Parts arrive out of order or concurrently.** Allowed; order is the part
  number, not arrival.
- **The last part is short.** Expected. Every other part is exactly the part
  size, which R2 requires.
- **A `.dd2vtt` whose declared grid disagrees with its image.** Refused at
  declaration if the image header is readable in the browser and at processing
  in any case, with a message naming both — not imported misaligned.
- **A `.dd2vtt` with no `image` field, or an empty one.** Refused at
  declaration as not a map.
- **A `.dd2vtt` with an image but no walls.** Imported, as today.
- **Two uploads to one scene at once.** The second declaration is refused while
  the first is unfinished, naming the first and offering to continue or cancel
  it. Two concurrent imports to one scene would race to set its background.
- **A scene deleted mid-upload.** Completion is refused and the upload aborted.
- **The server restarts mid-upload.** Upload state is in the database and parts
  are in storage; the client's next part or state read succeeds against the new
  process.
- **The server restarts mid-processing.** Processing is recorded as a job; it
  is either resumed or marked failed with a message, never left "processing"
  forever.
- **Storage is unreachable during a part.** The part fails with a retryable
  status; the client retries with backoff and says the server is having trouble,
  not that the user's connection is.
- **A proxy with a smaller limit than the part size** (nginx's default
  `client_max_body_size` is 1 MB). User Story 3 scenario 5. The operator
  documentation says what part size the instance uses.
- **An old browser tab running the previous client** posts to the retired
  endpoint. It receives a response that the tool renders as "this page is out
  of date, reload it", not a network error (FR-005).
- **A 500 MB file on a phone.** The browser parse reads the file as a stream and
  never holds it as one string; whether a given device can do it is the
  device's, but the tool never needs the whole file in memory at once.

## Requirements *(mandatory)*

### Functional Requirements

**The upload session**

- **FR-001**: A map upload MUST be an upload session with an explicit start,
  numbered parts, an explicit completion, and an explicit cancel. No step MUST
  carry more than one part's bytes in one request.
- **FR-002**: The protocol MUST be this product's own small protocol (Decision
  4), served under `/api/uploads`, authenticated and CSRF-protected as every
  other mutating REST route is. The operations are:

  | Operation | Request | Response |
  |---|---|---|
  | Start | purpose, target, declared byte size, declared type, file name, and for a map its declaration (FR-030) | upload id, part size, part count, expiry, parts already confirmed (empty) |
  | Send part *n* | the part's bytes, its length, its SHA-256 | confirmed, with the checksum stored |
  | Read state | — | status, parts confirmed with sizes and checksums, expiry, processing step or result or refusal |
  | Complete | the ordered list of part checksums | accepted for processing |
  | Cancel | — | cancelled |

- **FR-003**: The **server** MUST choose the part size and return it at start;
  the client MUST use it. Every part but the last MUST be exactly that size.
  The default MUST be **8 MiB**: at least S3's and R2's 5 MiB non-final
  minimum, under Cloudflare's smallest plan limit by more than ten times, a
  500 MB upload in 60 parts against a 10,000-part maximum, and a retry that
  costs at most 8 MiB.
- **FR-004**: A part request MUST be refused if its length is not the expected
  length for its number, if its number is outside the declared count, or if its
  body's SHA-256 does not match the checksum it declares. Refusals MUST carry a
  machine-readable code the client routes on (FR-050).
- **FR-005**: `POST /api/scenes/{scene_id}/import/uvtt` MUST be retired in the
  same change. A request to it MUST receive a response the current client
  renders as "this page is out of date — reload it". One way to import a map,
  not two with different ceilings.

**Idempotency and resume**

- **FR-010**: Sending part *n* with bytes whose checksum equals the part already
  confirmed at *n* MUST succeed without storing it again.
- **FR-011**: Sending part *n* with a different checksum from the confirmed
  part MUST be refused with a code meaning "a different part is already here",
  unless the request explicitly says it replaces the part. Completion MUST
  assemble exactly the parts whose checksums the completion request lists, in
  order, and MUST refuse if any differs from what is stored.
- **FR-012**: The client MUST retry a failed part on its own, with exponential
  backoff and jitter, up to a stated number of attempts (default 5, capped at
  30 s between attempts) before reporting the upload as waiting. Other parts
  MUST continue meanwhile.
- **FR-013**: The client MUST send at most **4** parts concurrently.
- **FR-014**: When requests are failing for connectivity, the client MUST pause
  the whole upload, say so, keep the progress it has, and resume by itself when
  a state read succeeds — checking at most every 5 s while waiting.
- **FR-015**: The client MUST remember an unfinished upload (its id, scene, file
  name, size, last-modified time and first part's checksum) in the browser, so
  that after a reload the import tool on the same scene offers to continue it.
- **FR-016**: Continuing after a reload MUST ask the Game Master to pick the file
  again — a browser does not keep a file across a reload — and MUST refuse a
  file whose size, or whose first part's checksum, differs from the remembered
  one, offering to start that file fresh instead.
- **FR-017**: On continuing, the client MUST read the upload's state from the
  server and send only the parts not confirmed there. The server's list is the
  truth; the browser's memory is only how the upload is found.

**Expiry and cleanup**

- **FR-020**: An upload MUST expire **24 hours** after its last confirmed part
  or its start, whichever is later. An expired upload MUST refuse parts and
  completion with a code meaning expired.
- **FR-021**: A sweep MUST abort expired and cancelled uploads in storage and
  mark them so, at least every 15 minutes, bounded per pass, on the same kind of
  schedule the feedback retention sweep uses (`src/server/src/feedback/schedule.rs`).
- **FR-022**: The sweep MUST also abort any multipart upload in storage under
  the staging prefix that no live upload record names — the case of a process
  that died between creating one and recording it.
- **FR-023**: Staged bytes MUST live under their own storage prefix, never
  deduplicated and never referenced by an asset row, so that removing them
  needs no reference counting (the rule in `storage/dedupe.rs:24-34`). The
  delete guard in `rustfs.rs:327-330` MUST admit this prefix as it admits
  `feedback/`, for the same stated reason, and nothing else.
- **FR-024**: Once a map is processed — imported or refused — its staged
  original MUST be removed. The stored background is the transcoded rendition,
  as today (Question 3).
- **FR-025**: A user MUST hold at most **3** unfinished uploads at once; a
  fourth declaration MUST be refused naming the ones they have.
- **FR-026**: The operator documentation MUST recommend a storage lifecycle rule
  that aborts incomplete multipart uploads after 7 days, as a backstop to the
  sweep. R2 applies one by default; S3 does not.

**Authorisation**

- **FR-030**: Start MUST refuse unless the caller may import a map onto the
  target scene now: the importer's rule, the Owner or a Game Master of the
  scene's world (`is_dm_of_scene`, `map_import/mod.rs:144`), an account in good
  standing, and a world that is not paused (spec 051's `refuse_if_paused`,
  `:154`).
- **FR-031**: Every part, state read, completion and cancel MUST be refused
  unless the caller is the user who started the upload. A refusal to anyone else
  MUST be indistinguishable from an upload that does not exist.
- **FR-032**: Completion MUST re-run FR-030 in full. If it fails, nothing MUST be
  written to the scene.
- **FR-033**: The check that authorises writing walls, doors, lights and the
  background MUST run inside the transaction that writes them. Today the pause
  check (`:154`) and the write transaction (`:240`) are separate, with the
  background transcode between them; a pause that lands in that window is not
  honoured.
- **FR-034**: Neither the start nor any later step MUST let a client choose a
  storage key. Keys MUST be derived on the server from the upload id, as
  `object_key` derives them today (`rustfs.rs:67-83`).

**The map declaration and what the server re-validates**

- **FR-040**: For a `.dd2vtt`, the browser MUST parse the file and send, at
  start, a declaration holding every top-level field of the file except
  `image`, **as the file states them** — `format`, `resolution`,
  `line_of_sight`, `objects_line_of_sight`, `portals`, `lights`,
  `environment`. The browser MUST NOT convert coordinates, build walls or
  choose a grid; the server's existing conversion (`map_import/geometry.rs`,
  `ambient.rs`, `alignment.rs`) stays the only one.
- **FR-041**: The browser MUST then upload the image as its decoded binary
  bytes, not as base64.
- **FR-042**: The server MUST validate the declaration exactly as it validates a
  whole file today (`parse_uvtt`, `map_import/parse.rs:11-39`), plus: every
  number finite; the declaration at most **4 MiB**; at most 100,000 wall
  polygon points, 10,000 portals and 10,000 lights; every portal with exactly
  two bounds. A declaration that fails MUST be refused at start, before any
  bytes are sent.
- **FR-043**: The server MUST read the image's type and pixel dimensions from
  its own header once the bytes are assembled, MUST refuse a type that is not
  PNG, JPEG or WebP, and MUST refuse dimensions whose decoded buffer would
  exceed the decode ceiling (FR-085) — before decoding.
- **FR-044**: The browser MUST read the same header from the first part before
  sending anything, and refuse early with the same words. This is a courtesy to
  the user; FR-043 is the rule.
- **FR-045**: The server MUST refuse a map whose image dimensions disagree with
  `resolution.map_size × resolution.pixels_per_grid` by more than one cell on
  either axis, with a message naming both, rather than import it misaligned.
- **FR-046**: The declared byte size MUST equal the sum of the parts; the
  declared type MUST match the sniffed type. Either mismatch refuses completion.

**Where the browser parses**

- **FR-047**: The `.dd2vtt` parse MUST run in a Web Worker, for every file size —
  one code path, not a small-file shortcut.
- **FR-048**: The parse MUST read the file as a stream and MUST NOT hold the
  file's text, or the image's base64, as a single string. It MUST decode the
  image's base64 incrementally into binary chunks, and the parts sent MUST be
  slices of those decoded bytes. The worker's working memory, apart from the
  decoded image the browser holds as a `Blob`, MUST stay under **16 MiB** for any
  input.
- **FR-049**: A plain PNG, JPEG or WebP MUST skip the parse and upload as it is,
  with purpose "map image, no geometry" (User Story 5): the scene's background is
  set and its grid, walls, doors and lights are untouched.

**Limits and messages**

- **FR-050**: The ceiling MUST be an instance setting, `uploads.map_max_bytes`,
  declared in the settings registry (`src/server/src/settings/registry.rs`) with
  an environment variable `THUNDERFORGE_MAP_UPLOAD_MAX_BYTES`, default
  **500 MB** (524,288,000 bytes), resolved by the registry's one precedence rule
  — environment, then the instance's stored value, then the default
  (`settings/resolver.rs:1-2`).
- **FR-051**: The registry has no numeric-size kind (`registry.rs:104-116`); it
  MUST gain one, validated as a whole number of bytes within a range, and the
  admin surface MUST render and accept it in megabytes.
- **FR-052**: The setting MUST accept values from **1 MB** up to **500 MB**. An
  operator may lower the ceiling, not raise it past the owner's decision; raising
  the maximum is a later change with its own memory bound (FR-085).
- **FR-053**: Start MUST refuse a declared size over the ceiling, and the
  refusal MUST carry the declared size and the ceiling as numbers. An upload
  keeps the ceiling it was accepted under.
- **FR-054**: The server MUST refuse, on bytes received, any part that would
  take the upload past its declared size.
- **FR-055**: The client MUST NOT say "check your connection" for any response
  the server or a proxy actually sent. It MUST say it only when requests fail
  without a response **and** a state read also fails. Each refusal code MUST map
  to its own sentence:

  | Situation | What the Game Master reads |
  |---|---|
  | Declared too large | "This map is 612 MB. This server accepts maps up to 500 MB." |
  | Image too many pixels | "This map's image is 16,000 × 12,000 pixels. This server can read maps up to 134 megapixels." |
  | Grid disagrees with image | "This map says it is 35 × 20 squares at 128 pixels, but its image is 3,000 × 2,000 pixels." |
  | Not a map file | "This isn't a map file this server can read. Choose a .dd2vtt, PNG, JPEG or WebP." |
  | A proxy returned 413 for a part | "Something between you and this server refused part of the upload as too large. The server's operator needs to allow requests of at least 8 MB." |
  | Paused | spec 051's pause notice |
  | Not allowed | "You don't have permission to import a map into this scene." |
  | Expired | "This upload was left unfinished too long and has been cleared. Start it again." |
  | The file changed | "This file changed while it was uploading. Start it again." |
  | Waiting | "Waiting for the connection. The upload will carry on when it's back." |
  | Server trouble (5xx) | "The server is having trouble storing the upload. It will keep trying." |

**Other uploads**

- **FR-060**: Portrait, token, pasted, actor, lore and feedback image uploads
  MUST stay on their current GraphQL multipart path in this spec (Decision 5).
- **FR-061**: Their "too large" branch MUST become reachable: the route's body
  limit MUST sit above the handler's ceiling by enough to admit the multipart
  envelope, so an oversize image gets the handler's own refusal
  (`transcode.rs:56-62`) rather than a parse error.
- **FR-062**: Their clients MUST check the file's size against the ceiling
  before sending, and MUST NOT report a refusal as a connection fault (FR-055's
  rule).

**Progress**

- **FR-070**: The tool MUST show a determinate progress bar of bytes sent while
  uploading, with `role="progressbar"` and its current value, and the text
  "*n* MB of *m* MB".
- **FR-071**: Bytes sent MUST come from the browser's upload progress events, and
  MUST fall back to the last confirmed part when a part is retried, so the bar
  may pause but never runs backwards by more than one part.
- **FR-072**: After completion is accepted, the tool MUST show the step the
  server reports — *Checking the file*, *Preparing the art*, *Placing walls,
  doors and lights* — as named, indeterminate steps. It MUST NOT show a
  percentage for processing.
- **FR-073**: The client MUST learn the processing step by reading state, at most
  once a second, and MUST stop when a result or refusal arrives.
- **FR-074**: A polite live region MUST announce: start; 25, 50 and 75 percent;
  waiting; resuming; each processing step; and the result or refusal. It MUST
  NOT announce more than once every 5 seconds except for the result.
- **FR-075**: The result MUST keep today's summary — walls, doors, lights,
  skipped polygons (`MapImportTool.tsx:179-196`) — and add the art's stored size
  when it was reduced ("the art was reduced to 4,096 × 2,341 pixels for
  display").
- **FR-076**: Cancel MUST be a visible, named button for the whole of the upload
  and processing-queued states, and absent once processing has begun writing.

**Storage**

- **FR-080**: Parts MUST be written to storage with S3 multipart upload —
  `CreateMultipartUpload` at start, `UploadPart` per part,
  `CompleteMultipartUpload` at completion, `AbortMultipartUpload` on cancel and
  expiry — through the existing `aws-sdk-s3` client and the existing credential
  path, extended rather than replaced.
- **FR-081**: The scoped credential for an upload MUST allow exactly the
  multipart actions on exactly that upload's staging key — `s3:PutObject`,
  `s3:AbortMultipartUpload`, `s3:ListMultipartUploadParts` — as ADR-039 scoped
  `PutObject` to one key, and MUST be proven against RustFS the way ADR-039's
  was: allowed on its key, denied on any other.
- **FR-082**: Processing MUST stream the assembled object from storage to a
  temporary file and decode from the file. The assembled object MUST NOT be held
  in memory as one buffer.
- **FR-083**: Processing MUST decode the image **once** and produce the
  background and the preview from that one decode.
- **FR-084**: Decode, resize and encode MUST run on blocking threads
  (`spawn_blocking` or a dedicated pool), not on an async worker.
- **FR-085**: The decode ceiling MUST be stated as a number in code — **512 MiB**
  of decoded pixels, the `image` crate's current default made explicit — and
  map processing MUST run **one at a time** per server process by default, so
  that the bound in SC-008 holds under concurrent imports.
- **FR-086**: The server MUST bound the memory used to receive parts: at most
  one part buffered per in-flight part request, and at most **16** part requests
  in flight across the process; a request beyond that MUST get **503** with
  `Retry-After`, which the client treats as a retry, not a failure.
- **FR-087**: Before any change to the processing code, the 18 s import of
  `demo.dd2vtt` MUST be measured stage by stage — request receipt, JSON parse,
  base64 decode, image decode, resize, encode (background and preview), each
  storage call, the database transaction — on the owner's dev stack and on a
  release build, and the table recorded in this spec's research. FR-083 and
  FR-084 are the expected fixes; the measurement decides whether they are
  enough.
- **FR-088**: The work MUST be proven against RustFS. It MUST be written so that
  the storage endpoint and the credential source are the only differences
  between RustFS, R2 and S3, and MUST NOT assume behaviour R2 lacks: all
  non-final parts equal size, no bucket policies, no conditional
  `UploadPartCopy`.

**Proof**

- **FR-090**: The e2e harness MUST run a **body-limit proxy** — a small Node
  process started per shard beside the GitHub and OAuth stubs
  (`scripts/e2e-parallel.mjs`) — in front of the web server for the upload
  lane. It MUST refuse with 413 any request whose body exceeds a configured
  limit, counting streamed bytes and not trusting `Content-Length`, and close
  the connection as a proxy does.
- **FR-091**: The proxy MUST expose a control that severs the next in-flight
  request after a given number of body bytes, so a test can kill a part
  mid-transfer at the network layer rather than by aborting it in Playwright.
- **FR-092**: The e2e MUST generate its large maps at run time — a `.dd2vtt`
  wrapping an incompressible image of the needed size, with known walls, doors
  and lights — because `examples/maps` is dev fixtures that are not
  redistributable and none exceeds 8 MB.
- **FR-093**: The e2e MUST prove, through the real import tool:
  1. a map larger than one part and larger than the proxy's limit imports, and
     no request exceeded the limit (proxy's own log);
  2. a connection severed mid-part resumes, and no part confirmed before the cut
     is sent again (proxy's log of part numbers);
  3. a reload mid-upload, with the file picked again, continues from the last
     confirmed part;
  4. a declaration over the ceiling is refused before any part is sent, with
     FR-055's sentence, and without the word "connection";
  5. the imported scene's walls, doors and lights match the generated file's
     counts and coordinates, and `demo.dd2vtt` still yields 31, 2 and 12;
  6. a Player's declaration is refused, and a GM demoted mid-upload cannot
     complete;
  7. the live region's announcements, in order.
- **FR-094**: The default e2e lane MUST use a proxy limit of **24 MiB** and a
  generated map of about **64 MiB**, so the behaviour is proven in seconds. One
  test tagged for the long lane MUST use Cloudflare's actual **100 MB** limit and
  a map of about **130 MB**, so that the number the owner cares about is tested
  rather than assumed.
- **FR-095**: A Rust integration test MUST prove the sweep aborts an expired
  upload in RustFS and that `ListMultipartUploads` under the staging prefix is
  empty afterwards.

### Key Entities

- **Upload**: one file on its way to storage — who started it, for what purpose
  and target, the declared size and type, the ceiling it was accepted under,
  its part size and count, its storage multipart id, its status (receiving,
  processing, done, refused, cancelled, expired), its expiry, and for a map its
  declaration.
- **Part**: one numbered, fixed-size slice of an upload — its size, its SHA-256,
  its storage ETag, when it was confirmed.
- **Map declaration**: the `.dd2vtt`'s fields other than the image, as the file
  states them.
- **Processing step**: what the server is doing with a completed upload, named
  for a person.
- **Map upload ceiling**: the instance setting.
- **Staging prefix**: where parts and assembled originals live until processed,
  deletable because nothing references them.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A map larger than the proxy limit imports behind the body-limit
  proxy in **100%** of runs, with **0** requests over the limit — at 24 MiB in
  the default lane and at 100 MB in the long lane.
- **SC-002**: After a mid-part cut, the upload completes with **0** confirmed
  parts re-sent, and the bytes re-sent are at most **one part size times the
  concurrency** (32 MiB).
- **SC-003**: An over-ceiling declaration is refused with **0** part requests
  made, within **1 s** of choosing the file for a `.dd2vtt` under 1 GB, and the
  message contains both sizes and never the word "connection".
- **SC-004**: `demo.dd2vtt` imports, from choosing the file to "Map imported",
  in **≤ 6 s** on the owner's dev stack through the Vite proxy (measured 18 s),
  and **≤ 3 s** against a release backend — the e2e asserting ≤ 10 s so it
  cannot flake on a loaded shard, and logging the number.
- **SC-005**: `demo.dd2vtt` imported through the new path yields walls, doors
  and lights whose count **and** coordinates equal the old importer's output for
  the same file — captured as a golden before FR-005 retires it.
- **SC-006**: Upload overhead: sending a generated 256 MiB map through the local
  stack takes no more than **1.25×** the time of a single `PUT` of the same bytes
  through the same proxy to the same storage.
- **SC-007**: While receiving uploads, the server's resident memory rises by at
  most **160 MiB** above idle, whatever the size or number of uploads (16 parts
  × 8 MiB plus overhead).
- **SC-008**: While processing the largest image the ceiling admits, resident
  memory rises by at most **1 GiB** above idle, and a second concurrent import
  waits rather than adding to it.
- **SC-009**: **0** expired uploads remain in storage 30 minutes after expiry,
  and **0** orphaned multipart uploads under the staging prefix after a sweep.
- **SC-010**: A screen-reader user hears **every** state change in FR-074 and
  **no** more than one announcement per 5 s while bytes are moving; axe reports
  **0** violations on the tool in every state.
- **SC-011**: **0** paths remain on which a refusal the server sent is shown as
  "check your connection" — for map import and for every GraphQL image upload.

## Assumptions

- **RustFS supports S3 multipart** — create, upload part, list parts, complete,
  abort and list multipart uploads — and applies STS session policies to them
  as it applies them to `PutObject`. It is S3-compatible and ADR-039 proved its
  STS; FR-081 proves the multipart actions before anything is built on them.
- **The server remains the only thing that writes to storage** (ADR-039). Bytes
  pass through it. That is what makes this work behind `cloudflared`, where a
  storage endpoint is usually not reachable from the browser.
- **Cloudflare counts the limit per request.** Its 413 page recommends chunking
  for exactly that reason, and independent reports of chunked uploads through
  tunnels agree. The long-lane e2e (FR-094) proves the behaviour against a proxy
  that enforces the same rule; it does not prove Cloudflare, and a manual check
  through `make dev-tunnel` is part of acceptance.
- **A file picked in a browser does not survive a reload.** Resume after a
  reload therefore asks for the file again (FR-016). Keeping a handle would need
  the File System Access API, which is Chromium-only.
- **Browsers can hold a decoded image of up to 500 MB as a `Blob`** without
  holding it in the JavaScript heap; engines back large blobs with disk. The
  worker never builds it as an `ArrayBuffer` in one piece.
- **`fetch` has no portable upload progress**, so part requests use
  `XMLHttpRequest`, whose upload progress events every supported browser fires.
- **The 18 s is dominated by processing**, on the evidence in "The problem".
  FR-087 is there because that is an inference, not a measurement.
- **The existing geometry conversion is correct.** This spec moves where the
  fields come from, not what is done with them.

## Out of Scope

- **Presigned direct-to-storage uploads.** A later phase (Decision 1), for R2 or
  S3 with a publicly reachable endpoint, where the browser sends part *n* to a
  storage URL the server signed instead of to the server. This spec leaves room
  for it: parts are numbered, fixed-size and checksummed, which is S3's own
  shape, so the later phase changes where a part's `PUT` goes and adds nothing
  to the start, state, complete or cancel steps. It is named here so that it is
  not mistaken for dropped.
- **Moving portrait, token, pasted, actor, lore and feedback images** onto the
  chunked path (Decision 5). Their message is fixed here (FR-060 to FR-062).
- **Making the storage client work against R2 or real S3 at all** — the STS
  dependency and the hard-coded role ARNs (Question 1).
- **Serving art larger than 4096 px.** Tiling, not this spec.
- **Raising the ceiling above 500 MB.**
- **Content collections and pack imports**, which have their own import path.
- **Resumable downloads.**
- **Spec 043**, the browser cache's worker blob store.

## Dependencies

- **Spec 001**: the importer, the `.dd2vtt` parser and geometry conversion, and
  its e2e counts.
- **Spec 002 / ADR-039**: storage through the server with single-object-scoped
  STS credentials, which FR-081 extends to multipart.
- **Spec 022**: the scene preview, which FR-083 produces from the same decode.
- **Spec 028**: the content hash of the stored bytes, which processing keeps
  computing on the stored rendition, not the upload.
- **Spec 051**: the pause gate, at start and at completion (FR-030, FR-032).
- **ADR-088 / the settings registry** (spec 040): the ceiling setting and its
  precedence.
- **Spec 037**: the feedback retention sweep, whose schedule and delete
  exception FR-021 and FR-023 follow.
- **ADR-104 (proposed with this spec)**: large uploads arrive in numbered parts
  through the server. This changes how bytes reach storage, and ADR-039 is the
  decision it extends.

## Decisions (owner, 2026-09-16)

1. **Chunked first, presigned later.** Chunked, resumable uploads through the
   server are the path that always works, including behind `cloudflared`.
   Presigned direct-to-storage multipart is a later optimisation for R2 or S3
   with a publicly reachable endpoint; this spec leaves room for it without
   building it.

2. **500 MB ceiling, operator-configurable.** The default is 500 MB per map or
   scene upload, as an instance setting in the settings registry, following its
   environment-override pattern, so an operator can lower it.

3. **Read the `.dd2vtt` in the browser.** The map image is uploaded as binary,
   not base64, and walls, doors, lights and grid go as a small separate
   request. The server still validates everything it receives and never trusts
   the client's parse (Principle III).

The following are this spec's own, recorded as decisions because each has a
clear answer:

4. **A small protocol of our own, not tus.** tus (resumable upload protocol
   1.0) was considered and rejected, for four reasons. Its core is an
   offset-append stream, while S3 and R2 multipart are numbered parts with a
   5 MiB minimum and — on R2 — equal sizes, so a tus server over S3 buffers and
   re-slices what the client could have sent already sliced. Parallel parts need
   tus's concatenation extension, which is a second upload per part. Its upload
   metadata travels in a base64 header, which cannot carry a map's walls under
   Cloudflare's 128 KB header limit. And the Rust servers for it (`fileloft`,
   `tus_axum`) bring their own S3 stores and credentials, which would bypass
   ADR-039's scoped writes. Numbered parts are the shape presigned upload will
   need anyway (Decision 1). The cost is a client of our own instead of
   `tus-js-client`, which the e2e in FR-093 exists to hold to account.

5. **Map import moves now; other images later.** Maps are the only upload that
   reaches Cloudflare's limit — the next largest ceiling is 50 MB and every
   other image is stored at 4096 px or less. Moving the others is a change of
   client, not protocol, and it can follow when there is a reason.

6. **Processing is not part of the upload request.** Completion is accepted and
   processed afterwards, and progress reads the step. A proxy read timeout of
   125 s cannot fail an import whose bytes have arrived.

7. **The retired endpoint goes.** One import path, with one ceiling and one set
   of messages (FR-005).

## Questions for the owner

1. **Q1 — Does this spec also make storage work on R2?**
   *(recommendation: B)*

   Today every storage read and write starts with STS `AssumeRole`
   (`rustfs.rs:216-236`, `:265-289`), which R2 does not offer, against a role
   ARN that exists only in RustFS (`:222`, `:276`). The upload protocol does not
   make this worse, but "works against R2" is not true of anything that stores
   bytes until it is fixed.

   | Option | Answer | Implications |
   |---|---|---|
   | A | Yes, in this spec | One spec reaches a hosted R2 instance end to end. Adds a credential mode for stores without STS, an amendment to ADR-039, and R2 in the test matrix — roughly doubling the storage work, and holding the Cloudflare fix behind it. |
   | **B** | **No: a separate, small spec amends ADR-039 first or alongside** | This spec ships the protocol proven on RustFS, written against one credential seam (FR-088). The owner's hosted instance needs both before it can store to R2; if the hosted instance stores to RustFS behind the tunnel, it needs only this one. |
   | C | Not now | Leaves an instance behind Cloudflare with RustFS as its only store. |

2. **Q2 — Does a pause that lands mid-upload keep the bytes?**
   *(recommendation: A)*

   | Option | Answer | Implications |
   |---|---|---|
   | **A** | **Parts are still accepted; completion is refused while paused; the upload survives until expiry** | A staged part changes nothing anyone sees, so accepting it is not play. A GM mid-way through 400 MB does not lose it to a pause that lifts in ten minutes. Spec 051's rule — nothing in the world changes — holds at completion, where the world would change. |
   | B | A pause refuses parts too | Stricter reading of "paused". The upload stalls, and expires if the pause outlasts 24 hours. |
   | C | A pause cancels unfinished uploads in that world | Simplest to reason about; costs the GM the upload for every pause. |

3. **Q3 — Keep the original after import?** *(recommendation: A)*

   The stored background is at most 4096 px; a 500 MB original is mostly detail
   nobody is shown.

   | Option | Answer | Implications |
   |---|---|---|
   | **A** | **No: remove it once processed (FR-024)** | Storage holds what is served. A future tiling spec asks the GM to import again — which is also how a grid mismatch is fixed today. |
   | B | Keep it, deduplicated by content hash | Tiling could use art already on the server. Up to 500 MB per map kept indefinitely, and a new referenced object class that needs reference counting before anything may delete it (`dedupe.rs:24-34`). |
   | C | Keep it for 30 days | Covers "I imported the wrong grid" without re-uploading; adds a retention sweep and a second prefix rule. |

4. **Q4 — Should the browser shrink the art before sending it?**
   *(recommendation: B)*

   Since the server stores at most 4096 px, the browser could resize first and
   send a few megabytes instead of hundreds.

   | Option | Answer | Implications |
   |---|---|---|
   | A | Yes, always | The fastest upload by far. Browser resampling differs from the server's Lanczos3, the server's grid-exact cell sizing (`transcode.rs:177-192`) must be reproduced in the browser, and decoding a 134 MP image in a tab takes over 500 MB of memory. Decision 3 says upload the image. |
   | **B** | **No, not in this spec** | Decision 3 as written; one resizer, on the server. Revisit with tiling, when whether to keep the full image is decided (Q3). |
   | C | Only above a size, say 200 MB | Two pipelines to keep in step, with the misalignment bug `map_import/mod.rs:181-188` records waiting in the seam. |
