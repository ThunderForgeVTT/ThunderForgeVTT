# Specification Quality Checklist: A Map That Arrives in Pieces

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-16
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

**This spec names storage operations, part sizes and a REST route, on
purpose.** The feature *is* how bytes reach storage, and the constraints that
decide it — Cloudflare's per-request body limit, S3's and R2's 5 MiB part
minimum, R2's equal-size parts — are stated in those terms by the systems that
impose them. The user-facing requirements (what a Game Master reads, what
survives a dropped connection, who may complete an upload) stay in plain words.

**The owner's 18 seconds is not fixed by chunking, and the spec says so.**
Loopback moves 4.2 MB in milliseconds. The importer decodes the base64 image
twice (`map_import/image.rs:85-87`, `:123-125`), decodes the pixels twice
(`storage/transcode.rs:124-126`, `:333-335`), encodes WebP twice, and does all
of it on an async worker thread (`image.rs:133`). FR-087 measures before FR-083
and FR-084 fix, because the attribution is an inference from the code.

**The clean "too large" refusal has never been reachable.** Both the map route
(`map_import/mod.rs:77` against `:432`) and `/graphql` (`src/app/src/main.rs:636-639`
against `transcode.rs:56-62`) set the body limit to the same number the handler
checks, and the body is always larger than the file in it. So every oversize
upload has been a 400 "error parsing multipart", and the web tool turns the
dropped connection that often accompanies it into "check your connection".

**"Works against R2 and S3" is not true of the existing storage client**, which
this spec does not cause and cannot fix alone. Every read and write calls STS
`AssumeRole` (`storage/rustfs.rs:216-236`, `:265-289`), which R2 does not offer,
with role ARNs hard-coded in RustFS's placeholder form (`:222`, `:276`).
Decision 8 makes that a separate spec amending ADR-039.

**There is a second ceiling nobody has hit.** The `image` crate's default
512 MiB decode allocation refuses images over about 134 megapixels, after the
whole upload. FR-043 to FR-045 and FR-085 make it a stated, early refusal.

**Cloudflare limits were checked on 2026-09-16** against Cloudflare's Workers
limits page, its Error 413 support page, its Connection limits page (proxy read
timeout 125 s) and the 2026-09-04 changelog (Enterprise self-serve to 5 GB).
None of those pages mentions tunnels explicitly; the spec states the tunnel
case as an assumption proven by a manual check through `make dev-tunnel`.
