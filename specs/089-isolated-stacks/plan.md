# Implementation Plan: Isolated Stacks

**Branch**: `089-isolated-stacks` | **Date**: 2026-10-09 | **Spec**: [spec.md](spec.md)

## Summary

Four phases, each proven on its own:

1. **The install exposes only the app** (US2, P1). `compose.yml` loses its
   `container_name`s and every `ports:` except the app's. A new
   `compose.ports.yml` publishes Postgres, RustFS and Mailpit on `127.0.0.1`
   for the operator who wants them. README's install section names it.
2. **A stack per checkout** (US1 and US3, P1). A new `compose.e2e.yml`
   holds Postgres and RustFS. It runs as the project `tf-<checkout>`, with
   both services on `127.0.0.1::<port>`. A new module,
   `scripts/e2e/stack.mjs`:
   - brings the project up, waits until it is healthy, and reads its ports;
   - hands `e2e-parallel.mjs` a `stack` object (`databaseUrl(name)`,
     `psql()`, `rustfsEndpoint`, `project`).

   Every `localhost:5432` and `docker exec thunderforge-postgres` in the
   runner goes through it.
3. **Free host ports and a concurrency cap** (US1, P1).
   - **Ports:** the fixed port bases go. `scripts/e2e/ports.mjs` takes
     free ports from the OS for each shard's Vite, backend and stubs, and
     Mailpit containers publish `127.0.0.1::1025/8025`.
   - **Locks:** `run-lock.mjs` keeps the per-checkout lock, and its
     machine-wide lock becomes N slot files.
4. **Cleanup, `cargo test` and the other runners** (US3 and US4, P2/P3).
   - `make e2e-stacks` lists stacks with their checkout and size.
   - `make e2e-stacks-rm STACK=…` removes one, after you confirm.
   - `make test-rust STACK=checkout` points `cargo test` at the checkout's
     stack.
   - Journeys and torture move to random localhost ports.

No server code changes. The server already takes `DATABASE_URL`,
`RUSTFS_ENDPOINT`, `RUSTFS_BUCKET` and the SMTP settings from its
environment.

## Technical Context

- **Language:** Node.js ESM scripts (`scripts/**/*.mjs`), Docker Compose v2
  YAML and a Makefile.
- **Dependencies:** none new. `node:net` handles port allocation, and
  `docker compose port` and `docker compose ps --format json` handle
  discovery.
- **Storage:**
  - **Postgres:** each checkout gets its own server, on a named volume
    `tf-<checkout>_db`. Templates live there and survive between runs, so a
    second slice reuses them.
  - **RustFS:** each checkout gets its own, with one bucket per shard as
    today.
- **Testing:**
  - **Unit:** `node --test` under `scripts/e2e/__tests__/`, via
    `pnpm test:scripts`.
  - **Real runs:** two worktrees running short slices at the same time.
- **Target:** Linux developer machines and agents. macOS Docker Desktop
  supports `127.0.0.1::<port>` the same way.

## Decisions (spec.md Open items, closed here)

1. **Mailpit stays per shard, run by `mailpit.mjs`**, not as a compose
   service:
   - **Why:** the number of shards changes from run to run, and compose
     services are fixed.
   - **Names:** containers are named `<project>-mailpit-<index>` and labelled
     `thunderforge.checkout=<path>`. Today they are
     `thunderforge-e2e-mailpit-<index>`, shared by every checkout.
   - **Ports:** each publishes `127.0.0.1::1025` and `127.0.0.1::8025`, read
     back with `docker port`.
   - **No mixing:** Mailpit has no per-recipient mailbox, so a mailbox per
     shard would put shards' mail together. One container per shard keeps
     today's guarantee.
2. **No `tmpfs` for the e2e Postgres by default.** SC-004 depends on
   templates surviving, and `tmpfs` loses them on every Docker restart.
   `THUNDERFORGE_E2E_TMPFS=1` adds a `compose.e2e.tmpfs.yml` override for
   anyone who wants speed over reuse. It ships unmeasured, and the default
   doesn't change without a measurement.
3. **Project name**: `tf-` followed by the checkout directory's basename,
   lower-cased, with every character outside `[a-z0-9-]` replaced by `-`,
   plus the first 6 hex characters of the SHA-1 of the checkout's real path.
   - **Why the hash:** two checkouts both named `ThunderForgeVTT` in
     different parents would otherwise share a stack.
   - **Example:** `tf-thunderforgevtt-088-3fa2c1`.
4. **Ports for host processes**:
   - **How:** listen on `127.0.0.1:0`, read the port, close, then hand the
     port to the child.
   - **The race:** something else can take the port between the close and
     the child's bind. `startShard` already waits for each URL, so a child
     that fails to bind becomes a shard start failure. The runner then
     retries that shard once with fresh ports.
   - **What the old check becomes:** `assertPortsFree` goes away. Its
     purpose, never testing against a leftover process, is met because a
     fresh port can't have a leftover on it.
5. **The cap**:
   - **Slots:** `$THUNDERFORGE_E2E_LOCK_DIR` (default `/tmp`) holds slot
     files `thunderforge-e2e.slot-<n>`, for `n` in `0..MAX`.
   - **Taking one:** a run takes the first free or stale slot, with
     exclusive create plus the stale rule `f4263b49` already has.
   - **Default:** `THUNDERFORGE_E2E_MAX_CONCURRENT=2`.
   - **Older runs:** while `thunderforge-e2e.lock`, the pre-089 machine
     lock, is live, every slot counts as taken (FR-010). New runs never
     write that file.
   - **Same checkout:** the per-checkout lock is `<root>/.e2e-running`, as
     it was before `f4263b49` (FR-008).
6. **cargo test (FR-014)**:
   - **No new variable:** the Rust side reads `TEST_DATABASE_URL` already
     (`test_support::choose_test_database_url`, mirrored in
     `scripts/test-db.mjs`). So `THUNDERFORGE_TEST_STACK` isn't a new
     variable for Rust to learn.
   - **How it works instead:** `make test-rust STACK=checkout` runs
     `node scripts/e2e/stack.mjs env`, which brings the stack up and prints
     `TEST_DATABASE_URL` and `RUSTFS_ENDPOINT`, then runs `cargo test` with
     them.
   - **Unchanged default:** the default stays the dev Postgres. FR-014's
     wording in spec.md is updated to say so.
7. **The install's override (`compose.ports.yml`)** publishes `127.0.0.1`
   only, on the old port numbers (`42432`, `42900`, `42025`, `42825`).
   - **Why:** an operator who scripted against them adds `-f
     compose.ports.yml` and changes nothing else.
8. **Pre-push** refuses while this checkout's run is live, or while any
   pre-089 run is live (FR-011). Another checkout's 089-style run no longer
   blocks a push, because it touches nothing this checkout owns.

## Project Structure

```text
compose.yml                 # app-only ports, no container_name
compose.ports.yml           # new: opt-in 127.0.0.1 ports for the install
compose.e2e.yml             # new: postgres + rustfs, 127.0.0.1 random ports
compose.e2e.tmpfs.yml       # new: opt-in tmpfs override
compose.journeys.yml        # ports → 127.0.0.1 random, read back
compose.torture.yml         # ports → 127.0.0.1 random, read back
scripts/e2e/stack.mjs       # new: project name, up, health, ports, psql, env
scripts/e2e/ports.mjs       # new: free host ports
scripts/e2e/run-lock.mjs    # per-checkout lock + N machine slots
scripts/e2e/mailpit.mjs     # project-scoped names, random localhost ports
scripts/e2e-parallel.mjs    # uses stack, ports, slots; no fixed bases
scripts/e2e/__tests__/      # stack, ports, slots tests
scripts/journeys.mjs        # reads its ports instead of fixing them
scripts/torture.mjs         # same
.hooks/pre-push, .hooks/pre-commit
Makefile                    # e2e-stacks, e2e-stacks-rm, test-rust STACK=
docs/CONTRIBUTING.md        # replaces "One e2e run per machine"
README.md                   # install: only the app is exposed; the override
```

## Risks

- **The runner is cross-cutting.** Every slice runs through
  `e2e-parallel.mjs`, so a mistake here shows up as unrelated failures in
  every slice. To limit that:
  - phase 2 lands before phase 3;
  - each phase is proven with a non-engine slice and an engine slice;
  - the full suite is never the gate (memory: slices only).
- **Breaking runs in flight.** A run on an older branch in another worktree
  keeps using the shared Postgres and the old lock, and FR-010 keeps it safe
  from new runs.
  - **Order:** land phase 3's slot logic in the same commit as the switch to
    per-checkout stacks, never before. Otherwise a new run could start next
    to an older run that still uses the shared Postgres.
- **Disk.** One Postgres volume per checkout. `make e2e-stacks` shows sizes,
  and nothing is removed automatically (CLAUDE.md: never prune volumes
  automatically).
- **Operators upgrading the install** lose the host ports they had before.
  The release notes and README tell them about the override file.
