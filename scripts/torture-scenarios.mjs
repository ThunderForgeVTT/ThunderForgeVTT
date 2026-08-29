/**
 * The torture scenarios, as data.
 *
 * # Why these live in one file
 *
 * Every one of these runs used to be a hand-assembled command line: a tier
 * here, a `TORTURE_WORLDS` there, a spec filter remembered from the last time.
 * That is not a reproducible test, it is a thing somebody once typed. Two
 * people running "the thousand-world test" would run two different tests, and
 * neither could tell.
 *
 * So a scenario is a name, and the name carries everything: which specs, what
 * size, which environment. `node scripts/torture.mjs --scenario worlds-1000`
 * runs the same thing today, next month, and in CI.
 *
 * # Why the prose lives here too
 *
 * `question`, `proves` and `failureMeans` are not decoration. A load test that
 * nobody can interpret is a number with no consequence attached — the run goes
 * green, everyone nods, and nothing was learned. Keeping the interpretation
 * beside the parameters means the report a run posts can say what its own
 * result *means*, and `docs/torture-tests.md` cannot drift from what the
 * runner actually does.
 */

/** Every scenario, keyed by the id you pass to `--scenario`. */
export const SCENARIOS = [
  {
    id: "smoke",
    title: "Smoke",
    specs: [],
    tier: 5,
    env: {},
    runtime: "~2 minutes",
    question: "Is the real-time path working at all?",
    proves:
      "Every scenario's assertions hold at a size small enough to run on every change.",
    failureMeans:
      "Something is broken outright. Do not read the larger scenarios until this is green.",
  },
  {
    id: "suite",
    title: "Full suite",
    specs: [],
    tier: 25,
    env: {},
    runtime: "~4 minutes",
    question:
      "Do delivery, isolation, survival and authority all hold together under ordinary concurrency?",
    proves:
      "All five storms pass at a size past any real table: exact-once delivery, no cross-table leakage, survival through churn, and permission holding under simultaneous writes.",
    failureMeans:
      "A regression in the backplane. This is the gate a change should have to pass.",
  },
  {
    id: "suite-100",
    title: "Full suite, wide",
    specs: [],
    tier: 100,
    env: {},
    runtime: "~10 minutes",
    question: "Does the same hold with a hundred participants per scenario?",
    proves:
      "The suite's guarantees are not an artifact of small numbers. 17 tables of 6, 100 writers, 100 subscribers.",
    failureMeans:
      "Something scales badly between 25 and 100 — most likely contention rather than logic.",
  },
  {
    id: "fanout-1000",
    title: "One table, a thousand listeners",
    specs: ["fanout-storm"],
    tier: 1000,
    env: {},
    runtime: "~2 minutes",
    question: "What does the thousandth listener on one table cost?",
    proves:
      "Depth is free. Publishing writes a world's ring once and every listener reads from it, so the marginal cost of another subscriber is roughly a nanosecond. 1,000 subscribers, nothing starved, nothing lagged.",
    failureMeans:
      "Fan-out has acquired a per-subscriber cost it did not have — most likely something added inside the publish loop.",
  },
  {
    id: "writers-1000",
    title: "A thousand writers, one table",
    specs: ["write-storm"],
    tier: 1000,
    env: {},
    runtime: "~2 minutes",
    question:
      "Under a thousand simultaneous writers, does every write still arrive exactly once?",
    proves:
      "3,000 writes delivered, zero duplicated, zero lost. Exact-once is the property the cursor and de-duplication machinery exist for, and this is the size that breaks a naive implementation of either.",
    failureMeans:
      "Duplicates point at the de-duplication memory; losses point at the cursor advancing past something uncommitted.",
  },
  {
    id: "worlds-1000",
    title: "A thousand tables at once",
    specs: ["world-storm"],
    tier: 1000,
    env: { TORTURE_WORLDS: "1000", TORTURE_PLAYERS_PER_WORLD: "10" },
    runtime: "~1 minute",
    question:
      "Can a thousand separate games run at the same time without hearing each other?",
    proves:
      "10,000 concurrent subscribers across 1,001 live channels, each receiving exactly their own table's event. This is the breadth case: one delivery loop polls across every world with no per-world fairness, so this is where that design is actually tested.",
    failureMeans:
      "A socket receiving more than one event is overhearing another table — a routing failure invisible at five worlds. A socket receiving none is the poll loop failing to keep up across worlds.",
  },
];

/** Look one up, or throw with the list of valid ids. */
export function scenarioById(id) {
  const found = SCENARIOS.find((s) => s.id === id);
  if (!found) {
    throw new Error(
      `Unknown scenario "${id}". Choose one of: ${SCENARIOS.map((s) => s.id).join(", ")}`,
    );
  }
  return found;
}

/** The environment a scenario needs, over and above the stack's own. */
export function scenarioEnv(scenario) {
  return {
    ...scenario.env,
    TORTURE_SESSIONS: String(scenario.tier),
    ...(scenario.specs.length > 0
      ? { TORTURE_SPECS: scenario.specs.join(" ") }
      : {}),
  };
}
