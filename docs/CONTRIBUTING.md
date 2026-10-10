# Contributing

First of all, thank you for your interest in contributing! It is greatly appreciated whether its a small typo, patch with fixes, or new feature.
However, before you check your code in be sure to adhere to the following rules!

## Branch Naming

Please use `[github username]/[issue id]-[short description]` for your branch naming schema. If there is no issue for the branch you are creating,
than prefix it with whether it is a `fix, feature, doc, security`. Here are a few examples:

```sh
# Simple
octocat/1-patch-readme

# With Issue
mbround18/2-add-new-authorizor

# No Issue
mbround18/fix-http-get-route

mbround18/feature-new-auth-platform

mbround18/doc-minor-readme-typo
```

## Build commands

`make help` lists every target. The ones you will use most:

```sh
make dev            # postgres + rustfs, migrations, demo seed, then the app
make lint           # clippy for the host and for wasm32, plus the file-length check
make test-rust      # cargo test (ARGS="-p thunderforge-server --lib settings")
pnpm verify         # formatting and lint, both languages
```

### Standalone harnesses

Two pages run without the stack: no `make dev`, no database, no server, no
login. Reach for them before the full app when the work is in their part.

- [`apps/engine-sandbox`](../apps/engine-sandbox/README.md): the wasm engine
  on a canvas with example maps (`pnpm -F @thunderforge/engine-sandbox dev`).
- [`apps/hero-builder`](../apps/hero-builder/README.md): the hero builder on a
  page of its own, for tuning looks and race-aware dice in `packages/heroes`
  (`pnpm -F @thunderforge/hero-builder-app dev`). It needs no stack at all.

### Large downloads

Anything large the web app fetches — the engine, a scene's images, the
world cache's assets — goes through `packages/downloads`
(`@thunderforge/downloads`), reached from the app as
`apps/web/src/services/downloads.ts`. A file at or above the threshold
(16 MiB), uncompressed, offering byte ranges and a strong `ETag` or a
`Last-Modified`, is fetched in parts (8 MiB, four at a time) that resume
after a cut; anything else is one plain request. The server's ranged
answers come from `crates/thunderforge-server/src/assets_serve/ranged.rs`,
and built files from `ServeDir`, which offers ranges on the precompressed
copies too.

Fixtures are rarely 16 MiB. In a dev build a test can shrink the numbers
before the page loads; a production build never reads this:

```ts
await page.addInitScript(() => {
  globalThis.__thunderforgeDownloadSettings = {
    threshold: 4096,
    partSize: 16384,
  };
});
```

`pnpm e2e:resumable-downloads` proves it: the package against a real socket,
then the app against the stack.

### Rolls

Every roll is a row in `world_rolls` and a world event: `ROLL_MADE` (36)
when it is made, `ROLL_REVEALED` (37) when the GM reveals it. The payload is
`{rollId, visibility}` and nothing more; a client fetches the roll itself
with `worldRoll`. A board animates a roll from its event, never from the
panel that asked for it, so a roll from the sheet in another tab plays on the
same board.

Who may see what is decided in one place, `crates/thunderforge-server/src/rolls/visibility.rs`:
`may_roll` (who may pick a visibility), `view_of` (a whole roll, a
`MaskedRoll` or nothing) and `event_reaches` (whether a `ROLL_MADE` event is
delivered at all). The fetch, the feed, the live subscription and the
catch-up all go through it. A new path to a roll must use it too, rather
than check visibility itself. A `MaskedRoll` cannot be built from a roll's
row, so a masked roll cannot carry its numbers by mistake.

The demo answers GraphQL in the browser, so it mirrors the rule in
`apps/demo/src/backend/handlers/dice.ts` and `events.ts`. Its tabs share one
world through `apps/demo/src/backend/tabs.ts`. The tab holding the
`thunderforge-demo-world` Web Lock runs every tab's operations, one at a time
and each as the asking tab's viewer. The other tabs ask over the
`thunderforge-demo` BroadcastChannel. Every tab filters the events it hears
for its own viewer. When the holder closes, a waiting tab reads the saved
world and takes over.

`pnpm e2e:rolls` proves all of it: the demo's tests and its two-tab e2e,
then the app against the stack.

#### Roll facets and rerolls

Spec 084. A game system shapes its rolls through the `roll_facets` slot of
its server contribution (`RollFacets` in
`crates/thunderforge-canvas-core/src/roll_facets.rs`). The slot has:

- `shape`: given a `ShapeInput` (the roll's kind, its formula, the actor's
  `trait_data`, the advantage chosen, and for damage whether the attack was
  melee and the item's properties), it returns the formula to roll and the
  facet ids it applied, or `None` to roll the formula as written. 5e's
  is `packs/systems/dnd5e/server/src/roll_facets.rs`: advantage, Halfling
  Luck and Great Weapon Fighting.
- `reroll`: whether the sheet can pay a spend (`inspiration`,
  `luck_point`) and the `trait_data` after paying.
- `labels` and `spends`: the names the table sees for each id.

The host calls `shape_roll` in `crates/thunderforge-server/src/rolls/facets.rs`
for every check, to-hit and damage roll, and records the result's facets on
the roll row. A system without the slot rolls everything as written and
refuses advantage.

A shape never edits formula text. It goes through
`thunderforge_dice::rewrite_dice_terms`, which hands it each dice term
(`TermView`) and applies the `TermEdit` it returns: a new count, or added
modifiers such as `kh1`, `r1` or `min3`. A reroll goes through
`thunderforge_dice::replay`, which replays a recorded resolution with one
die rerolled (`ReplayEdit::RerollDie`), so the other dice keep their values.
The rules for who may reroll, until when and with what are in
`crates/thunderforge-server/src/rolls/reroll.rs`. A rerolled to-hit
re-judges its attack in `combat/attack_reroll.rs`, against the defence
stored with the first attack.

Shared code carries facet, spend and item-property ids as opaque strings.
It stores them, checks them against what the pack declares, and passes them
to the pack, but it never branches on one. Only the pack knows what
`great_weapon_fighting` or `two_handed` does. Item properties are declared
in the pack's `system.json` under `itemProperties`.

The demo mirrors 5e's shaping in `apps/demo/src/backend/handlers/facets.ts`.

#### Dice on the board

A board throws a roll as dice (spec 083). The throw is built in two places:

- `crates/thunderforge-canvas-core/src/dice_throw/` decides everything a
  throw shows: which solid each die is (`shapes`), the orientation that puts
  the server's face toward the viewer (`landing`), the seeded path from the
  roll's id (`tumble`), how a `DieOutcome`'s chain expands into drawn dice
  (`expand`), the line of arithmetic (`readout`), the timings (`TIMINGS`)
  and the burst queue (`queue`).
- `crates/thunderforge-engine/src/plugins/dice/` only draws it: one
  `Mesh2d` per die, rewritten each frame, on a stage that follows the
  camera so the throw sits in screen space.

The logic lives in canvas core because the engine compiles for wasm32 only,
so a `#[test]` there never runs. Canvas core builds for the host, and
`cargo test -p thunderforge-canvas-core dice_throw` covers it.

The web hands the engine the whole roll: `buildDiceThrow` in
`apps/web/src/engine/bevy/diceThrow.ts` sends
`{type: "trigger_dice_roll", roll}`, where `roll` is the `WorldRoll`'s id,
roller, label, formula, `bindings` and resolution, with each die's `rolls`,
`steps` (`REROLL` or `EXPLODE`), `kept` and `finalValue`. The engine never
rolls, totals or guesses: the faces and the total are the server's. Reduced
motion is `{type: "set_reduced_motion", reduced}`, sent from the OS setting
by `watchReducedMotion`. The roll panel waits on the engine's own timings
(`dice_timings()`), so the two never drift.

A test reads what the engine drew from `__engineProbe.diceLanded()`: one
entry per throw, with each die's `sides`, `face`, `kept`, `rerolled`,
`clamped`, `explosionOf` and `restingPlace`, plus the `readout`, the `chip`
and whether it was `skipped`. `diceEntities()` is how many dice entities are
alive, 0 once every throw has faded. `apps/web/e2e/fixtures/rolls.ts` wraps
both. `dicePlayed()` is unchanged: what the board was handed.

To tune a throw, use the engine sandbox (`apps/engine-sandbox`). Its formula
field and **Roll** button roll with the dice crate and throw the result, and
its reduced-motion checkbox sends `set_reduced_motion`.

One follow-up is open (research R4): the dice crate sets an exploded die's
`final_value` to the last value of its chain, rather than the chain's sum
(`crates/thunderforge-dice/src/eval.rs:303`). The readout uses the crate's
addends, and when they do not reach the server's total it falls back to
`formula = total`, so the board never shows a sum the server did not make.

### Drawings

A player draws by default (spec 082): `effective_authoring_tools` gives a
member Select and Shapes unless a row in `world_authoring_tool_revocations`
takes one away. A Game Master's grant or revocation is announced as
`AUTHORING_TOOLS_CHANGED` (38), `{userId}`, and the web rail asks again.

Every shape write asks one function,
`crates/thunderforge-server/src/auth/shape_authority.rs`: `Dm` may do
anything, `Creator` may change only the shapes they made and cannot hide them,
and `None` is told the shape or scene was not found. `createShape`,
`updateShape`, `deleteShape`, `clearShapes` and `shapeCreators` all go
through it, and a new path to a shape must too. The engine and the web gate
on `createdBy` as a courtesy; the server is the rule.

`clearShapes` records one ordinary `deleted` shape event per shape, so no
client needed a new event. Deleting an account removes the user's drawings in
other people's worlds (`users/shape_cleanup.rs`), announced by each world's
owner, because `shapes.created_by` and `updated_by` have no `ON DELETE`.

The demo mirrors all of it in `apps/demo/src/backend/handlers/shapes.ts`.

### Proving a change

A change is proven by its feature's slice, not by the full e2e suite. A
slice is the feature's own specs plus the neighbouring specs that read what
it writes, and it runs in minutes. The rule is
[constitution Principle VI](../.specify/memory/constitution.md#vi-every-feature-is-proven-by-its-own-slice);
how slices are declared is
[ADR-107](adrs/20260922-107-a_feature_is_proven_by_a_declared_slice.md).

Ask which slices your change needs, then run them:

```sh
pnpm e2e:which --diff     # your branch against origin/main, plus uncommitted files
pnpm e2e:combat           # each slice it names
pnpm e2e:slices           # every slice, its specs and how long it takes
```

`pnpm e2e:<slice>` runs the slice's stack-free suite first where it has
one, then `pnpm e2e:<slice>:integration`, which runs its specs against the
real stack.

**The full suite is the gate when a cross-cutting path changes.** Those are
the paths any slice could depend on: the GraphQL schema, auth, migrations,
the e2e fixtures and harness, shared UI and styles, and the lockfiles. The
lookup says `FULL SUITE` beside them, and the full list is `crossCutting` in
`scripts/e2e/slices.json`. Run `node ./scripts/e2e-parallel.mjs` before
merging.

**Adding a spec.** Every spec under `apps/web/e2e` belongs to exactly one
slice. Add it to the `own` list of its feature's slice in
`scripts/e2e/slices.json`. An entry is either an exact file name
(`combat-panel.spec.ts`) or a prefix (`combat-`); an exact name beats a
prefix, and a longer prefix beats a shorter one. If the spec also asserts
something another feature writes, add it to that slice's `neighbours` with
a one-sentence `seam` saying what crosses. A new slice needs its scripts in
the root `package.json`, and `node scripts/check-e2e-slices.mjs --fix`
writes them.

**If you forget**, the `e2e-slices` step of `pnpm verify`, which also runs
on every commit, fails and names the file and the fix:

```text
apps/web/e2e/new-thing.spec.ts belongs to no slice — add it to "own" of a slice in scripts/e2e/slices.json
```

It fails the same way on a neighbour or an exact name that no longer
exists, a prefix or path glob that matches nothing, a spec two slices own
equally, and a slice script in `package.json` that is missing or has
drifted. Only the scripts are fixed for you. Which slice a spec belongs to
is a judgement about the feature, so the check never guesses it.

### One e2e run per machine

Every checkout and worktree on a machine shares what an e2e run uses: the
shard databases (`thunderforge_e2e_<shard>`) on one Postgres server, the
shards' ports, and the mailpit containers. So the run lock is machine-wide,
not per checkout: `/tmp/thunderforge-e2e.lock` (`THUNDERFORGE_E2E_LOCK_DIR`
moves it), holding the pid, start time, arguments and checkout of the run
that owns it (`scripts/e2e/run-lock.mjs`).

- **A second run waits.** Started from any checkout while another run is
  live, `e2e-parallel.mjs` (and every `pnpm e2e:<slice>`) prints which
  checkout and pid it is waiting on, repeats that every five minutes, and
  starts when the first run exits. `--no-wait` makes it fail instead.
- **A stale lock never blocks.** A lock whose pid is dead, or now belongs to
  something that is not an e2e run (a run killed with `kill -9`), is replaced.
- **The hooks.** `pre-commit` refuses only during a run started from *this*
  checkout, because `pnpm verify` writes into the tree that run's Vite servers
  watch; another checkout's run is a one-line note. `pre-push` refuses during
  a run from *any* checkout, because clippy competes with it for every core.
  `THUNDERFORGE_IGNORE_E2E_LOCK=1` turns either refusal into a warning; it
  never lets two e2e runs share the machine.
- A checkout on a branch from before the machine-wide lock writes
  `.e2e-running` inside itself instead. Those are read in every worktree of
  the repository, so such a run is waited on too.

```sh
node scripts/e2e/run-lock.mjs check --any-checkout   # is any e2e run live? (exit 1 if so)
```

### The test database

`cargo test` never touches the development database. Database-backed tests use
`thunderforge_test`, which the test harness (`crates/thunderforge-server/src/test_support.rs`)
creates and migrates the first time a test asks for it. `TEST_DATABASE_URL`
names a different one; without it, the name in `DATABASE_URL` is swapped for
`thunderforge_test`. The harness refuses the development database and the e2e
shards' databases outright.

That separation is what makes `cargo test` safe beside an e2e run — it used to
share the development database's global settings rows with it — and it is why
the development database no longer fills with test users.

```sh
make test-db-reset   # drop thunderforge_test and rebuild it, migrated and empty
node scripts/cleanup-dev-test-rows.mjs   # one-off: count (or --apply to delete) the
                                         # test rows left in the development database
```

`cargo test -p thunderforge` needs `RUST_MIN_STACK=16777216`; `make test-rust`
sets it.

### Cleaning up build output

Cargo never deletes what it built. Old incremental sessions, artifacts from
feature and flag combinations nobody builds any more, and finished agent
worktrees pile up — past a terabyte on one machine. `make clean` is not the
answer: it takes the warm cache with it.

```sh
make clean-builds                  # dry run: what would go, and the size
make clean-builds ARGS="--apply"   # delete it
```

It removes, from this checkout only:

- incremental sessions, keeping each crate's newest and anything from the last
  3 days (`--incremental-days=N`);
- cargo units (`deps/`, `build/`, `.fingerprint/`) not rebuilt in 7 days
  (`--deps-days=N`) — the same rule as `cargo-sweep --time`, without installing
  it;
- worktrees under `.claude/worktrees/` that are clean, merged into `main`, not
  locked and not any running process's working directory, with their merged
  branch (`git branch -d`). Each worktree it keeps is printed with the reasons.

Anything it deletes costs at most a rebuild. It refuses while an e2e run or a
cargo process is working in the checkout, and it never touches anything outside
`target/` and `.claude/worktrees/`. `make dev` and the e2e harness print a
one-line note when the build output has grown enough to be worth it.
`--root=<path>` points it at another checkout.

### Upgrading Bevy

Spec 087 took the engine from 0.19.1 to 0.20.0. Its `research.md` records
what moved; the steps it followed, in `quickstart.md`, are the ones to repeat.

- **Take a baseline first, on the old version.** Frame rate from
  `pnpm e2e:engine-limits` (three runs, on a quiet machine, with the load
  average next to each), the release `.wasm` raw and brotli, the render-probe
  lines, and the four captures. Once a version has moved, there is nothing to
  compare against.
- **bevy and glam move together.** `thunderforge-canvas-core` depends on
  `glam` directly and must name the major the new bevy pulls in, or
  `bevy::prelude::Vec2` and canvas-core's `Vec2` become two types. After the
  bump, `cargo tree -d --workspace --target all` must show one `glam` and one
  `wgpu`.
- **Shaders are WESL.** They are `.wesl` files with `import …;` lines, not
  naga_oil's `#import`. A shader that fails to compile does not fail the
  build; it shows up at runtime, so check the darkness slice.
- **The render halves.** `bevy_sprite`, `bevy_ui` and `bevy_gizmos` each need
  their `*_render` feature. Without it everything compiles, runs and logs
  nothing, and the canvas shows only the clear colour. The render probe
  (`set_render_probe`) tells the two apart.
- **Build in a worktree.** A worktree resolves `@thunderforge/engine` through
  its own `dist/engine`, so the new engine stays out of other work's e2e runs
  until it merges. After the merge, rebuild main's engine with
  `node scripts/build.mjs --only-wasm`.

### Telemetry

Spec 086 reports through OpenTelemetry, on by default, and the operator
redirects or turns it off ([the guide](guides/telemetry.md)). Anything that
adds a span, a metric or a log record follows these rules:

- **Names.** Instruments are `thunderforge.<area>.<thing>`, with the unit in
  the instrument, not the name; Prometheus adds `_total` and the unit suffix
  after the collector. Browser events and spans use the names in
  `packages/telemetry/src/allowList.ts`.
- **Bounded labels only.** A label is one of a known, small set: a GraphQL
  root field taken from the schema (never the client's operation name, and
  `unknown` for one the schema lacks), an event code name, an outcome, a roll
  visibility, a pool state. A query with several root fields is labelled by
  its first, with `root_fields=multiple` on the span.
- **No ids as labels.** No user, world, actor, scene or token id, no email,
  and no session id is ever a Prometheus label or a Loki index label. A
  browser session id is Loki structured metadata, nothing more. Page views
  name the route template (`/world/:id/play`), never the path.
- **The anonymous allow-list is code.** What the anonymous tier may send is
  the constants in `crates/thunderforge-telemetry-policy` and the matching
  lists in `packages/telemetry/src/allowList.ts`; a Rust test holds the two
  together, and the server and the telemetry gateway read the same crate.
  Adding an entry to either list is a constitution-level change
  (Principle VII), reviewed as one: say what it sends, why it cannot
  identify anyone, and update Appendix A of `specs/086-full-telemetry/spec.md`
  and the places that quote it in the same change.
- **Tests send nothing.** Every Rust test and e2e stack runs with
  `TELEMETRY=false`. A test that needs telemetry installs an in-memory
  exporter or routes the browser's requests in Playwright.

#### The public gateway and the policy crate

`telemetry.thunderforge.dev` is not the collector. It is
`apps/telemetry-gateway` (`thunderforge-telemetry-gateway`), a small Axum
service in front of the collector's public receiver. It is the project's
alone: a self-hosted operator never runs it, and their server sends straight
to their own collector. The contract is
`specs/086-full-telemetry/contracts/telemetry-gateway.md`.

- **One list.** Everything the gateway decides comes from
  `crates/thunderforge-telemetry-policy`: the service names, the attribute
  allow-lists per place, the size caps, the metric names (`INSTRUMENTS`,
  exactly; R27), the labels it sets and the drop reasons. The server reads
  the same crate, so the gateway cannot drift from what a release sends.
  `scripts/check-observability.mjs` reads the instrument names from the same
  place (`print_instruments`), so a dashboard cannot read a series the
  gateway would refuse.
- **What it does.** It decodes OTLP protobuf or JSON, drops a resource whose
  service is not ours or whose instance id is malformed, drops a record with
  an attribute over its cap, strips (and counts) an attribute not on the
  list, drops a metric whose name is not listed, sets the labels below, and
  forwards a freshly built protobuf request with only `Content-Type`. What
  it dropped comes back in OTLP's `partial_success`, merged with the
  collector's own.
- **The labels.** `thunderforge.ingress`, `thunderforge.source`
  (`owner_site`, `self_hosted_browser`, `server`), `thunderforge.origin.host`,
  `thunderforge.instance.id`, `thunderforge.client.version`,
  `thunderforge.user_agent.family` and `.major`, and `thunderforge.country`
  when Cloudflare gave one. They overwrite a sender's attribute of the same
  name, and are resource attributes: none becomes a Prometheus or Loki index
  label. `Origin`, `User-Agent` and `CF-IPCountry` can be forged, so the
  labels are indicative.
- **The IP address.** It is read from `X-Forwarded-For` at
  `TELEMETRY_GATEWAY_TRUSTED_HOPS`, hashed with a per-process random key to
  find its rate-limit bucket, and dropped when the request ends. It is never
  logged, never a metric attribute and never forwarded. A test holds this:
  the address and the user agent appear in no byte the fake collector
  received, no log line and no metric.
- **The drop reasons.** `thunderforge.telemetry_gateway.dropped` counts once
  per request per `reason`: `rate_limited_ip` and `rate_limited_instance`
  (`429` with `Retry-After`), `body_too_large` (`413`), `undecodable`
  (`400`), `unknown_service`, `instance_id`, `attribute_too_large` and
  `metric_name` (`200` with `partial_success`), and `overloaded` and
  `upstream_error` (`503`). `DropReason::ALL` is the list; a new reason is
  added there, to the contract and to the Server dashboard's row together.
- **Shipping order (R27).** The gateway refuses a metric it does not list,
  so a release that adds an instrument ships its gateway with it or before
  it: `make push-telemetry-gateway`, then restart the Flux Deployment, then
  release the server. A server released first loses the new series until
  the gateway catches up; nothing else breaks.
- **Running it.** `cargo test -p thunderforge-telemetry-gateway` runs the
  router in process against a fake collector. The fixtures in
  `apps/telemetry-gateway/tests/fixtures/` are captured, not written: the
  browser's with `node apps/telemetry-gateway/tests/fixtures/capture-browser.mts`,
  the server's with
  `cargo test -p thunderforge-telemetry-gateway capture_server_fixtures -- --ignored`.
  Recapture them when either encoder changes.

## Feature flags

A feature can be merged before it is switched on. A flag is how: a boolean
instance setting in the `Features` group, resolved per request, so an
administrator changes it from the instance's settings and the next request
plays by it — no rebuild, no restart. The environment beats the instance's
stored value, which beats the declared default, exactly as for every other
setting ([Instance configuration](INSTANCE_CONFIGURATION.md)).

Flags are for the instance as a whole. There are no per-user or per-world
flags, percentages or experiments. A compile-time switch
(`#[cfg(debug_assertions)]`, `import.meta.env.DEV`) is for local development
only and never decides what a deployed instance offers.

### When a feature takes one

- It will be merged before it is finished, and must not be reachable until
  it is. Declare it with a default of `false`.
- It is finished, but an operator has a real reason to run without it: it
  costs something, it carries a legal or moderation burden, or it is new
  enough that switching it off must not need a deploy. Declare it with a
  default of `true`.

A bug fix, a refactor, or a change with one right answer does not take a
flag. Neither does anything a world's Game Master should decide; that is a
world setting.

### Declaring one

1. **Declare it** in
   `crates/thunderforge-server/src/settings/registry/declarations.rs`:
   `Kind::Bool`, `group: "Features"`, a key `feature.<name>`, an environment
   variable `THUNDERFORGE_FEATURE_<NAME>`, and a default. `what_to_set` and
   `what_is_limited` are what the administrator reads; write them for that
   reader.
2. **List it** in `FEATURES` in `settings/features.rs`, saying whether a
   visitor who is not signed in may know it. A test refuses a flag that is
   declared and not listed, or listed and not declared.
3. **Enforce it on the server**, at the mutation or query that does the
   thing: `settings::flag_on(state, features::YOUR_FLAG).await?`, and refuse
   with a sentence that says an administrator can switch it on. Hiding a
   control is a courtesy; this is the rule.
4. **Hide the control** in the web app with `useFeatureFlag(key)` from
   `@/hooks/useFeatureFlag`, with the key exported from
   `@/api/featureFlags`. It reads as off until the server has answered. No
   component reads the environment or a build constant to decide what to
   show.
5. **Document the variable** in `.env.example`, and regenerate the schema if
   you touched it (`node scripts/check-graphql-contract.mjs --schema --fix`).
6. **Prove both halves** end to end, as
   `apps/web/e2e/instance-feature-flags.spec.ts` does for the first flag:
   off, the control is gone and the server refuses; on, both come back.

### Removing one

A flag that defaults to `false` is a promise to finish. When the feature is
on by default and has been through a release that way, either delete the
flag — declaration, `FEATURES` entry, `flag_on` call, `useFeatureFlag` call
and the `.env.example` line, in one commit — or say in its declaration why
an operator keeps the switch. A stored value for a removed flag is an inert
row and needs no migration.

## Commits

When comitting, its fine to have short commits or using or own style but this repository uses squash and merge for pull requests.

## Pull Requests

Similar to branch naming, prefix your pull request with `fix, feature, doc, security` and give it a slightly more verbose title than the branch name.
Then in the pull request use the template provided to include details about whats changed. Example title:

```sh
[Fix] Minor data issue on token events
```
