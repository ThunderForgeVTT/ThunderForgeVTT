import { REPO } from "../links.ts";

// Every figure here is copied from marketing/*.json, which scripts in the
// repository generate from a measured run. Nothing is rounded up.

const MEASURES = [
  {
    figure: "3,200",
    unit: "tokens at 30 fps",
    then: "1,600 at 60 fps",
    what: "on one board, with status displays on. The same as with them off.",
    source: "engine-status-capacity.json · 2026-08-30 · 32-core desktop",
  },
  {
    figure: "10,000",
    unit: "live subscribers",
    then: "0 starved, 0 leaked",
    what: "1,000 worlds with 10 players each, every one of them hearing every change.",
    source: "load-tier-1000.json · 2026-08-29 · debug build",
  },
  {
    figure: "75 / 75",
    unit: "writes delivered",
    then: "0 duplicates",
    what: "25 people writing to the same worlds at once.",
    source: "load-tier-25.json · 2026-08-29 · debug build",
  },
];

export function Numbers() {
  return (
    <section data-section="numbers" className="section numbers" aria-labelledby="numbers-title">
      <h2 id="numbers-title" className="marker-head">
        Measured, not promised.
      </h2>
      <p className="lede">
        Every number below came out of a script in the repository, run against the real server and
        engine. The server runs were debug builds; a release build is faster.
      </p>
      <ul className="measures">
        {MEASURES.map((m) => (
          <li key={m.source}>
            <p className="measure">
              <span className="figure">{m.figure}</span>
              <span className="unit">{m.unit}</span>
              <span className="then">{m.then}</span>
            </p>
            <p className="what">{m.what}</p>
            <p className="source">{m.source}</p>
          </li>
        ))}
      </ul>
      <p className="numbers-foot">
        The scripts and their output are in the <a href={`${REPO}/tree/main/marketing`}>repository</a>.
      </p>
    </section>
  );
}
