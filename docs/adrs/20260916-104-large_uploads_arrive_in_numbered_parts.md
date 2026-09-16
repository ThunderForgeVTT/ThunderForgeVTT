# ADR-104: Large Uploads Arrive in Numbered Parts, Through the Server

**Date:** 2026-09-16
**Status:** **PROPOSED** 2026-09-16 with spec 059. Decisions 1–3 are the owner's; the protocol choice is proposed.
**Participants:** ThunderForgeVTT Team
**Related:** spec 059 (Decisions 1–7, FR-001 … FR-095), ADR-039 (extended), ADR-088 (settings precedence), spec 051 (pause gate), spec 037 (retention sweep and the delete exception)

---

## Problem Statement

Every upload reaches the server as one HTTP request carrying the whole file:
map import through `POST /api/scenes/{scene_id}/import/uvtt`
(`src/server/src/map_import/mod.rs:76`), images through GraphQL multipart
(`src/app/src/main.rs:636-639`). ADR-039 decided *who* writes to storage — the
server, with a credential scoped to one key — and implicitly that the bytes
arrive in one piece.

One piece does not survive Cloudflare. Its proxied request body limit is 100 MB
on Free and Pro, 200 MB on Business and 500 MB by default on Enterprise
(Cloudflare Workers limits and Error 413 pages, checked 2026-09-16), and an
instance behind `cloudflared` is behind that limit. A `.dd2vtt` carries its
image as base64, a third larger than the image, and large battlemaps pass
100 MB. One piece also does not survive a dropped connection, and its
processing holds the request open past Cloudflare's 125 s proxy read timeout.

## Decision

1. **A large upload is a session of numbered, fixed-size, checksummed parts,
   sent through the server.** Start declares purpose, target, size, type and —
   for a map — the map's non-image fields; parts are sent by number; completion
   lists the parts' checksums; cancel aborts. The server chooses the part size
   (8 MiB by default) and derives every storage key.

2. **Parts become an S3 multipart upload under a staging prefix**, through the
   existing `aws-sdk-s3` client and ADR-039's scoped credential, extended from
   `s3:PutObject` to the multipart actions on the one staging key. Staged
   objects are never deduplicated or referenced, so the delete guard admits
   their prefix as it admits `feedback/`.

3. **Authority is checked at start and again at completion**, inside the
   transaction that writes the result. An upload does not carry authority past
   the moment it is used.

4. **Processing happens after completion, not inside a request.** The client
   reads named steps. No request waits on a decode.

5. **The protocol is this product's own, not tus.** See Alternatives.

6. **Presigned direct-to-storage parts are a later phase**, and this shape is
   chosen so that phase changes only where part *n* is sent.

7. **Map import moves first**; other image uploads stay on GraphQL multipart
   until there is a reason to move them.

## Rationale

The server-in-the-middle path is the one that works on every deployment this
product supports, including a home server behind a tunnel whose storage
endpoint is not reachable from a browser. Numbered parts are the native shape
of S3 and R2 multipart, so the server passes a part straight to `UploadPart`
without re-slicing, parts can be retried and sent in parallel independently,
and a presigned phase can hand the browser a URL per part without a new
protocol. Checking authority twice is what makes a session that may last hours
no weaker than a request that lasts seconds.

## Consequences

- **Memory is bounded by parts, not files.** At most one part buffered per
  in-flight part request, and a process-wide cap on those requests; processing
  streams the assembled object to a temporary file.
- **The storage module gains a second deletable prefix and multipart calls.**
  The rule in `storage/dedupe.rs` — nothing referenced may be deleted — is kept
  by construction, because nothing references a staged object.
- **A sweep now owns abandoned uploads.** Expired sessions and orphaned
  multipart uploads under the staging prefix are aborted on a schedule, with a
  bucket lifecycle rule recommended as a backstop.
- **The client owns a small upload engine** — retries, concurrency, resume —
  that an off-the-shelf tus client would otherwise provide, and spec 059's e2e
  (a body-limit proxy, a severed part, a reload) exists to hold it to account.
- **This does not make storage work on R2.** ADR-039's per-operation STS
  `AssumeRole` has no R2 equivalent; that is a separate amendment to ADR-039
  (spec 059 Question 1).
- **The single-request map import endpoint is retired.**

## Alternatives Considered

- **tus 1.0.** A mature open protocol with a good browser client. Rejected: its
  core is an offset-append stream, where S3 and R2 want numbered parts of at
  least 5 MiB and R2 wants them equal, so a tus server over S3 buffers and
  re-slices; parallel parts need its concatenation extension; its metadata
  rides in a base64 header, which cannot carry a map's walls under Cloudflare's
  128 KB header limit; and the Rust servers available bring their own S3 store
  and credentials, bypassing ADR-039.
- **Presigned multipart first.** Fastest, and no bytes through the server.
  Rejected for now by the owner: it needs a storage endpoint reachable from the
  browser, which a tunnelled RustFS is not, and it does not work on the setup
  the owner runs.
- **Raise the limit instead.** Only Enterprise can, and it fixes neither resume
  nor the read timeout.
- **Shrink the art in the browser and send one request.** Small enough for any
  proxy, and it moves the resize — and its grid-exact cell arithmetic — out of
  the server, contrary to the owner's Decision 3. Left as spec 059 Question 4.
