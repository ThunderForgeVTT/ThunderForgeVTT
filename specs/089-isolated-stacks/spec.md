# Feature Specification: Isolated Stacks

**Feature Branch**: `089-isolated-stacks`
**Created**: 2026-10-09
**Status**: Draft (spec only; plan.md and tasks.md not written)
**Input**: The owner, 2026-10-09: "how feasible would it be to setup a
compose network and only expose the app so i can do many pg databases on my
system". The agreed answer was to give each checkout its own private stack,
with Postgres, RustFS and Mailpit published only on random ports bound to
localhost. The test harness keeps running the server on the host and finds
those ports at startup.

## Why

Every checkout on this machine shares one set of test infrastructure:

- one Postgres (`thunderforge-postgres` on `5432`), which holds every
  checkout's `thunderforge_e2e_<shard>` databases and both templates;
- one RustFS on `9000`;
- a fixed block of host ports for each shard's Vite, backend, Mailpit,
  GitHub stub and OAuth stub (`scripts/e2e-parallel.mjs:88-110`).

Two checkouts running e2e at the same time drop each other's live
databases and fight over the same Mailpit containers and ports. This
happened repeatedly on 2026-10-09, with four spec branches in four
worktrees. The fix that went in that day (`f4263b49`) is a machine-wide
lock. It made the runs correct by making them take turns, so four agents
now wait in line for a test run, each slice behind the others.

The install stack (`compose.yml`) has a related, smaller problem. It
publishes Postgres (`42432`), RustFS (`42900`) and Mailpit (`42025`/`42825`)
on the host as well as the app. An operator running the install never needs
those ports, and each one is a service reachable from outside the
container network.

## What exists

Counted on 2026-10-09 against `main` at `f4263b49`.

- **`compose.yml`** is the install: app plus postgres, rustfs and mailpit,
  each with its own fixed container name (`thunderforge-app-*`) and its own
  published host port. The app reaches the others by service name already
  (`postgres:5432`), so the published ports are not used by the app.
- **`compose.dev.yml`** is the dev dependencies for a host-run `make dev`:
  postgres on `5432`, rustfs on `9000` and mailpit, with fixed container names
  (`thunderforge-postgres`, `thunderforge-rustfs`). `cargo test` uses the
  `thunderforge_test` database on this same Postgres.
- **`compose.journeys.yml`** and **`compose.torture.yml`** each define their
  own Postgres and RustFS with published ports.
- **`scripts/e2e-parallel.mjs`**:
  - runs the backend binary and Vite on the host, one each per shard;
  - talks to Postgres at `localhost:5432` through the container named by
    `THUNDERFORGE_POSTGRES_CONTAINER` (default `thunderforge-postgres`);
  - gives each shard a RustFS bucket (`tf-e2e-<index>`) on the shared RustFS;
  - starts one Mailpit container per shard (`scripts/e2e/mailpit.mjs`).
  - **Port bases:** every shard's port is a fixed base plus the shard index:
    - Vite `5200`
    - backend `30100`
    - Mailpit SMTP `31025` and API `38025`
    - GitHub stub `31500`
    - OAuth stub `31600`
- **`scripts/e2e/run-lock.mjs`** holds the machine-wide lock at
  `/tmp/thunderforge-e2e.lock`. A second run from any checkout waits for it.

## Decisions already made

- **Random localhost ports, not a fully containerised harness.** The server
  and Playwright keep running on the host, so a run never has to build the
  server into an image. Backing services publish `127.0.0.1::<port>`: Docker
  picks a free port, nothing is reachable from off the machine, and the
  harness reads the port with `docker compose port`.
- **One compose project per checkout**, named after the checkout. The
  project brings its own network, containers and volumes, so N checkouts give
  N Postgres servers that all listen on 5432 inside their own networks.
- **The install exposes only the app.** Postgres, RustFS and Mailpit stop
  publishing host ports in `compose.yml`. An operator who wants them on the
  host opts in with an override file.

## User Scenarios & Testing

### User Story 1 - Two checkouts run e2e at the same time without touching each other (Priority: P1)

A developer, or an agent, starts a slice in one worktree while another
worktree's slice is already running. Both start immediately. Each one runs
against its own Postgres, RustFS and Mailpit, and its shards use host ports
that nothing else holds. Neither run can see, drop or mail into the other's
state.

**Why this priority**: this is the point of the spec. Today four branches
take turns for the test infrastructure, and a slice can wait for hours
behind runs that have nothing to do with it.

**Independent Test**: start the same short slice from two worktrees within
a few seconds of each other. Both pass, both logs show no waiting, and each
names a different compose project and different ports.

**Acceptance Scenarios**:

1. **Given** checkout A is mid-run, **When** checkout B starts a slice,
   **Then** B starts without waiting, on its own project
   (`tf-<checkout>`) and its own ports, and A's databases are untouched.
2. **Given** both runs are going, **When** either one drops and recreates
   its shard databases, **Then** the other's queries keep working.
3. **Given** a run in checkout B sends mail, **When** checkout A reads its
   newest message, **Then** A never sees B's message.
4. **Given** a run from this checkout is already going, **When** a second
   run starts from the same checkout, **Then** it waits, as the per-checkout
   lock does today.

---

### User Story 2 - The install exposes only the app (Priority: P1)

An operator runs `docker compose up -d` from `compose.yml`. Only the app's
port is published on the host. Postgres, RustFS and Mailpit are reachable
only inside the project's network.

**Why this priority**: it is the owner's original ask, it is a few lines,
and it closes ports that serve no purpose for an operator.

**Independent Test**: `docker compose -f compose.yml up -d`, then
`docker compose ps` shows a published port only for `app`, and the app
works: sign-up mails reach Mailpit and an uploaded map reaches RustFS.

**Acceptance Scenarios**:

1. **Given** the install is up, **When** the operator lists published
   ports, **Then** only the app's (`THUNDERFORGE_PORT`, default `42080`)
   appears.
2. **Given** the operator needs the database on the host for a backup,
   **When** they add the documented override file, **Then** Postgres, and
   only Postgres, is published on `127.0.0.1`.
3. **Given** two installs on one machine under different project names,
   **When** both are up, **Then** they do not collide. This includes the
   fixed `container_name`s, which this story removes or derives from the
   project name.

---

### User Story 3 - Each checkout's stack starts, is found and is cleaned up without anyone thinking about it (Priority: P2)

A run brings up its checkout's stack if it is not already up, waits for it to
be healthy, reads its ports, and leaves it running for the next run, so a
second slice does not pay for start-up again. A command removes it, and the
existing `gc` hygiene reports stacks whose checkout no longer exists.

**Why this priority**: without it, isolation costs a cold Postgres start
per run and leaves stacks behind for deleted worktrees.

**Independent Test**: run a slice twice in one worktree. The second run's
log shows it reused the stack. Remove the worktree, then the cleanup
command lists that stack and removes it only when asked.

**Acceptance Scenarios**:

1. **Given** no stack for this checkout, **When** a run starts, **Then** it
   brings one up, waits for Postgres to be healthy, and builds its templates
   there.
2. **Given** the stack is already up, **When** a run starts, **Then** it
   reuses it, and reuses the templates if the migration stamp still matches.
3. **Given** a worktree was deleted, **When** the developer runs the
   cleanup, **Then** its stack is listed by name and size and is removed only
   on confirmation, never automatically. Volumes hold data.

---

### User Story 4 - The development stack and `cargo test` keep working (Priority: P2)

`make dev` and `cargo test` keep their current behaviour by default:
`compose.dev.yml`'s Postgres on 5432, and `thunderforge_test` for cargo.
A developer can point a checkout's `cargo test` at that checkout's own stack
with one variable.

**Why this priority**: the dev loop is used all day, and this spec must not
make it slower or different.

**Independent Test**: `make services-up && make dev` works unchanged.
`cargo test` passes against the dev Postgres, and passes again against this
checkout's stack with the variable set.

**Acceptance Scenarios**:

1. **Given** a fresh clone, **When** the developer follows CONTRIBUTING's
   setup, **Then** nothing about the dev stack has changed.
2. **Given** `THUNDERFORGE_TEST_STACK=checkout`, **When** `cargo test`
   runs, **Then** it uses this checkout's Postgres and RustFS bucket, and two
   checkouts' `cargo test` runs do not interfere.

---

### Edge Cases

- **Many stacks at once.** Engine-heavy runs compete for CPU and GPU even
  when their data is isolated. A machine-wide cap on concurrent runs, default
  2, replaces the single machine-wide lock: run 3 waits for a slot and says
  which runs hold them. The 2026-09-11 measurement (one run never saturated
  the GPU or RAM) is the starting point, not proof that 4 would be fine.
- **Host ports for host processes.** Vite, the backend and the stubs run on
  the host, not in compose, so random Docker ports don't help them. Each run
  gets free ports from the OS (bind to port 0, or probe a block) instead of
  fixed bases, and passes them to Playwright and the backend through the env
  it already uses.
- **A stack that is down or unhealthy.** The run says which project and
  service, and how to bring it up. It never falls back to the shared
  `thunderforge-postgres`, because that would quietly bring the collisions
  back.
- **Docker restarts.** Random host ports can change when a container
  restarts, so the harness reads ports at the start of every run and never
  caches them across runs.
- **Older branches.** A checkout on a branch from before this spec still
  uses the shared Postgres and the machine-wide lock. While such a run holds
  the lock, a new-style run treats it as holding every slot. This keeps
  `f4263b49`'s guarantee for branches that predate this spec.
- **Disk.** Each stack costs roughly a Postgres data directory plus RustFS
  objects. Shard databases are created from a template, so they stay small.
  Postgres for e2e may use `tmpfs`; whether that's worth it is in Open
  items.
- **Journeys and torture.** `compose.journeys.yml` and `compose.torture.yml`
  move to random localhost ports in the same way. Their own runners already
  start their stacks.

## Requirements

### Functional Requirements

- **FR-001**: `compose.yml` MUST publish a host port only for `app`.
  Postgres, RustFS and Mailpit MUST be reachable from the app by service name
  and from nowhere else by default.
- **FR-002**: An override file (`compose.ports.yml`) MUST publish Postgres,
  RustFS and Mailpit on `127.0.0.1` for an operator who needs them. The
  install guide in `docs/guides/` MUST document it.
- **FR-003**: `compose.yml` MUST NOT set fixed `container_name`s, so two
  projects can run side by side.
- **FR-004**: The e2e harness MUST run its backing services as a compose
  project named for the checkout (`tf-<sanitised worktree dir>`), with
  Postgres, RustFS and Mailpit published only on `127.0.0.1` random ports.
- **FR-005**: The harness MUST read each service's host port from
  `docker compose port` at the start of every run, and MUST NOT use fixed
  ports for backing services.
- **FR-006**: The host processes the harness starts (Vite, backend, GitHub
  stub, OAuth stub) MUST take free ports from the OS for each run, not fixed
  bases.
- **FR-007**: Mailpit MUST be one per shard inside the checkout's project,
  or one per checkout with a mailbox per shard. The plan chooses which.
  Either way, mail MUST never cross checkouts.
- **FR-008**: The per-checkout lock MUST remain: one run per checkout at a
  time.
- **FR-009**: The machine-wide lock MUST become a concurrency cap,
  `THUNDERFORGE_E2E_MAX_CONCURRENT` with default 2. A run past the cap waits,
  naming the holders. The run's `--no-wait` MUST fail instead of waiting.
- **FR-010**: A lock file written by a pre-089 run (`.e2e-running` in a
  checkout, or the 2026-10-09 machine lock) MUST count as holding every slot.
- **FR-011**: The pre-push and pre-commit hooks MUST keep their current
  meaning: push refuses while this checkout's run is going, and also while
  any pre-089 run is going, since that run uses shared state.
- **FR-012**: A cleanup command (`make e2e-stacks`) MUST list every
  `tf-*` project with its checkout path, whether that path still exists, and
  its volume size. It MUST remove a stack only on explicit confirmation. The
  `gc` target MAY report orphans but MUST NOT delete them.
- **FR-013**: `compose.dev.yml`, `make dev` and the default `cargo test`
  database MUST be unchanged.
- **FR-014**: `cargo test` MUST accept `THUNDERFORGE_TEST_STACK=checkout`
  to use this checkout's stack.
- **FR-015**: `docs/CONTRIBUTING.md` MUST replace the "One e2e run per
  machine" section with how stacks, ports, the cap and cleanup work.

### Key Entities

- **Checkout stack**: a compose project `tf-<checkout>` with Postgres,
  RustFS and Mailpit. It has its own network and volumes, and publishes only
  random `127.0.0.1` ports.
- **Run slot**: one of `THUNDERFORGE_E2E_MAX_CONCURRENT` machine-wide
  slots. A slot records pid, checkout, start time and args, and is stale
  when its pid is dead or isn't an e2e run.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Two worktrees start the same slice within 5 seconds of each
  other. Both pass, and neither log contains a wait.
- **SC-002**: A worktree drops and recreates its shard databases mid-run.
  The other worktree's run reports zero database errors.
- **SC-003**: With the install up, `docker compose ps --format` lists
  exactly one published port.
- **SC-004**: A second slice in the same worktree reaches its first test at
  least as fast as it does today on the shared Postgres. Reusing the stack
  means no cold start.
- **SC-005**: `make dev` and default `cargo test` behave exactly as before
  on a fresh clone.

### Proof

- The run-lock and slot logic gets unit tests in
  `scripts/e2e/__tests__/` with fake holders, as `f4263b49` did. It is
  never proven by starting real runs alongside other agents' runs.
- **SC-001 and SC-002:** two real short slices (`book-import` and another
  non-engine slice) run from two worktrees at once, when no other e2e is
  running.
- **The install:** `docker compose -f compose.yml up -d` on a clean project
  name, then a sign-up mail and a map upload.
- No full suite. These are the slices `pnpm e2e:which` names for the
  harness files.

## Assumptions

- Docker Compose v2 with `docker compose port` is available, as every
  existing compose file already assumes.
- 2 concurrent runs is a safe default on this machine. US1's proof
  measures it, and the default is raised only on evidence.
- The server needs no code change: it already takes `DATABASE_URL`, the
  RustFS endpoint and SMTP settings from the environment.

## Open items

- **Mailpit:** per shard (as today, simplest) or one per checkout with a
  mailbox per shard (fewer containers). Decided in plan.md.
- **`tmpfs` Postgres for e2e stacks:** faster and leaves nothing on disk, but
  the templates are rebuilt after every Docker restart. To be measured in the
  plan.
- **Operators upgrading:** anyone relying on `42432` and the other
  published ports from `compose.yml` loses them. The release notes and the
  install guide name the override file.
