import { Check } from "../ink/Strokes.tsx";

const BURNED = [
  "A major version upgrade that broke everything, and sometimes took the data with it.",
  "A new edition pushed over the source material we loved.",
  "Paid modules that kept collecting while their updates fell further and further behind.",
];

const WANTED = ["Easy to host.", "Easy to use.", "Resilient."];

export function Dream() {
  return (
    <section id="dream" data-section="dream" className="section dream" aria-labelledby="dream-title">
      <div className="dream-burned">
        <h2 id="dream-title" className="marker-head">
          Burned one too many times.
        </h2>
        <p className="lede">
          I love what Foundry, Roll20 and the others have done for us. They just never fit the game
          I have been trying to run. Somewhere along the way the tools started getting between my
          table and the story.
        </p>
        <ul className="struck" aria-label="What went wrong">
          {BURNED.map((line) => (
            <li key={line}>
              <span className="strike">{line}</span>
            </li>
          ))}
        </ul>
      </div>
      <div className="dream-wanted">
        <p className="lede">
          We play to escape reality for a little while. The table should help the fantasy happen,
          not weigh it down. So I am building the one I wanted:
        </p>
        <ul className="checked">
          {WANTED.map((line) => (
            <li key={line}>
              <Check />
              {line}
            </li>
          ))}
          <li>
            <Check />
            In person, online, or a mix of both at the same table.
          </li>
          <li>
            <Check />
            Yours to keep. No upgrade, edition or subscription can take it away.
          </li>
        </ul>
        <p className="signed">
          <span className="marker-sign">MBRound18</span>
          <span>creator, and the GM who got burned</span>
        </p>
      </div>
    </section>
  );
}
