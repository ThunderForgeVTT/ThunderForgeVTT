# The torture tests

## Why these exist

Everything that makes ThunderForge feel like a table happens in the same
place: a token moves, a die lands, a corridor is revealed, and within a
fraction of a second every other person sees it. That path is the real-time
event backplane, and it has a property that makes it unusually dangerous to
get wrong.

**When it breaks, nothing says so.**

Rows keep committing to Postgres. HTTP keeps answering. `/readyz` keeps
returning 200. The process stays up and every dashboard stays green. The only
symptom is that the game stops moving on everybody's screen, and the first
report you get is a person saying "I think it's frozen?" — which is not a
stack trace.

This is not hypothetical. Every scenario below exists because a specific
failure of exactly that shape reached a running server:

- A housekeeping task that took a write lock on every shard blocked the
  delivery loop outright, so no event reached anyone. No panic, no error, no
  log line.
- A debug log statement on the delivery path did a blocking `write(2)` to
  stderr. When a log reader stalled, the write blocked the thread carrying the
  subscription, and events silently reached nobody.
- The cursor that tracks "everything up to here has been delivered" once
  advanced past a transaction that had taken its id but not yet committed,
  losing that event permanently, with no trace of any kind.

Unit tests cannot see any of these. Each one needs real concurrency, real
sockets and real contention to appear at all. That is what these are for.

## What makes them different from a benchmark

A benchmark asks _how fast_. These ask _is it still correct_, and the numbers
are a by-product.

Every scenario asserts a **property** rather than a threshold:

- every write arrives **exactly once** — not "mostly", and not twice
- no participant is **starved** — not "the average was fine"
- no table **overhears** another
- authority holds **under simultaneous writes**, not just in isolation

A run that is fast but drops one event in a thousand fails. That is the right
way round: a virtual tabletop that loses one token move per session is worse
than a slow one, because the players cannot tell it happened.

## How to run them

Every scenario is a name. The name carries its size, its spec filter and its
environment, so the same name runs the same test every time:

```bash
node scripts/torture.mjs --list                      # what's available
node scripts/torture.mjs --scenario suite            # the usual gate
node scripts/torture.mjs --scenario worlds-1000      # the big one
```

Each run stands up its own throwaway Postgres and object store on tmpfs, on
ports derived from a random run id, and tears them down afterwards. Nothing
touches a database anyone cares about, and two runs can overlap without
fighting.

The scenarios themselves live in `scripts/torture-scenarios.mjs`, together
with the question each one answers and what a failure would mean. That file is
the single source of truth: this document explains the reasoning, but the
runner reads the parameters from there, so the two cannot drift into
describing different tests.

## The scenarios

### `smoke` — is the real-time path working at all?

Tier 5, every spec, about two minutes. Small enough to run on every change.
If this is red, do not bother reading anything below it.

### `suite` — the gate

Tier 25, all five storms, about four minutes. This is the one a change should
have to pass. Twenty-five is already past any real table, so a pass here says
the guarantees hold with room to spare.

### `suite-100` — the same, wider

Tier 100. Seventeen tables of six, a hundred writers, a hundred subscribers.
Exists to catch things that scale badly between 25 and 100 — which is usually
contention rather than logic, and does not show up at the smaller size.

### `fanout-1000` — one table, a thousand listeners

Answers: what does the thousandth listener cost?

Almost nothing, as it turns out. Publishing writes a world's ring buffer once
and every listener reads from it, so the marginal cost of another subscriber
is around a nanosecond. A thousand subscribers on one world pass with the
delivery loop running at full rate and nothing lagged.

This scenario is also a caution about test design. The first attempt ran a
thousand sockets from a single page and failed with
`only 254/1000 sockets completed the subscribe handshake` — which is
**Chromium refusing to open a 256th WebSocket to one host**, not the server
refusing to serve one. Reporting that as a capacity ceiling would have been
the most misleading thing this suite could produce. `fanout-storm` shards its
sockets across browser contexts so the number describes the server.

### `writers-1000` — a thousand writers, one table

Three thousand writes from a thousand concurrent writers, every one delivered
exactly once, none duplicated. Exact-once is what the cursor and the
de-duplication memory exist for, and this is the size at which a naive
implementation of either falls over: duplicates point at the de-duplication
side, losses at the cursor advancing too eagerly.

### `worlds-1000` — a thousand tables at once

The most informative of them, because it tests the dimension the others do
not.

Depth is free — `fanout-1000` establishes that. **Breadth** is the untested
shape: the delivery loop polls `ORDER BY id ASC LIMIT 256` across _every_
world with no per-world fairness, so a thousand worlds share one window every
100ms. A thousand tables of ten players is 10,000 concurrent subscribers
across 1,001 live channels.

Its assertion is **exactness, not arrival**. One event is published per world
and every socket must receive exactly one. Fewer is delivery loss. _More_ is a
subscriber overhearing another table — a routing failure that hides easily
among five worlds and cannot hide among a thousand.

This is also the scenario that found where the ceiling actually is: fan-out
stayed free while **PostgreSQL rose to roughly one saturated core**. The
database is the first thing to run out, not delivery.

## Reading the output

Each scenario prints one `[torture] key=value ...` summary line, and those are
the same lines the assertions are made against — not a second measurement
taken for display.

The server prints a health line every ten seconds:

```
[PubSub] Metrics [10s]: sent=… (+…), polls=… (+…), cursor=…,
         errors=…, panics=…, timeouts=…, sockets=…, subs_lagged=…
```

`polls` is the one to watch. A poll happens every 100ms, so a healthy interval
shows roughly 100. **Zero does not mean quiet — it means the delivery loop has
stopped**, and the server says so in capitals rather than leaving it to be
inferred. That counter exists because separating "nothing is being written"
from "the loop reading it has died" once required attaching to a frozen
process; they are opposite faults in opposite halves of the system and `sent`
alone cannot tell them apart.

## Recording results

```bash
node scripts/torture-report.mjs --scenario worlds-1000          # dry run
node scripts/torture-report.mjs --scenario worlds-1000 --post   # publish
```

This runs the scenario and posts the result as a comment on that scenario's
tracking issue, creating the issue if it does not exist. One issue per
scenario, comments appended per run — not one issue per run, which would bury
the trend, and the trend is the interesting part.

Two guards, both deliberate:

- **`--post` is required.** Without it the script prints exactly what it would
  send. Creating issues in a shared repository should not be a side effect of
  somebody running a test to see what happens.
- **It refuses to post from any branch but `main`.** A scenario's issue is the
  record of what the engine does on the shared history. A run from a feature
  branch describes an experiment, and mixing the two means you can no longer
  read the issue and know what is true today.

A load test whose output lives in a terminal is one nobody can cite. The
numbers scroll away, the log is overwritten, and six weeks later the only
record that the engine ever carried ten thousand subscribers is somebody's
memory of having seen it — which is not a basis for saying so publicly.

## What these do not cover

Stated because the gaps matter more than the passes:

- **No backfill on subscribe.** The broadcast reaches only receivers that
  exist when an event is sent, and nothing replays. Every scenario here papers
  over this with a warm-up event or a settle window. A real client that
  subscribes between two commits still misses the one in the gap — **no load
  required**. This is the most important known gap in the delivery path.
- **A lagged subscriber gets no resync signal.** It silently loses events and
  has no reason to reconnect. It is now counted (`subs_lagged`), which is not
  the same as handled.
- **`settled_cursor` still has a documented hole**: a transaction holding its
  id longer than the two-second commit grace can be stranded.
- **One machine, one instance, debug build.** Client and server share a host,
  so past a certain size these measure contention rather than capacity.
  Horizontal scaling is designed for and untested.
- **One account.** In the large scenarios every subscriber authenticates as
  the same user from one address. That measures what a _connection_ costs the
  fan-out path — the question being asked — but it is not a thousand strangers
  on a thousand networks, and nothing here should be quoted as though it were.

## Proposed: camera storm (pan and zoom under lighting)

Raised 2026-08-30, immediately after status displays gained off-screen
culling. Not built yet.

Every scenario here so far scales _quantity_ — more sessions, more writers,
more tokens. This one holds quantity almost fixed and moves the **camera**: a
handful of tokens on screen, several lights, and a viewer panning and zooming
continuously. What it measures is stutter rather than throughput.

### Why it is worth having

Status displays cull off-screen tokens, which is what took a 3,200-token board
from 20fps back to 30 and cut the sprite delta from 12,800 to 964. The cull is
re-evaluated when the camera moves, because otherwise a token panned into view
would stay bare — for a token standing still, for ever.

That means panning is now the one input that can repaint on-screen bars
repeatedly, and it is the path with no coverage. A camera at rest jitters in
the low bits, so the redraw is gated behind an 8-unit tolerance to stop float
noise repainting every frame. **That tolerance is reasoned, not measured.** Too
tight and a still camera churns; too loose and bars visibly lag a fast pan.
Only a scenario that actually moves the camera can tell which.

Lighting belongs in the same scenario rather than a separate one: lights are
the other thing that redraws on camera movement, and the interesting question
is whether the two interact — a cost that appears only when both are moving is
exactly the kind that survives two separate green tests.

### What it should report

Frame-time **distribution**, not a mean. A pan that averages 30fps while
dropping one frame in twenty feels broken, and an average hides that
completely; the 99th percentile and the worst single frame are the figures
that correspond to what somebody notices. Sprite and geometry churn per second
is the other one worth recording, since that is the mechanism a regression here
would work through.
