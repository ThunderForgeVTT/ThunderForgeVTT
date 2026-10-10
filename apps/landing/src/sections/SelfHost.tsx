import { useState } from "react";
import { DISCUSSIONS, REPO } from "../links.ts";

const COMMANDS = `git clone \\
  https://github.com/ThunderForgeVTT/ThunderForgeVTT.git
cd ThunderForgeVTT
docker compose up -d --build
docker compose logs app | grep /setup/`;

export function SelfHost() {
  const [copied, setCopied] = useState(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(COMMANDS);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* the commands stay selectable */
    }
  }
  return (
    <section id="self-host" data-section="self-host" className="section self-host" aria-labelledby="host-title">
      <div className="host-copy">
        <h2 id="host-title" className="marker-head">
          Yours to keep.
        </h2>
        <p className="lede">
          Self-hosting is free, forever, for your group or your whole community. It is open source
          under the AGPL, so no subscription, edition or upgrade can take your table away. One
          command builds it from source.
        </p>
        <p>
          You need Docker with the compose plugin and about ten minutes for the first build. Open
          the setup link the last command prints, make your administrator account, and the table is
          at <code>localhost:42080</code>.
        </p>
        <p>
          <a href={`${REPO}#quick-start`}>The full quick start</a> covers ports, mail and the rest.
        </p>
      </div>
      <div className="host-run">
      <figure className="terminal">
        <div className="terminal-bar">
          <figcaption>Four lines to your own table</figcaption>
          <button type="button" className="copy" onClick={() => void copy()}>
            {copied ? "Copied" : "Copy"}
          </button>
        </div>
        <pre>
          <code>{COMMANDS}</code>
        </pre>
      </figure>
      <aside className="early-note" aria-labelledby="early-title">
        <h3 id="early-title" className="early-title">
          Early days, and we're glad you're here.
        </h3>
        <p>
          ThunderForge is in active development, and the container changes fast. Expect new releases
          often, and changes that break things between them. Back up your database before you pull
          a new one, and read what changed first.
        </p>
        <p>
          If that sounds like fun, you're exactly who we want. Early adopters shape what this
          becomes: <a href={DISCUSSIONS}>tell us what breaks</a> and what your table needs.
        </p>
      </aside>
      </div>
    </section>
  );
}
