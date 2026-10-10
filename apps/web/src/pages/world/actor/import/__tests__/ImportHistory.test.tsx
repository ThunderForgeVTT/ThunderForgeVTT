import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ActorImportRecord } from "@/api/sheetImport";
import { HistoryRow, historyText } from "../ImportHistory";

/** Spec 048 T078: the history's rows, and who is offered what. */

const record = (over: Partial<ActorImportRecord>): ActorImportRecord => ({
  id: "i1",
  kind: "import",
  appliedAt: "2026-10-01T12:00:00Z",
  appliedBy: { id: "u1", username: "wren", displayName: "Wren Player" },
  versionNo: 1,
  restoredFrom: null,
  correctedFields: [],
  fileAvailable: false,
  versionId: "v1",
  ...over,
});

const row = (r: ActorImportRecord, isGm: boolean, all = [r]) =>
  renderToStaticMarkup(
    <HistoryRow
      record={r}
      all={all}
      isGm={isGm}
      busy={false}
      confirming={false}
      onAsk={() => undefined}
      onConfirm={() => undefined}
      onCancel={() => undefined}
    />,
  );

describe("ImportHistory (T078)", () => {
  it("says who brought which version, and what a rollback went back to", () => {
    const first = record({});
    const back = record({
      id: "r1",
      kind: "rollback",
      versionNo: null,
      versionId: null,
      restoredFrom: "i1",
      appliedBy: { id: "g", username: "gm", displayName: "The GM" },
    });
    expect(historyText(first, [first])).toBe(
      "Version 1 brought in by Wren Player",
    );
    expect(historyText(back, [back, first])).toBe(
      "Rolled back to before version 1 by The GM",
    );
  });

  it("offers the download only where the server allows it", () => {
    expect(row(record({ fileAvailable: true }), false)).toContain(
      "/api/sheet-imports/v1/file",
    );
    expect(row(record({ fileAvailable: false }), true)).not.toContain(
      "import-history-download",
    );
  });

  it("offers a rollback to the GM alone, and never on a rollback", () => {
    expect(row(record({}), true)).toContain("import-history-rollback");
    expect(row(record({}), false)).not.toContain("import-history-rollback");
    expect(
      row(record({ kind: "rollback", restoredFrom: "x" }), true),
    ).not.toContain("import-history-rollback");
  });
});
