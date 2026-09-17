/**
 * The builder on a page of its own: a preset to start from, and the files a
 * hero leaves as. No server, no account, nothing saved but what the browser
 * downloads (FR-012).
 */
import { useState } from "react";
import {
  HeroBuilder,
  heroFiles,
  fileStem,
  parseHeroText,
  saveHeroFile,
  type HeroProblem,
} from "@thunderforge/hero-builder";
import {
  PRESET_HEROES,
  presetSource,
  type HeroSpec,
} from "@thunderforge/heroes";

const BLANK: HeroSpec = { name: "New hero" };

const button = "tfhb-app-button";

export function App() {
  const [preset, setPreset] = useState(PRESET_HEROES[0]?.slug ?? "");
  const [opened, setOpened] = useState<HeroSpec>(
    () => PRESET_HEROES[0]?.spec ?? BLANK,
  );
  // Bumped whenever a new hero is opened, so the builder starts over from it.
  const [generation, setGeneration] = useState(0);
  const [current, setCurrent] = useState<HeroSpec>(opened);
  const [importText, setImportText] = useState("");
  const [importProblems, setImportProblems] = useState<HeroProblem[]>([]);
  const [status, setStatus] = useState("");

  const open = (spec: HeroSpec) => {
    setOpened(spec);
    setCurrent(spec);
    setGeneration((n) => n + 1);
  };

  const importSpec = (text: string) => {
    const result = parseHeroText(text);
    if (!result.ok) {
      setImportProblems(result.problems);
      setStatus("");
      return;
    }
    setImportProblems([]);
    setPreset("");
    open(result.spec);
    setStatus(`Imported ${result.spec.name}.`);
  };

  const files = heroFiles(current);
  const copyPreset = async () => {
    const source = presetSource(fileStem(current.name), current);
    try {
      await navigator.clipboard.writeText(source);
      setStatus("Copied as a preset entry.");
    } catch {
      setStatus(
        "The browser would not copy; nothing was put on the clipboard.",
      );
    }
  };

  return (
    <div className="tfhb-app">
      <style>{APP_CSS}</style>
      <header className="tfhb-app-header">
        <h1>Hero builder</h1>
        <label className="tfhb-app-field">
          <span>Start from</span>
          <select
            value={preset}
            data-testid="preset-picker"
            onChange={(event) => {
              const slug = event.target.value;
              setPreset(slug);
              open(
                PRESET_HEROES.find((entry) => entry.slug === slug)?.spec ??
                  BLANK,
              );
            }}
          >
            <option value="">A new hero</option>
            {PRESET_HEROES.map((entry) => (
              <option key={entry.slug} value={entry.slug}>
                {entry.spec.name}
              </option>
            ))}
          </select>
        </label>
      </header>
      <main>
        <HeroBuilder
          key={generation}
          initialSpec={opened}
          idPrefix={`hb${generation}`}
          initialRace={null}
          onChange={setCurrent}
          actions={
            <>
              <button
                type="button"
                className={button}
                data-testid="export-portrait"
                onClick={() => saveHeroFile(files.portrait)}
              >
                Export portrait
              </button>
              <button
                type="button"
                className={button}
                data-testid="export-token"
                onClick={() => saveHeroFile(files.token)}
              >
                Export token
              </button>
              <button
                type="button"
                className={button}
                data-testid="export-json"
                onClick={() => saveHeroFile(files.json)}
              >
                Export spec
              </button>
              <button
                type="button"
                className={button}
                data-testid="copy-preset"
                onClick={copyPreset}
              >
                Copy as preset
              </button>
            </>
          }
        />
        <section className="tfhb-app-import" aria-labelledby="import-heading">
          <h2 id="import-heading">Import a spec</h2>
          <p>
            Paste a hero spec, or load a <code>.hero.json</code> file. Drawings
            cannot be imported — only specs.
          </p>
          <label className="tfhb-app-field">
            <span>Spec</span>
            <textarea
              rows={5}
              value={importText}
              data-testid="import-text"
              onChange={(event) => setImportText(event.target.value)}
            />
          </label>
          <div className="tfhb-app-row">
            <button
              type="button"
              className={button}
              data-testid="import-apply"
              onClick={() => importSpec(importText)}
            >
              Import
            </button>
            <label className={button}>
              Load file
              <input
                type="file"
                accept="application/json,.json"
                data-testid="import-file"
                className="tfhb-app-hidden-input"
                onChange={async (event) => {
                  const file = event.target.files?.[0];
                  event.target.value = "";
                  if (file) importSpec(await file.text());
                }}
              />
            </label>
          </div>
          {importProblems.length > 0 && (
            <ul
              role="alert"
              className="tfhb-app-problems"
              data-testid="import-problems"
            >
              {importProblems.map((problem, index) => (
                <li key={index} data-field={problem.field}>
                  {problem.field === ""
                    ? problem.message
                    : `${problem.field}: ${problem.message}`}
                </li>
              ))}
            </ul>
          )}
        </section>
        <p role="status" data-testid="status" className="tfhb-app-status">
          {status}
        </p>
      </main>
    </div>
  );
}

const APP_CSS = `
.tfhb-app { font: 15px/1.4 system-ui, sans-serif; padding: 16px; max-width: 1200px; margin: 0 auto; }
.tfhb-app *, .tfhb-app *::before, .tfhb-app *::after { box-sizing: border-box; }
.tfhb-app h1 { font-size: 22px; margin: 0; }
.tfhb-app h2 { font-size: 17px; margin: 0 0 4px; }
.tfhb-app-header { display: flex; flex-wrap: wrap; align-items: end; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
.tfhb-app-field { display: grid; gap: 4px; min-width: 0; }
.tfhb-app-field > span { font-size: 13px; color: #b8ad9c; }
.tfhb-app select, .tfhb-app textarea {
  min-height: 44px; padding: 8px 10px; max-width: 100%; font: inherit;
  color: #f2e8d8; background: #2a2533; border: 1px solid #4a4257; border-radius: 8px;
}
.tfhb-app textarea { width: 100%; font-family: ui-monospace, monospace; }
.tfhb-app-button {
  display: inline-flex; align-items: center; min-height: 44px; min-width: 44px; padding: 0 14px;
  font: inherit; color: #f2e8d8; background: #2a2533; border: 1px solid #4a4257; border-radius: 8px; cursor: pointer;
}
.tfhb-app-button:focus-within, .tfhb-app select:focus-visible, .tfhb-app textarea:focus-visible { outline: 3px solid #f4c542; outline-offset: 2px; }
.tfhb-app-hidden-input { position: absolute; opacity: 0; width: 1px; height: 1px; }
.tfhb-app-row { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
.tfhb-app-import { margin-top: 24px; padding-top: 16px; border-top: 1px solid #4a4257; }
.tfhb-app-problems { color: #ff8a7a; }
.tfhb-app-status { min-height: 1.4em; color: #b8ad9c; }
`;
