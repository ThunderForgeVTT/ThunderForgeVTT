# Phase 0 Research: Optional Lore Synchronisation to an External Repository

**Feature**: `034-lore-git-sync` · **Date**: 2026-09-04

Every decision here was checked against the tree rather than recalled. Where
something does not exist yet, that is stated as a finding, because "we already
have this" is the assumption that costs the most when it is wrong.

---

## R1. How the server talks to a repository

**Decision**: Invoke the `git` binary as a subprocess, over HTTPS, against a
server-managed working clone per connection.

**Rationale**: The deciding argument is FR-004b/FR-004c, not convenience. The
spec requires that everything downstream of the credential grant be
host-neutral — path mapping, commit synthesis, attribution, divergence
detection, verification — and *git over HTTPS is the host-neutral protocol*.
GitHub's Contents and Git Data REST APIs would put GitHub inside the
synchronisation engine, which FR-004c forbids in as many words: no component
beyond the grant may read an artefact of how access was arranged.

Three further properties come free and would each be real work otherwise:

- **Rename detection.** FR-010 requires a move to be recorded as a move. Git
  computes this at diff time from content similarity; nothing needs to track it.
- **Divergence detection.** FR-031 needs to know the remote history no longer
  contains what we wrote. `git push --force-with-lease` refuses exactly that
  case, atomically, at the server.
- **Verification.** FR-034 needs the repository's contents to be checkable
  rather than assumed. A fetch plus `git rev-parse` answers it against the same
  data the push wrote.

**Alternatives considered**:

| Option | Why not |
|---|---|
| `git2` (libgit2 bindings) | A C dependency in a workspace that is otherwise pure Rust and rustls throughout — `reqwest` is configured `default-features = false, features = ["rustls"]`, deliberately avoiding OpenSSL. libgit2's HTTPS support drags that decision back in. |
| `gix` (gitoxide) | Pure Rust and the right long-term shape, but **push is its least mature surface** and this feature is nothing but pushing. Worth revisiting; not worth betting the first delivery on. |
| GitHub Contents / Git Data REST API | Rejected on FR-004c. Also makes an atomic multi-file commit a three-call tree-building dance, and makes force-push detection a manual SHA comparison rather than a server-enforced precondition. |

**Consequences that must be carried into the plan**:

1. **`git` becomes a deployment requirement, and nothing records that today.**
   There is no `Dockerfile` in this repository — `compose.yml` runs Postgres and
   RustFS only, and the application is run directly. So this adds the first
   external binary dependency the server has. It must be checked for at startup
   and reported the way FR-036c requires of a partial configuration, not
   discovered when a Game Master first connects.
2. **The credential must never reach `argv`.** Embedding a token in the remote
   URL puts it in the process table, which FR-035 forbids ("MUST never appear
   in logs"); a process listing is worse than a log. Credentials are supplied
   through `GIT_ASKPASS` pointing at a helper that reads from an environment
   variable, and the environment of a short-lived child process is the narrowest
   channel available without a library.

---

## R2. Where the working clone lives

**Decision**: One persistent clone per connection, under a server-managed
directory beside the existing asset directory, treated as a rebuildable cache.

**Rationale**: A fresh clone per run is simplest and wrong at the size this has
to work at — SC-002 says "a world of any size", and re-fetching every object
every 60 seconds (SC-003) is wasteful in exactly the case that matters. A
persistent clone makes the steady state a small fetch.

It is a **cache, not state**: everything in it can be reconstructed from the
world and the remote. Losing it must cost a full re-clone and nothing else,
which is what keeps FR-030's "converge without user reconstruction" true after
a disk is wiped or an instance is moved.

**Alternatives considered**: an in-memory or `tmpfs` clone per run (loses the
incremental fetch, same objection); a single shared clone with multiple remotes
(couples worlds together, and FR-033 exists precisely to keep two worlds from
sharing a writing surface).

---

## R3. What drives synchronisation

**Decision**: A background task spawned at startup, following the pattern
already established in `src/app/src/main.rs`.

**Rationale**: This is not a new mechanism — the binary already spawns a
presence sweep on a 60-second loop, a session cleanup task
(`session::spawn_session_cleanup_task`), a presence listener
(`network::spawn_presence_listener_task`), and the spec 028 content-hash
backfill. The convention is a `spawn_*_task` function in the library, called
from `main.rs`, owning its own schedule and staying off every hot path.
A `spawn_lore_sync_task` is the same shape and needs no new infrastructure.

That the read and the write happen on one pass is FR-034b, decided in
clarification: divergence detection and write verification are answered from
remote state the pass already fetched.

**Alternatives considered**: a queue triggered per revision (tighter latency
than SC-003 requires, and would need FR-020's batching window rebuilt on top of
it anyway); an external scheduler (a new deployment component for a loop the
process can hold itself).

---

## R4. Credential storage

**Decision**: Reuse the established `*_encrypted` column convention with
AES-256-GCM, and **extract the existing helpers from `auth/mod.rs` into a
shared module** rather than writing a second implementation.

**Rationale**: The pattern is already in the tree and already trusted:
`aes-gcm = "0.11.1"` is a direct dependency, `users.two_factor_secret_encrypted`
and the OAuth `access_token_encrypted` / `refresh_token_encrypted` columns all
use it, and `auth/mod.rs` holds `encrypt_secret`, `decrypt_secret` and
`encryption_key_from_config_secret`.

**The finding is that those three functions are private to `auth/mod.rs`.** So
this feature either moves them somewhere shared or copies them, and copying an
encryption routine is how two implementations drift until one is wrong. The
extraction is small and belongs in this feature's first task, not deferred.

**Alternatives considered**: a new key and cipher for this feature (a second key
to manage and rotate, for no gain); storing nothing and re-prompting (impossible
— synchronisation is a background task with no user present).

---

## R5. Authenticating as an installed application

**Decision**: Sign a short-lived RS256 JWT with the application's private key,
exchange it for an installation access token, cache that token until shortly
before it expires, and refresh on demand.

**Rationale**: This is the mechanism FR-036a chose. Installation tokens are
short-lived by construction, which is what makes FR-036d ("MUST be refreshed
rather than stored beyond their lifetime") natural rather than an extra rule.

**The finding is that nothing in this workspace can sign an RS256 JWT.** There
is no `jsonwebtoken`, no `rsa`, no `ring`, and no `p256` in `src/server`'s
manifest — the existing crypto surface is `aes-gcm`, `sha2`, `rand` and
`totp-rs`, none of which does asymmetric signing. So this decision **adds a
dependency**, and that should be a deliberate line in the plan rather than a
surprise in a diff.

**What is stored**: the application's private key is *instance* configuration,
not per-connection data, and belongs with the operator's other secrets. The
per-connection record stores an installation reference and nothing else —
FR-004c forbids that reference from being read anywhere past the grant.

**Alternatives considered**: a pasted fine-grained token (rejected in
clarification; would have needed no new dependency, which is worth noting as the
cost of the choice that was made); a long-lived token exchanged once (defeats
FR-036d).

---

## R6. Commit identity

**Decision**: The **committer is the application** — `ThunderForge VTT` with a
no-reply address on the instance's own domain. The **author is the world member
who wrote the revision**, under a generated no-reply address, never a personal
one.

**Rationale**: Git carries two identities per commit, and this feature has
exactly two facts to record: *who wrote this* and *what put it here*. Using both
fields for their actual meaning satisfies FR-017's two halves without
compromise — attribution to the authoring account, and no disclosure of a
personal email address the user has not chosen to publish.

Naming the application as committer is also the honest description of what
happened. A human did not run `git commit`; the platform did, on their behalf,
and a history that claims otherwise is a history that misleads a reader who
later has to work out where a change came from.

**Alternatives considered**: the user's real email (FR-017 forbids it, and the
platform has no consent to publish it); the application as *both* author and
committer (loses per-user attribution, which FR-017 requires and which is most
of the value of a readable history); the user's login on the repository host
(the platform does not know it — a repository grant is not an identity, and
conflating the two is the same error FR-036a's note warns about).

---

## R7. Path mapping

**Decision**: The path is derived from the entry's tree position and title and
is a **label**; the durable identifier in the file header is the **key**
(FR-009).

**Rationale**: Already settled in the spec, restated here because it is the
premise every path edge case rests on. A title that normalises to nothing, two
siblings differing only by case or accent, a path too long for a filesystem —
all of these are label collisions, and a label collision is resolved by
deterministic disambiguation, never by matching on the label.

The rule that falls out: **the system may rename a file freely and must never
identify a file by its name.** FR-027 is the same rule seen from the import
side — a file with no recognised identifier is a proposed new entry, never a
match by path.

---

## Open, and deliberately not resolved here

- **FR-042's determination.** The constitution requires an on-record,
  owner-accepted determination before implementation begins. The spec supplies
  the reasoning; it is a signature, and no research resolves it. **This gates
  implementation, not planning.**
- **Repository size for image-heavy worlds.** FR-014 mirrors uploaded
  originals. The spec names referencing as the fallback and calls it a
  plan-time decision; it is left open until a real world's numbers exist,
  because choosing now would be choosing without the measurement this project
  usually insists on.
