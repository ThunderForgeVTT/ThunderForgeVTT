# The e2e suite's own health

What a full `playwright test e2e --workers=1` pass currently reports, and how
much of it is the product versus the suite. Written because the answer is not
obvious from the output, and because reading "42 failed" without this context
leads to exactly the wrong conclusion.

## The measurement

Two runs, on the same machine, back to back.

| Run                                    | Tests | Passed | Failed | Skipped | Wall clock |
| -------------------------------------- | ----- | ------ | ------ | ------- | ---------- |
| Full suite, `030-interactive-elements` | 202   | 159    | 42     | 1       | 1.5h       |
| The 20 failing files, on `main`        | 77    | 44     | 32     | 1       | 35m        |

The second run exists because the first is unreadable without it. 42 failures
means nothing until you know what the same machine does with the same specs on
a branch that has none of the new work.

## What the failures are

Of 42 failures, **40 are "element not found" or a locator timeout**. Two are
assertions about a value. That ratio is the finding: the suite is
overwhelmingly failing to _find_ the UI it wants, not disagreeing with what the
product computed.

```
18  Test timeout
18  Error: element(s) not found
17  expect(locator).toBeVisible() failed
 5  locator.click: Test timeout
 4  locator.click: Test ended
```

## Two distinct causes

### 1. UI drift, in files nobody updated

`canvas-authoring` (6) and `map-editor-tooling` (3) look for `wall-tool` and
`map-import-success`. Both test ids exist in the source. They are not reachable
from where the specs look, because commit `7c14b6b` ("Replace the play-view
sidebar with a dock and a GM tool rail") moved them and the specs were not
brought along.

`auth-providers` (6–7) and `token-authoring` (4) are the same shape.

These fail identically on `main`. They are a backlog item — specs describing a
UI that has moved — not a regression, and not flake: they fail every time.

### 2. A flaky shared fixture, and it is the big one

`inviteAndJoinAsPlayer` in `e2e/fixtures/helpers.ts` drives the two-browser
invite flow through the real UI: click **Generate Join Link**, read the code,
register a second account, visit `/join/:code`, click **Join Campaign**. Under
suite load one of those buttons intermittently fails to appear, and the test
dies on a 300-second timeout.

It does not fail in the same place twice. Observed across four runs:

| Run                 | Which spec it killed   | Where              |
| ------------------- | ---------------------- | ------------------ |
| Full suite          | `interactive-prop`     | Generate Join Link |
| 8-spec group        | `interactive-prop`     | Join Campaign      |
| 8-spec group, again | `genie-resource-trade` | (invite flow)      |
| `main` subset       | `invite-membership`    | (invite flow)      |

**It is whichever spec happens to be using the helper when the machine is
busy.** Every spec that uses it is a candidate, which is why the failure list
looks alarmingly broad and moves between runs.

Run alone, the specs it kills pass in seconds — `interactive-prop` passes in
15.7s, twice in a row, immediately after failing inside a group.

This is worth fixing at the fixture rather than per spec. Two obvious options:
create the membership through GraphQL and reserve the UI walkthrough for
`invite-membership.spec.ts`, which is the spec that actually tests the invite
flow; or give the helper a retry with a longer budget. The first is better —
every other spec is paying the cost of testing something it is not about.

## Reading a run

1. **A spread of failures across unrelated features is the fixture, not the
   product.** Look at whether they cluster on specs that open a second browser.
2. **A failure that reproduces alone is real.** One that vanishes alone is
   load.
3. **Compare against `main` before concluding anything.** Cheaply: run just the
   failing files on `main`, not the whole suite.

## Known-good subsets

These pass reliably in isolation and are the ones worth gating on until the
fixture is fixed:

```bash
# Spec 030, all seven stories plus its capacity measurement — 8 passed, 2.1m
pnpm exec playwright test e2e/interactive-*.spec.ts e2e/engine-interaction-limits.spec.ts --workers=1

# Spec 029
pnpm exec playwright test e2e/status-*.spec.ts --workers=1
```

## Capacity, from the same full pass

Recorded here because a full run measures it for free and the figures otherwise
live only in a scrollback.

```
engine sweep      3200 tok  30fps 33.3ms | 4800 tok 30fps 33.6ms | 5600 tok 20fps 50.2ms
status displays    400 tok  61fps 16.5ms | 1600 tok 61fps 16.5ms | 3200 tok 30fps 33.5ms
interactives       200 tok + 50 regions: absent 17.2ms/58fps, present 16.9ms/59fps, ratio 0.983
```
