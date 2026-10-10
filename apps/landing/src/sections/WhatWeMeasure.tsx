import { TELEMETRY_GUIDE } from "../links.ts";

/**
 * Spec 086 FR-024: what this page and the demo send, in Appendix A.5's
 * words. `apps/thunderforge/src/telemetry/disclosure_tests.rs` compares this
 * text with the appendix, so a change here is a change there.
 */
export function WhatWeMeasure() {
  return (
    <section
      id="telemetry"
      data-section="telemetry"
      className="section measure"
      aria-labelledby="telemetry-title"
    >
      <h2 id="telemetry-title" className="marker-head">
        What we measure.
      </h2>
      <div className="measure-copy">
        <p>
          We count what happens, not what you say. This page and the demo send us anonymous usage:
          which pages you view, how far you get in the demo, how long things take, errors, your
          browser family and a rough device class, and a random id that is forgotten when you close
          the tab. We never receive what you type, roll, name or upload, cookies, or any account.
          Our endpoint uses your IP address only in memory, to stop one sender flooding it, and
          never stores or passes it on. It notes which site the report came from, and your country
          when our network edge supplies it. It goes to <code>telemetry.thunderforge.dev</code> and
          is kept for 14 days. If your browser sends Global Privacy Control or Do Not Track, we
          receive errors only.
        </p>
        <p>
          ThunderForge you run yourself sends us the same kind of anonymous diagnostics by default,
          so we hear about the bugs we would otherwise never see. Its operator can send them to
          their own collector instead, or turn them off with <code>TELEMETRY=false</code>.{" "}
          <a href={TELEMETRY_GUIDE}>How</a>
        </p>
      </div>
    </section>
  );
}
