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

### 2. A flaky shared fixture — **fixed**

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

**Fixed** by the first of the two options that were open: membership is now
created over GraphQL, and the UI walkthrough is reserved for
`invite-membership.spec.ts`, the one spec that is actually about the invite
flow — it still clicks every button. Everything else gets a real second
account, a real registration, a real session cookie and a real `joinWorld`
mutation, without paying to exercise an interface it is not testing.

`genie-resource-trade` and `status-disclosure` went green as a direct result,
having never had anything wrong with them.

## The catalogue of causes, so far

Fixing the first batch turned up four distinct causes, none of which the
failure text pointed at. They are worth knowing before touching any remaining
spec, because between them they explain most of what is left.

**1. A tool panel asserted without opening it.** The GM rail mounts _only the
open tool's content_ — deliberately, so a tool nobody is using leaves no
listeners on the canvas. So `wall-tool`, `shape-tool` and the rest are simply
not in the DOM until their icon is clicked, and a reload closes the rail again.
`openGmTool` / `openDockTab` in `e2e/fixtures/helpers.ts` handle it, and are
idempotent because both regions are toggles.

**2. Scene creation moved.** `SceneSwitcher` — and with it "New scene" — is
mounted in exactly one place: the Settings section of the play view's dock. It
is not on `/staging`, which is where `registerAndCreateWorld` leaves the page.
A helper calling it there looked for a button that did not exist, then clicked
a dock tab that does not exist on that route, and waited out the full test
timeout. That reads like a broken app rather than a helper on the wrong page.

**3. A created scene is not the world's _active_ scene.** Which scene a reload
lands on is server state (ADR-046). Creating one through the switcher selects
it for this client only — so a spec that draws on the new scene, reloads, and
asserts the content survived is silently returned to the previous scene, where
its content genuinely does not exist. This one cost the most time, because
"walls do not survive a reload" is a completely plausible bug and the store,
the props and the DOM were all correct.

**4. A reload refetches scene content over a separate round trip.**
`waitForEngineReady` waits for the canvas, which says nothing about whether the
scene's _walls_ have arrived. Clicks land before them and select nothing.
`waitForWallsLoaded` / `waitForShapesLoaded` poll the store rather than
sleeping longer, because how long a refetch takes is not a constant.

### And one real product bug

`GmToolRail` held "which tool is open" in local state, and it is rendered only
once the scene and the viewer's role have resolved — so it remounted as those
settled and lost the state. A Game Master clicked Walls, the panel appeared,
and it vanished again for no reason they could see. Lifted to the page.

## Not every failing spec should be made to pass

Some assert behaviour the product deliberately removed. `invite-membership`
expected the join page to say "invalid invite code"; spec 027 (FR-011/SC-005)
now gives every dead link the _same_ message, because telling the holder of a
killed link which cause applied is precisely what the server refuses to
disclose. The right fix was to assert the new requirement — and that no reason
appears anywhere on the page — not to restore the old text.

So each failure needs a verdict, not just a patch: stale spec, real product
bug, or intentionally-removed behaviour. Weakening an assertion to get green is
the one outcome worse than leaving it red.

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
