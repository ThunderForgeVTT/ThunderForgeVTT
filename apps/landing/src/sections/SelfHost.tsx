import { useState } from "react";
import { REPO } from "../links.ts";

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
    <section id="self-host" className="section self-host" aria-labelledby="host-title">
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
    </section>
  );
}
