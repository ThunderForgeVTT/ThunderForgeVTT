import { Ring } from "../ink/Strokes.tsx";

const SYSTEMS: { name: string; field?: boolean }[] = [
  { name: "5E System Core (SRD 5.2.1)", field: true },
  { name: "Roll for Shoes", field: true },
  { name: "Pathfinder Second Edition" },
  { name: "Blades in the Dark / Forged in the Dark" },
  { name: "Fate Core" },
  { name: "Cypher System" },
  { name: "Year Zero Engine" },
  { name: "Genie, our own house system" },
  { name: "Basic Game System" },
];

export function Systems() {
  return (
    <section className="section systems" aria-labelledby="systems-title">
      <div className="systems-copy">
        <h2 id="systems-title" className="marker-head">
          Bring your system.
        </h2>
        <p className="lede">
          A game system is a pack that plugs in, never a fork of the app. Nine are in the box today.
          The two circled are being field-tested first: a 5e group should be able to sign up, build
          a table, put characters on a map, roll, fight, and come back next week.
        </p>
      </div>
      <ol className="system-list">
        {SYSTEMS.map((s) => (
          <li key={s.name} className={s.field ? "is-field" : undefined}>
            <span className="system-name">
              {s.name}
              {s.field && <Ring seed={s.name.length} />}
            </span>
            {s.field && <span className="system-tag">field test first</span>}
          </li>
        ))}
      </ol>
    </section>
  );
}
