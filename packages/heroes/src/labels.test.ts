import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { HERO_LABELS, labelProblems } from "./index.ts";

describe("the labels", () => {
  it("name every field, every choice and every race", () => {
    assert.deepEqual(labelProblems(), []);
  });

  it("fail by naming what a removed label belonged to", () => {
    const { wizard: _, ...headgear } = HERO_LABELS.choices.headgear!;
    const { race: __, ...fields } = HERO_LABELS.fields;
    const problems = labelProblems({
      ...HERO_LABELS,
      fields,
      choices: { ...HERO_LABELS.choices, headgear },
    });
    assert.deepEqual(problems.sort(), ["headgear.wizard", "race"]);
  });
});
