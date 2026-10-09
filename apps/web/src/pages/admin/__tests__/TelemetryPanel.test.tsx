import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TelemetryStatus } from "@/api/telemetry";
import {
  ANONYMOUS_TEXT,
  CHANGE_TEXT,
  OFF_TEXT,
  TelemetryPanelView,
  redirectedText,
} from "../TelemetryPanel";

const SPEC = readFileSync(
  resolve(__dirname, "../../../../../../specs/086-full-telemetry/spec.md"),
  "utf8",
);

/** Appendix A.4, with markup and line wrapping removed. */
const A4 = (() => {
  const start = SPEC.indexOf("### A.4");
  const end = SPEC.indexOf("### A.5", start);
  return words(SPEC.slice(start, end));
})();

function words(text: string): string {
  return text
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/[`*]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

function text(html: string): string {
  return words(
    html
      .replace(/<[^>]+>/g, " ")
      .replace(/&#x27;/g, "'")
      .replace(/&quot;/g, '"')
      .replace(/&amp;/g, "&"),
  );
}

const OFF: TelemetryStatus = {
  enabled: false,
  serverExporting: false,
  serverTier: "off",
  serverEndpoint: null,
  browserEnabled: false,
  browserTier: "off",
  browserEndpoint: null,
  instanceId: "0b0e8c56-1111-4222-8333-944455556666",
};

describe("TelemetryPanel (Appendix A.4)", () => {
  it("holds each state's sentence verbatim", () => {
    expect(A4).toContain(`"${ANONYMOUS_TEXT}"`);
    expect(A4).toContain(`"${OFF_TEXT}"`);
    expect(A4).toContain(`"${redirectedText("<destination>")}"`);
    expect(A4).toContain(`"${CHANGE_TEXT} What is sent"`);
  });

  it("shows off, both rows, the install id and how to change it", () => {
    const html = text(
      renderToStaticMarkup(<TelemetryPanelView status={OFF} />),
    );
    expect(html).toContain(OFF_TEXT);
    expect(html).toContain("Server off off nowhere");
    expect(html).toContain("Browsers off off nowhere");
    expect(html).toContain(`Install id: ${OFF.instanceId}`);
    expect(html).toContain(`${CHANGE_TEXT} What is sent`);
  });

  it("is anonymous when either row reaches the project", () => {
    const html = text(
      renderToStaticMarkup(
        <TelemetryPanelView
          status={{
            ...OFF,
            enabled: true,
            browserEnabled: true,
            browserTier: "anonymous",
            browserEndpoint: "https://telemetry.thunderforge.dev",
          }}
        />,
      ),
    );
    expect(html).toContain(ANONYMOUS_TEXT);
    expect(html).toContain(
      "Browsers on anonymous https://telemetry.thunderforge.dev",
    );
  });

  it("names the operator's collector when redirected", () => {
    const html = text(
      renderToStaticMarkup(
        <TelemetryPanelView
          status={{
            ...OFF,
            enabled: true,
            serverExporting: true,
            serverTier: "full",
            serverEndpoint: "https://otel.example.org",
          }}
        />,
      ),
    );
    expect(html).toContain(redirectedText("https://otel.example.org"));
  });

  it("offers no toggle", () => {
    const html = renderToStaticMarkup(<TelemetryPanelView status={OFF} />);
    expect(html).not.toMatch(/<(input|button|select)\b/);
  });
});
