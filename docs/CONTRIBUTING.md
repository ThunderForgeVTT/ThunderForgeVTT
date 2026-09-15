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

### The test database

`cargo test` never touches the development database. Database-backed tests use
`thunderforge_test`, which the test harness (`src/server/src/test_support.rs`)
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

## Commits

When comitting, its fine to have short commits or using or own style but this repository uses squash and merge for pull requests.

## Pull Requests

Similar to branch naming, prefix your pull request with `fix, feature, doc, security` and give it a slightly more verbose title than the branch name.
Then in the pull request use the template provided to include details about whats changed. Example title:

```sh
[Fix] Minor data issue on token events
```
