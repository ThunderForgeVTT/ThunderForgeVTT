# Phase 0 Research: Hand-Drawn Authoring & Per-Campaign Asset Storage

## 1. Scope of "hand-drawn authoring" work (User Stories 1 & 2)

**Decision**: This half of the feature is primarily an **e2e test-coverage gap**, not new
engine functionality. `WallPlugin`/`ShapePlugin` (`src/engine/src/plugins/wall.rs`,
`shape.rs`) and their systems (`src/engine/src/systems/wall.rs`, `shape.rs`) already
implement click-drag wall creation, door toggle (`O` key), delete, undo, and all five
shape tool modes (freehand/rect/ellipse/line/text) with GM-gating via `IsGameMaster`.
Real-time propagation already emits `create_wall`/`create_shape`/`delete_shape` events.

**Rationale**: `specs/001-bevy-canvas-authoring/tasks.md` T067 explicitly documents this:
Scenario 2 (import) e2e coverage exists and passes; Scenario 1 (hand-drawn wall +
cross-session vision-occlusion) and Scenario 4 (hand-drawn shapes + GM/player visibility)
are open only because they need simulated canvas mouse interaction plus a second browser
context in Playwright — not because the underlying feature is missing.

**Alternatives considered**: Treating US1/US2 as net-new engine work was rejected — it
would duplicate already-shipped, already-tested-at-unit-level systems. Implementation
work here is: (a) verify the two known gaps flagged in the code (ellipse renders as a
rect placeholder — `systems/shape.rs`; text has no in-canvas entry system yet —
`systems/shape.rs:12-13`) don't silently fail the acceptance scenarios, closing them if
they do, and (b) writing the Playwright coverage T067 left open.

## 2. Object storage backend: RustFS

**Decision**: RustFS, self-hosted, S3-compatible object storage, run as a new service in
`compose.yml` alongside the existing `postgres` service.

**Rationale**: Specified directly in the feature input. RustFS speaks the S3 API
(compatible with standard S3 SDKs and STS `AssumeRole`-style temporary credentials),
which lets the server mint short-lived, path-scoped credentials per FR-017 without
building a bespoke auth protocol, and lets asset objects be addressed the same way the
existing filesystem-backed background-image path is addressed today (opaque path string
stored in Postgres).

**Alternatives considered**: MinIO — also self-hosted S3-compatible, more mature/battle
tested — was considered but rejected because the feature input explicitly names RustFS;
no functional requirement in spec.md depends on a MinIO-specific capability, so this is
a directed choice, not a technical necessity, and is recorded here rather than
re-litigated.

## 3. Short-lived scoped credentials (FR-017)

**Decision**: Server acts as an STS broker, but the minted credential is held and used
**server-side only** — it is never returned to any GraphQL client. On each authorized
asset-write request (`uploadCanvasImage`, and the migrated map-import background-image
path), the server calls RustFS's STS-compatible `AssumeRole` endpoint (via the
`aws-sdk-sts` + `aws-sdk-s3` crates, S3-compatible endpoint override) to mint a
credential scoped by an inline session policy to `PutObject`/`GetObject` on exactly the
one object key being written (`{owner_user_id}/{world_id}/{scene_id}/{asset_id}.webp`),
with a short TTL (target: 15 minutes), uses that credential itself to perform the write
to RustFS, then discards it. This aligns with and is superseded in detail by §4's
"transcode server-side, synchronously" decision: the client already sends original
bytes to Axum over the existing multipart path (reused from `map_import.rs`), so there
is no separate client→RustFS leg for a credential to serve.

**Rationale**: Minting a real, single-object-scoped, short-TTL credential per write
(rather than the server just using one long-lived internal service credential) keeps
FR-016's "reject before any object is created" gate and FR-017's "short-lived,
per-request credential scoped only to that user's permitted campaign paths" true in the
strongest available sense: even a compromised server process only ever holds a
credential good for one object key for a few minutes, not a durable root key. It also
means the *client* never handles any RustFS credential at all — a strictly narrower
attack surface than a client-facing scoped credential would be, while still being
verifiable per-request (quickstart.md Scenario 4 step 6 inspects the credential the
server used, not one it handed out).

**Alternatives considered**: (a) Client-facing scoped credential returned by a
`requestUploadCredential`-style mutation, with the client PUTing directly to RustFS —
initially the working assumption, but rejected on reflection: it would require a
*second* code path re-implementing FR-013's size/format enforcement client-side (or
trusting the client), and provides no benefit over the server-side write once §4 already
requires the server to decode and transcode the bytes in-process — the client-PUT leg
would be pure unused API surface. (b) One long-lived internal service credential reused
across all writes — rejected, contradicts "short-lived" and "per-request" in
FR-017/SC-007. (c) Server-side proxy upload using RustFS's permanent root credential —
rejected outright by FR-017's literal text; per-write STS minting is what makes the
proxy-style write compliant.

## 4. WebP transcoding (FR-012)

**Decision**: Transcode server-side, synchronously, in the same request that mints the
write credential and records the asset row — decode the client-uploaded bytes (still
routed through Axum for this step, capped at the existing 50MB `MAX_UPLOAD_BYTES`
ceiling reused from `map_import.rs`), re-encode to WebP using the `image` crate (`webp`
feature), then hand the transcoded bytes to RustFS using a short-lived credential minted
for that single write.

**Rationale**: FR-013 requires rejecting oversized images "before any partial/corrupt
asset is persisted" — doing the size check and transcode server-side before the RustFS
write, rather than trusting a client-side transcode, is the only way to guarantee that.
It also keeps one code path (Axum handler) authoritative for both the existing
map-import background image and new pasted images, per FR-018 ("exactly one asset
storage mechanism").

**Alternatives considered**: Client-side transcode-then-direct-PUT (skip the server
round-trip entirely) — rejected because it can't enforce FR-013's size/format guarantee
server-side and would require duplicating the transcode logic in TypeScript.

## 5. Path convention

**Decision**: `{owner_user_id}/{world_id}/{scene_id}/{asset_id}.webp`. Per §3's revised
decision, each write mints its own credential scoped to that one exact key, not a
world-level prefix — since the credential is minted and consumed server-side in the
same request (never handed to a client to reuse across multiple writes), there is no
benefit to widening the policy's scope beyond the single object being written, and
narrowing it to one key is strictly tighter for FR-017.

**Rationale**: Matches spec.md's explicit path description ("per owning user then per
campaign (world) then per scene") and Assumption 1 (no new "organization" entity above
users). The path itself still carries the full user→world→scene hierarchy for
readability/debugging and for the read side (`canvasImageAssetsForScene`'s membership
check), even though the write-credential's policy only ever needs to name one key at a
time.

## 6. Local dev provisioning (FR-020, SC-008)

**Decision**: Add a `rustfs` service to the existing root `compose.yml` (today it
defines only `postgres`), with a companion one-shot init step (either a RustFS
bootstrap container command or a server-startup bootstrap routine) that creates the
bucket and root credentials on first run, sourced from `.env` alongside the existing
`DATABASE_URL` pattern (`dotenvy::dotenv()` in `src/server/src/main.rs`).

**Rationale**: `docker compose up` is already the single provisioning command for this
project (there is exactly one `compose.yml`, no docker-compose per-service split);
adding RustFS as a second service in the same file satisfies "no additional manual
configuration step" (FR-020) without introducing a second provisioning tool.

**Alternatives considered**: A separate `compose.storage.yml` — rejected, adds a second
command the constitution's "single provisioning command" framing doesn't call for.

## 7. Authorization model for asset writes (FR-015, FR-016)

**Decision**: New shared guard function (e.g. `require_world_member(user_id, world_id)`
in a new or existing auth-helpers module) that checks `world_members` (owner row or
accepted-invite row) — the first consumer of `world_members` for authorization outside
`mutations_invites.rs`. Existing wall/shape/token mutations check `scenes::owner_id`
directly and are out of scope to refactor; the new asset-write path introduces the
`world_members`-based check net-new, as the spec requires (FR-015 explicitly says reuse
the *existing model*, not necessarily the existing call sites).

**Rationale**: Constitution Principle III requires ownership/authorization enforced
server-side at the data boundary; a single shared guard function (rather than inlining
the query at each new call site) keeps that enforcement consistent and testable in one
place, matching Principle II's "narrow public surface" spirit even outside the engine
crate.

**Alternatives considered**: Extending `scenes::owner_id`-only checks to cover invited
members too — rejected as broader-scoped than this feature; spec.md's Key Entities
section calls out `world_members` specifically as "reused, unchanged," so the guard
reads that table rather than changing scene-ownership semantics.
