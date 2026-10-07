import { useEffect, useRef, useState, type FormEvent } from "react";

// The dice crate the server rolls with, compiled to WebAssembly and running
// right here. Loaded with this section, never with the first viewport.

type Sides = { Numeric: number } | "Fate" | "Coin";
interface Die {
  sides: Sides;
  rolls: number[];
  kept: boolean;
  final_value: number;
}
interface Resolution {
  formula: string;
  dice: Die[];
  kind: { Total: number } | { SuccessCount: number };
}

type DiceModule = typeof import("@thunderforge/dice");
let loaded: Promise<DiceModule> | null = null;
function dice(): Promise<DiceModule> {
  loaded ??= import("@thunderforge/dice").then(async (m) => {
    await m.default();
    return m;
  });
  return loaded;
}

const PRESETS = [
  { label: "Attack", formula: "1d20+5" },
  { label: "Advantage", formula: "2d20kh1+5" },
  { label: "Ability score", formula: "4d6kh3" },
  { label: "Fireball", formula: "8d6" },
  { label: "Fate", formula: "4dF" },
];

function shape(sides: Sides): string {
  // Outline per die, in a 40 x 40 box.
  if (sides === "Fate" || sides === "Coin") return "M6 6H34V34H6Z";
  switch (sides.Numeric) {
    case 4:
      return "M20 4L36 34H4Z";
    case 6:
      return "M6 6H34V34H6Z";
    case 8:
      return "M20 3L37 20L20 37L3 20Z";
    case 10:
    case 100:
      return "M20 3L36 16L20 37L4 16Z";
    case 12:
      return "M20 3L37 15L31 36H9L3 15Z";
    case 20:
      return "M20 2L36 11V29L20 38L4 29V11Z";
    default:
      return "M20 3A17 17 0 1 1 19.9 3Z";
  }
}

function face(sides: Sides, v: number): string {
  if (sides === "Fate") return v > 0 ? "+" : v < 0 ? "−" : "·";
  if (sides === "Coin") return v ? "H" : "T";
  return String(v);
}

function seed() {
  return crypto.getRandomValues(new Uint32Array(4));
}

export default function DiceTray() {
  const [formula, setFormula] = useState("2d20kh1+5");
  const [result, setResult] = useState<Resolution | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [ready, setReady] = useState(false);
  const count = useRef(0);

  useEffect(() => {
    void dice()
      .then(() => setReady(true))
      .catch(() => setError("The dice could not load in this browser."));
  }, []);

  async function roll(f: string) {
    setFormula(f);
    try {
      const m = await dice();
      if (!m.validateFormula(f)) {
        setError(`"${f}" is not a formula the dice understand.`);
        return;
      }
      const r = JSON.parse(m.roll(f, "{}", seed())) as Resolution;
      setError("");
      setResult(r);
      count.current += 1;
      const total = "Total" in r.kind ? r.kind.Total : `${r.kind.SuccessCount} successes`;
      setHistory((h) => [`${r.formula} → ${total}`, ...h].slice(0, 4));
    } catch {
      setError(`"${f}" is not a formula the dice understand.`);
    }
  }

  function submit(e: FormEvent) {
    e.preventDefault();
    void roll(formula.trim());
  }

  const total = result ? ("Total" in result.kind ? result.kind.Total : result.kind.SuccessCount) : null;

  return (
    <div className="tray">
      <div className="tray-controls">
        <div className="presets" role="group" aria-label="Example rolls">
          {PRESETS.map((p) => (
            <button
              key={p.formula}
              type="button"
              className="chip"
              onClick={() => void roll(p.formula)}
              disabled={!ready}
            >
              <span>{p.label}</span>
              <code>{p.formula}</code>
            </button>
          ))}
        </div>
        <form className="formula" onSubmit={submit}>
          <label htmlFor="formula">Or write your own</label>
          <div className="formula-row">
            <input
              id="formula"
              value={formula}
              onChange={(e) => setFormula(e.target.value)}
              spellCheck={false}
              autoComplete="off"
              inputMode="text"
            />
            <button type="submit" className="ink-btn ink-btn--black" disabled={!ready}>
              Roll
            </button>
          </div>
        </form>
      </div>

      <div className="tray-felt" aria-live="polite">
        {!result && !error && (
          <p className="tray-empty">{ready ? "Pick a roll." : "Loading the dice…"}</p>
        )}
        {error && <p className="tray-error">{error}</p>}
        {result && !error && (
          <>
            <ul className="dice" key={count.current} aria-label="Dice rolled">
              {result.dice.flatMap((d, i) =>
                d.rolls.map((v, j) => (
                  <li
                    key={`${i}-${j}`}
                    className={d.kept ? "die" : "die die--dropped"}
                    aria-label={`${face(d.sides, v)}${d.kept ? "" : ", dropped"}`}
                  >
                    <svg viewBox="0 0 40 40" aria-hidden="true">
                      <path d={shape(d.sides)} />
                      {!d.kept && <path className="die-strike" d="M5 35L35 5" />}
                    </svg>
                    <span aria-hidden="true">{face(d.sides, v)}</span>
                  </li>
                )),
              )}
            </ul>
            <p className="total">
              <span className="total-num">{total}</span>
              <code>{result.formula}</code>
            </p>
          </>
        )}
        {history.length > 1 && (
          <ol className="history" aria-label="Earlier rolls">
            {history.slice(1).map((h, i) => (
              <li key={i}>{h}</li>
            ))}
          </ol>
        )}
      </div>
    </div>
  );
}
