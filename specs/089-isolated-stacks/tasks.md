# Tasks: Isolated Stacks

**Input**: Design documents from `specs/089-isolated-stacks/`
**Prerequisites**: spec.md, plan.md

**Tests**: TDD for the pure parts, each written and seen failing before its
code:

- the project name (T010)
- port discovery parsing (T011)
- free-port allocation (T020)
- the slot lock (T021–T023)
- the orphan listing (T040)

The `pnpm test:scripts` suite stays green after every task.

**Proof**: real runs only where noted. Each one waits until no
`e2e-parallel.mjs` runs anywhere and the 1-minute load is under 8. Never
the full suite.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different file, no unfinished dependency).
- **[Story]**: US1 concurrent checkouts, US2 install, US3 stack lifecycle,
  US4 dev and cargo test.

---

## Phase 1: The install exposes only the app (US2)

- [ ] T001 [US2] `compose.yml`:
  - remove `ports:` from `postgres`, `rustfs` and `mailpit`;
  - remove every `container_name`;
  - keep the app's `${THUNDERFORGE_PORT:-42080}:30000`;
  - update the file's header comment.
- [ ] T002 [P] [US2] New `compose.ports.yml` publishing Postgres
  `127.0.0.1:${THUNDERFORGE_POSTGRES_PORT:-42432}`, RustFS `…:42900`, and
  Mailpit `…:42025` and `…:42825`. Include a header comment on when to use
  it.
- [ ] T003 [US2] Search the repo for `thunderforge-app-postgres`,
  `thunderforge-app-rustfs`, `thunderforge-app-mailpit`, `thunderforge-app`,
  `42432`, `42900`, `42025` and `42825`. Update every Makefile target
  (`container*`), script and doc that used a fixed name or port, to use
  `docker compose -f compose.yml exec <service>` or the override.
- [ ] T004 [US2] README.md install section: only the app is exposed, and
  how to add `compose.ports.yml` (for example for `pg_dump`). Add a release
  note line for operators who relied on the old ports.
- [ ] T005 [US2] **Proof:**
  - run `docker compose -p tf-install-check -f compose.yml up -d --build`;
  - `docker compose -p tf-install-check ps --format json` shows exactly one
    published port (SC-003);
  - sign up through the app and see the mail in Mailpit (via `exec`);
  - upload a map;
  - add `-f compose.ports.yml` and see the four `127.0.0.1` ports;
  - tear down with `down` (not `-v`), then list the volumes and ask before
    removing them.

**Checkpoint**: the install is ready on its own. This phase can merge
before the others.

---

## Phase 2: A stack per checkout (US1 data isolation, US3)

- [ ] T010 [P] [US3] Test, then `stack.mjs` `projectName(root)`:
  - the basename, sanitised to `[a-z0-9-]`, plus a 6-hex SHA-1 of the
    realpath;
  - stable for one path, different for two same-named checkouts.
- [ ] T011 [P] [US3] Test, then `parsePort(output)` for
  `docker compose port` output (`127.0.0.1:49153`, `0.0.0.0:…`, IPv6
  `[::1]:…`, empty). It refuses anything not bound to loopback.
- [ ] T012 [US3] New `compose.e2e.yml`:
  - **Postgres** (`postgres:18-alpine`, the same as dev): healthcheck,
    `max_connections` raised to match today's shard pool maths, and the
    volume `db`.
  - **RustFS**, pinned to the same version as `compose.dev.yml`.
  - **Ports:** both on `127.0.0.1::<port>`.
  - **Labels:** `thunderforge.checkout=${THUNDERFORGE_CHECKOUT}`.
  - **Names:** no `container_name`.
- [ ] T013 [P] [US3] New `compose.e2e.tmpfs.yml` override, which puts
  Postgres's data on `tmpfs`, used when `THUNDERFORGE_E2E_TMPFS=1`
  (plan.md Decision 2).
- [ ] T014 [US3] `stack.mjs` `ensureStack(root)`:
  - runs `up -d --wait` (Compose's own health wait) with the project name and
    the checkout label;
  - reads the ports;
  - returns `{ project, databaseUrl(name), psql(db, sql),
    rustfsEndpoint }`.

  If it fails, it reports which project and service, plus the last 20 log
  lines. It never falls back to `thunderforge-postgres` (spec Edge Cases).
- [ ] T015 [US1] `e2e-parallel.mjs`:
  - `psql`, `provisionTemplate`, `provisionFirstRunTemplate`,
    `cloneShardDatabase` and `startShard` take the stack;
  - every `localhost:5432` becomes `stack.databaseUrl(…)`;
  - `RUSTFS_ENDPOINT` goes in the shard env from `stack.rustfsEndpoint`;
  - `THUNDERFORGE_POSTGRES_CONTAINER` is honoured only as an explicit
    override, with a warning that it brings back sharing.
- [ ] T016 [US3] Template reuse: the migration stamp check that decides
  whether to rebuild templates now reads the stamp from the checkout's
  Postgres. The second run in a worktree logs "templates up to date"
  (SC-004).
- [ ] T017 [US1] `.gitignore` and CONTRIBUTING: nothing new is written to
  the checkout except what `.e2e-shards/` already holds.

**Checkpoint**: data is isolated, but runs still take turns under the
`f4263b49` machine lock. Prove nothing has regressed: one `book-import`
slice and one engine slice (`tokens`), each green, and both logs naming the
checkout's project. Don't merge this phase on its own until phase 3's slots
are ready (plan.md Risks).

---

## Phase 3: Free host ports and a concurrency cap (US1)

- [ ] T020 [P] [US1] Test, then `ports.mjs` `freePorts(n)`:
  - returns n distinct ports bound on `127.0.0.1`;
  - a port held by a test listener is never returned.
- [ ] T021 [P] [US1] Test: slots. With `MAX=2`:
  - two holders take slots 0 and 1;
  - a third waits and names both holders;
  - `--no-wait` fails with their details;
  - a dead holder's slot is taken.
- [ ] T022 [P] [US1] Test: with a live pre-089 `thunderforge-e2e.lock`,
  every slot is held (FR-010).
- [ ] T023 [P] [US1] Test: a second run from the same checkout waits on
  `.e2e-running` (FR-008) even when slots are free.
- [ ] T024 [US1] `run-lock.mjs`:
  - adds slot acquire and release next to the per-checkout lock (T021–T023
    go green);
  - stops writing `thunderforge-e2e.lock`, but keeps reading it.
- [ ] T025 [US1] `e2e-parallel.mjs`:
  - removes `WEB_PORT_BASE`, `BACKEND_PORT_BASE`, `GITHUB_STUB_PORT_BASE`,
    `OAUTH_STUB_PORT_BASE`, both Mailpit port bases and `assertPortsFree`;
  - each shard gets its ports from `freePorts` and carries them on the shard
    object;
  - every `BASE + index` reads the shard's own ports, including the
    Playwright env (`PLAYWRIGHT_BASE_URL`, `THUNDERFORGE_E2E_*_STUB`,
    Mailpit).
- [ ] T026 [US1] A shard whose child fails to bind retries once with fresh
  ports, then fails as today (plan.md Decision 4).
- [ ] T027 [US1] `mailpit.mjs`:
  - container names become `<project>-mailpit-<index>`, with the checkout
    label;
  - publishes `127.0.0.1::1025` and `127.0.0.1::8025`;
  - reads both ports back;
  - `clearLeftovers` clears only this project's containers.
- [ ] T028 [US1] `.hooks/pre-push` and `.hooks/pre-commit` follow plan.md
  Decision 8. `THUNDERFORGE_IGNORE_E2E_LOCK` keeps working.
- [ ] T029 [US1] **Proof, SC-001 and SC-002:**
  - **Setup:** two worktrees on this branch, the machine quiet.
  - **The runs:** start `pnpm e2e:book-import` in one and another
    non-engine slice in the other, within 5 seconds of each other.
  - **Expected:** both pass, neither waits, and each names its own project
    and ports.
  - **Database check:** a scripted
    `DROP DATABASE … ; CREATE DATABASE … TEMPLATE …` on one worktree's
    shard mid-run, and the other run shows no database errors.
- [ ] T030 [US1] **Proof:** the cap. Start a third run while two are going.
  It waits and names both, then starts when one ends.
- [ ] T031 [US1] **Proof:** one engine slice (`tokens`) alone, green. This
  shows the engine path is unaffected by the new ports.

**Checkpoint**: phases 2 and 3 merge together.

---

## Phase 4: Cleanup, `cargo test`, the other runners (US3, US4)

- [ ] T040 [P] [US3] Test, then `stack.mjs list()`:
  - parses `docker compose ls --format json` and the checkout labels;
  - marks each stack `checkout exists`/`gone`;
  - sizes its volumes from `docker system df -v --format json`.
- [ ] T041 [US3] `make e2e-stacks` prints the list. `make e2e-stacks-rm
  STACK=<project>` shows what it will remove (containers, network,
  volumes, with sizes) and asks `y/N` before `down -v`. Nothing removes a
  stack without that answer.
- [ ] T042 [US3] The `gc` target appends a one-line note when any stack is
  `gone` ("2 e2e stacks belong to deleted checkouts: make e2e-stacks"). It
  never removes them.
- [ ] T043 [US4] `stack.mjs env` prints `TEST_DATABASE_URL` (database
  `thunderforge_test` on the checkout's Postgres) and `RUSTFS_ENDPOINT`.
  `make test-rust STACK=checkout` uses them, and makes the
  `thunderforge-canvas-assets` bucket first (memory: cargo test needs that
  bucket).
- [ ] T044 [US4] Update spec.md FR-014's wording to match plan.md
  Decision 6.
- [ ] T045 [US4] **Proof:**
  - `make test-rust` (the default) passes against the dev Postgres;
  - `make test-rust STACK=checkout` passes against the checkout's stack;
  - both together from two worktrees, each with `STACK=checkout`, both
    pass.
- [ ] T046 [P] [US1] `compose.journeys.yml` and `compose.torture.yml`
  publish `127.0.0.1::<port>`, and `scripts/journeys.mjs` and
  `scripts/torture.mjs` read the ports back through `stack.mjs`'s
  `parsePort`. Keep their `JOURNEYS_*`/`TORTURE_*` port variables as
  explicit overrides.
- [ ] T047 [US1] **Proof:** `pnpm journeys` on one file, plus
  `make test-torture-session-5`, green.
- [ ] T048 [P] [US1] Check that `make dev` and `compose.dev.yml` are
  unchanged on a fresh clone (SC-005).

---

## Phase 5: Documentation

- [ ] T050 [US1] `docs/CONTRIBUTING.md`: replace "One e2e run per machine"
  with how stacks, ports, slots, `THUNDERFORGE_E2E_MAX_CONCURRENT`,
  `THUNDERFORGE_E2E_TMPFS`, `make e2e-stacks` and `STACK=checkout` work.
  Include what happens beside an older branch's run.
- [ ] T051 [US1] Update the agent memory notes that describe the shared
  test infrastructure, so they describe per-checkout stacks.

## Dependencies

- Phase 1 stands alone.
- Phases 2 and 3 merge together. Within them, T010–T014 come before T015,
  and T020–T024 before T025.
- Phase 4 depends on T014 (`stack.mjs`).
- Phase 5 comes last.
