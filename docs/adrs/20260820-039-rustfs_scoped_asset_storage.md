# ADR-039: RustFS Object Storage with Server-Held, Single-Object-Scoped STS Credentials

**Date:** 2026-08-20
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Extends:** ADR-037 (Native Bevy Canvas Authoring Supersedes Wrapped tldraw)

---

## Problem Statement

Spec 002 adds paste-to-canvas image assets and requires migrating the
existing map-import background-image path (`src/server/src/map_import.rs`'s
`save_background_image`, which writes directly to the local filesystem
under `Directories::asset_directory`) onto one shared, per-campaign-scoped
storage mechanism, so that no code path can read or write another
campaign's assets and no client-facing operation ever handles a durable,
broadly-scoped storage credential.

## Decision

Introduce **RustFS** (self-hosted, S3-compatible object storage,
`rustfs/rustfs:1.0.0-rc.2`) as a new service in `compose.yml`, alongside
the existing `postgres` service, as the single storage backend for both
newly-pasted canvas images and migrated map-import background images
(`canvas_image_assets` table, one row per stored object).

Every write goes through `src/server/src/storage/rustfs.rs::write_object`,
which:

1. Calls RustFS's STS `AssumeRole` endpoint (via `aws-sdk-sts`) with an
   **inline session policy** scoped to `s3:PutObject` on exactly the one
   object key being written (`{owner_user_id}/{world_id}/{scene_id}/{asset_id}.webp`),
   TTL 900s (15 minutes).
2. Uses the minted temporary credential (via `aws-sdk-s3`, endpoint
   overridden to RustFS) to perform that one `PutObject`.
3. Discards the credential immediately after. It is **never returned to
   any GraphQL client** — the client sends original image bytes to the
   server over the existing multipart path (mirroring
   `map_import.rs`'s handler), the server transcodes to WebP and performs
   the write itself.

This was verified against a real running RustFS instance before being
relied upon here (not assumed from documentation): `aws sts assume-role
--policy '{...Resource: [".../only-this-key.webp"]}'` against a local
`rustfs/rustfs:1.0.0-rc.2` container returned a working temporary
credential; that credential successfully wrote to the allowed key,
was denied (`AccessDenied`) writing to a different key in the same
bucket, and was denied `ListBuckets` entirely. RustFS's STS + inline
session policy enforcement is real and correctly scoped, not aspirational.

## Rationale

- **FR-017** ("short-lived, per-request credential scoped only to that
  user's permitted campaign paths, not the storage service's permanent
  root/admin credential") is satisfied in the strongest available sense:
  even a compromised server process only ever holds a credential good for
  one object key, for 15 minutes, never a durable key — and the client
  never touches a storage credential of any kind, which is a strictly
  narrower attack surface than a client-facing scoped-credential model.
- **FR-016** ("reject before any object is created") is satisfied by
  running the `world_members` authorization check
  (`src/server/src/auth/world_membership.rs::require_world_member`)
  before any credential is minted or transcode work begins.
- **FR-018** (one asset storage mechanism, not two) is satisfied by
  routing both the new `uploadCanvasImage` GraphQL mutation and the
  migrated `map_import.rs::save_background_image` through the same
  `storage/rustfs.rs` + `storage/transcode.rs` functions.
- Single-object-key policy scoping (rather than a world-level prefix)
  was chosen over a broader/reusable credential because the credential
  is minted and consumed inside one request and never handed out — there
  is no legitimate need for a wider scope, and narrower is strictly safer.

## Alternatives Considered

- **Client-facing scoped credential** (a `requestUploadCredential`-style
  mutation, client PUTs directly to RustFS): rejected — it would need a
  second code path to re-enforce FR-013's size/format checks (or trust
  the client), and provides no benefit once the server must already
  decode+transcode bytes server-side for WebP conversion (FR-012).
- **MinIO** instead of RustFS: rejected — the feature input specifically
  named RustFS; no functional requirement depends on a MinIO-specific
  capability, so this was a directed choice, not re-litigated here.
- **One long-lived internal service credential** reused across all
  writes: rejected — contradicts "short-lived"/"per-request" in
  FR-017/SC-007.
- **Server-side proxy write using RustFS's permanent root credential**:
  rejected outright by FR-017's literal text; per-write STS minting is
  what makes the proxy-style write compliant.

## Consequences

- `src/server/src/storage/rustfs.rs` and `transcode.rs` become the single
  choke point for all canvas image asset writes; any future asset type
  should route through the same module rather than re-implementing
  filesystem or S3 access.
- Local dev requires the `rustfs` compose service to be running for any
  asset-write path (paste or map import) to succeed; `docker compose up`
  remains the single provisioning command (FR-020) — no second tool.
- RustFS's STS credentials are JWTs signed by the RustFS root credential
  and are only as trustworthy as that root key's secrecy; the root key
  itself is never exposed outside `storage/rustfs.rs` and the compose
  environment.

**Satisfies Constitution Principle IV**: this ADR lands in the same
change set as the storage implementation, not as a retroactive
afterthought.
