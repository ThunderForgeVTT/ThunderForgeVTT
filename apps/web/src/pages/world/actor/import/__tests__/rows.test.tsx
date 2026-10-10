import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { SheetContentChange, SheetFieldChange } from "@/api/sheetImport";
import { ContentRow } from "../ContentRow";
import { FieldRow } from "../FieldRow";
import { GraphQLRequestError } from "@/api/graphqlClient";
import { CrossChecks, KeptInPlayList, UnmappedList } from "../PlanNotes";
import { refusalProblem, SheetRefused } from "../refusal";
import {
  changeKind,
  contentText,
  fieldLabel,
  filterFields,
  goesToText,
  parseCorrection,
  shownValue,
} from "../rows";
import { stagedEntries } from "../../../../../../../../packs/systems/dnd5e/web/src/sheet/stagedEntries.ts";

/** Spec 048 T039: the review's rows, and the staged marks on the sheet. */

const field = (over: Partial<SheetFieldChange>): SheetFieldChange => ({
  path: "abilities.str",
  target: "ability_data.strength",
  old: 10,
  new: 16,
  certainty: "READ",
  reason: null,
  source: null,
  playState: false,
  ...over,
});

const content = (over: Partial<SheetContentChange>): SheetContentChange => ({
  kind: "spell",
  name: "Shield",
  resolution: "WORLD",
  worldId: "w1",
  stagedId: null,
  removed: false,
  ...over,
});

describe("FieldRow", () => {
  it("shows the old and the new value, the certainty and the source", () => {
    const html = renderToStaticMarkup(
      <ul>
        <FieldRow
          field={field({
            certainty: "UNCERTAIN",
            reason: "two marks overlap",
            source: { page: 1, x0: 0, y0: 0, x1: 1, y1: 1, text: "STR 16" },
          })}
        />
      </ul>,
    );
    expect(html).toContain('data-testid="sheet-import-field-abilities.str"');
    expect(html).toContain('data-certainty="uncertain"');
    expect(html).toContain("strength");
    expect(html).toMatch(/line-through[^>]*>10</);
    expect(html).toContain(">16<");
    expect(html).toContain("check this");
    expect(html).toContain("two marks overlap");
    expect(html).toContain("Page 1: “STR 16”");
  });

  it("shows one value when nothing changes, and marks kept play state", () => {
    const html = renderToStaticMarkup(
      <ul>
        <FieldRow field={field({ old: 16, playState: true })} />
      </ul>,
    );
    expect(html).not.toContain("line-through");
    expect(html).toContain("in play, kept");
  });

  it("names a field and shows an absent value", () => {
    expect(fieldLabel(field({ target: "actor.label" }))).toBe("Name");
    expect(fieldLabel(field({ target: "spell_data.spell_slots_max" }))).toBe(
      "spell slots max",
    );
    expect(shownValue(null)).toBe("—");
    expect(shownValue(["Common", "Elvish"])).toBe("Common, Elvish");
    expect(shownValue({ gp: 3 })).toBe('{"gp":3}');
  });
});

describe("the review (T049)", () => {
  it("marks a replaced value apart from a new one", () => {
    expect(changeKind(field({}))).toBe("overwrites");
    expect(changeKind(field({ old: null }))).toBe("new");
    expect(changeKind(field({ old: 16 }))).toBe("same");
    const html = renderToStaticMarkup(
      <ul>
        <FieldRow field={field({})} />
      </ul>,
    );
    expect(html).toContain('data-change="overwrites"');
    expect(html).toContain("will overwrite");
  });

  it("filters to what needs checking and to what was not read", () => {
    const fields = [
      field({ path: "a" }),
      field({ path: "b", certainty: "UNCERTAIN" }),
      field({ path: "c", certainty: "UNREAD" }),
      field({ path: "d", certainty: "CORRECTED" }),
    ];
    expect(filterFields(fields, "all")).toHaveLength(4);
    expect(filterFields(fields, "uncertain").map((f) => f.path)).toEqual(["b"]);
    expect(filterFields(fields, "unread").map((f) => f.path)).toEqual(["c"]);
  });

  it("offers a correction only where the reader was unsure", () => {
    const onCorrect = () => undefined;
    const read = renderToStaticMarkup(
      <ul>
        <FieldRow field={field({})} onCorrect={onCorrect} />
      </ul>,
    );
    expect(read).not.toContain("sheet-import-correct-");
    for (const certainty of ["UNCERTAIN", "UNREAD", "CORRECTED"] as const) {
      const html = renderToStaticMarkup(
        <ul>
          <FieldRow field={field({ certainty })} onCorrect={onCorrect} />
        </ul>,
      );
      expect(html).toContain(
        'data-testid="sheet-import-correct-abilities.str"',
      );
    }
  });

  it("shapes a correction like the value it replaces", () => {
    expect(parseCorrection(field({}), " 15 ")).toBe(15);
    expect(parseCorrection(field({}), "fifteen")).toBeUndefined();
    expect(parseCorrection(field({}), "")).toBeUndefined();
    expect(
      parseCorrection(
        field({ old: ["Common"], new: ["Elvish"] }),
        "Common, Dwarvish",
      ),
    ).toEqual(["Common", "Dwarvish"]);
    expect(
      parseCorrection(field({ old: null, new: { gp: 3 } }), '{"gp":5}'),
    ).toEqual({ gp: 5 });
    expect(
      parseCorrection(field({ old: null, new: { gp: 3 } }), "five gold"),
    ).toBeUndefined();
    expect(parseCorrection(field({ old: "Ann", new: "Anne" }), "Anna")).toBe(
      "Anna",
    );
    expect(parseCorrection(field({ old: null, new: null }), "12")).toBe(12);
  });

  it("shows both numbers of a cross-check", () => {
    const html = renderToStaticMarkup(
      <CrossChecks
        checks={[{ path: "derived.skill.perception", sheet: 7, derived: 5 }]}
      />,
    );
    expect(html).toContain(
      'data-testid="sheet-import-crosscheck-derived.skill.perception"',
    );
    expect(html).toContain("perception");
    expect(html).toMatch(/crosscheck-sheet">7</);
    expect(html).toMatch(/crosscheck-derived">5</);
    expect(renderToStaticMarkup(<CrossChecks checks={[]} />)).toBe("");
  });

  it("lists values in play with a box to take the sheet's", () => {
    const html = renderToStaticMarkup(
      <KeptInPlayList
        kept={[{ target: "health_data.current_hp", current: 4, sheet: 31 }]}
        overwrite={["health_data.current_hp"]}
        onToggle={() => undefined}
        disabled={false}
      />,
    );
    expect(html).toContain(
      'data-testid="sheet-import-kept-health_data.current_hp"',
    );
    expect(html).toContain("checked");
    expect(html).toContain("current hp: 4 in play, 31 on the sheet");
  });

  it("says where each untracked value goes", () => {
    expect(goesToText("trait_data.notes")).toBe("notes");
    const html = renderToStaticMarkup(
      <UnmappedList
        unmapped={[
          {
            path: "notes.Defenses",
            value: "Disease - Immunity",
            goesTo: "trait_data.notes",
          },
        ]}
      />,
    );
    expect(html).toContain("1 value the system does not track");
    expect(html).toContain("Disease - Immunity");
    expect(html).toContain("into notes");
  });

  it("keeps the refusal's code beside its sentence", () => {
    expect(
      refusalProblem(
        new SheetRefused("The sheet is protected.", "SHEET_ENCRYPTED"),
      ),
    ).toEqual({ text: "The sheet is protected.", code: "SHEET_ENCRYPTED" });
    expect(
      refusalProblem(
        new GraphQLRequestError("x", {
          errors: ["The file is over 10 MB."],
          codes: ["SHEET_TOO_LARGE"],
        }),
      ),
    ).toEqual({ text: "The file is over 10 MB.", code: "SHEET_TOO_LARGE" });
    expect(
      refusalProblem(new GraphQLRequestError("x", { codes: ["PLAN_CHANGED"] }))
        .text,
    ).toContain("changed while you were reviewing");
  });
});

describe("ContentRow", () => {
  it("says where each piece comes from", () => {
    expect(contentText(content({}))).toBe("in the world");
    expect(contentText(content({ resolution: "STAGED_NEW" }))).toBe(
      "new: awaits the GM",
    );
    expect(contentText(content({ resolution: "STAGED_EXISTING" }))).toBe(
      "already awaiting the GM",
    );
    expect(contentText(content({ resolution: "DIFFERS" }))).toContain(
      "differs",
    );
    expect(contentText(content({ removed: true }))).toContain("removed");
  });

  it("marks a staged piece and strikes a removed one", () => {
    const staged = renderToStaticMarkup(
      <ul>
        <ContentRow
          change={content({
            kind: "feat",
            name: "Alert",
            resolution: "STAGED_NEW",
          })}
        />
      </ul>,
    );
    expect(staged).toContain('data-testid="sheet-import-content-feat-Alert"');
    expect(staged).toContain('data-resolution="staged_new"');
    expect(staged).toContain("text-amber-600");
    const removed = renderToStaticMarkup(
      <ul>
        <ContentRow change={content({ removed: true })} />
      </ul>,
    );
    expect(removed).toContain('data-removed="yes"');
    expect(removed).toContain("line-through");
  });
});

describe("stagedEntries", () => {
  it("lists pending and declined links, and leaves adopted ones out", () => {
    expect(
      stagedEntries([
        { id: "a", kind: "feat", name: "Alert", state: "PENDING" },
        { id: "b", kind: "item", name: "Rope", state: "DECLINED" },
        { id: "c", kind: "spell", name: "Shield", state: "ADOPTED" },
        { id: "d", kind: "class_feature", name: "Rage", state: "PENDING" },
      ]),
    ).toEqual([
      { id: "a", kind: "feat", name: "Alert", staged: "pending" },
      { id: "b", kind: "item", name: "Rope", staged: "declined" },
      { id: "d", kind: "feature", name: "Rage", staged: "pending" },
    ]);
  });
});
