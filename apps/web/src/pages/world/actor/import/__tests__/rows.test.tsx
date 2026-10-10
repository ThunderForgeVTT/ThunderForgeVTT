import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { SheetContentChange, SheetFieldChange } from "@/api/sheetImport";
import { ContentRow } from "../ContentRow";
import { FieldRow } from "../FieldRow";
import { contentText, fieldLabel, shownValue } from "../rows";
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
