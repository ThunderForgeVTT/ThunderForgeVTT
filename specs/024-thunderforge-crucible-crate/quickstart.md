# Quickstart: Validating the Crucible Crate

Prerequisites: `make dev` (or `pnpm dev`) running, or just the Rust
workspace (`cargo build`) — this feature has no frontend surface to
exercise.

## 1. Default (local) mode requires nothing new (User Story 1)

1. Start the `thunderforge` server exactly as today, with no
   `CRUCIBLE_MODE`/`CRUCIBLE_ENDPOINT` set.
2. Confirm the server starts successfully with no new required
   ports/processes — `ps`/`docker ps` shows nothing beyond what already ran
   before this feature.
3. Trigger any adjudication-eligible action through the normal server code
   path this feature wires into (see tasks.md for exactly which call site).
   Confirm it resolves successfully, in-process (no new network call
   observed).

## 2. Standalone mode produces identical results (User Story 2)

1. Run `cargo run --bin crucible-server` on its own (default port TBD in
   tasks.md).
2. Start a second `thunderforge` server instance with
   `CRUCIBLE_MODE=remote` and `CRUCIBLE_ENDPOINT` pointing at the
   `crucible-server` instance from step 1.
3. Trigger the same adjudication-eligible action as in section 1, step 3.
   Confirm the result matches section 1's result exactly.

## 3. Misconfiguration fails fast, not silently (Edge Cases, SC-003)

1. Start `thunderforge` with `CRUCIBLE_MODE=remote` and no
   `CRUCIBLE_ENDPOINT` set. Confirm the process exits immediately with a
   clear error naming the missing variable — it does not start accepting
   connections.
2. Start `thunderforge` with `CRUCIBLE_MODE=not-a-real-mode`. Confirm the
   same — immediate exit, clear error naming the accepted values (`local`,
   `remote`).

## 4. Unreachable remote produces a clear error, not a hang (SC-004)

1. Start `thunderforge` with `CRUCIBLE_MODE=remote` and
   `CRUCIBLE_ENDPOINT` pointing at a port nothing is listening on.
2. Trigger an adjudication-eligible action. Confirm a clear error surfaces
   within the bounded timeout (research.md §3) — not an indefinite hang.

## Automated coverage expectations (for tasks phase)

- `cargo test` coverage in `crates/thunderforge-crucible` for:
  `LocalAdjudicator`'s placeholder ruleset (always `Accepted` for a
  well-formed request, `InvalidRequest` for a malformed one), the
  request/response JSON (de)serialization round-trip, and an in-process
  integration test (research.md's "Testing" note in plan.md) proving
  `RemoteAdjudicator` against a locally-spawned `crucible-server` produces
  identical results to `LocalAdjudicator` for the same input (User Story
  2's core claim, without needing a separately-run process in CI).
- `cargo test`/`cargo check` coverage in `src/server` for: the startup
  config-parsing logic (valid `local`, valid `remote` with a valid
  endpoint, invalid mode, `remote` with missing endpoint) — matching
  quickstart section 3 above as unit-testable logic, not just a manual
  walkthrough.
