import { describe, expect, it } from "vitest";
import type { AccountNotice } from "@/api/standing";
import { noticeText } from "./noticeText";

function notice(kind: string, payload: Record<string, unknown>): AccountNotice {
  return {
    id: "notice-1",
    kind,
    payload,
    subjectRef: null,
    createdAt: "2026-09-10T00:00:00Z",
    readAt: null,
  } as unknown as AccountNotice;
}

describe("spec 039 US6: what an adopter is told", () => {
  it("says a disabled copy is not an accusation, not a strike, and not a deletion", () => {
    const text = noticeText(
      notice("adopted_copy_disabled", { copies: ["Moonblade"] }),
    );
    expect(text).toContain("Moonblade was disabled");
    expect(text).toContain("You are not accused of anything");
    expect(text).toContain("not a strike against you");
    expect(text).toContain("Nothing was deleted");
    expect(text).toContain("comes back on its own");
  });

  it("names nobody on the other side of the notice", () => {
    const text = noticeText(
      notice("adopted_copy_disabled", { copies: ["Moonblade"] }),
    ).toLowerCase();
    for (const word of ["claimant", "infring", "shared by", "you shared"]) {
      expect(text).not.toContain(word);
    }
  });

  it("speaks of several copies in the plural", () => {
    const text = noticeText(
      notice("adopted_copy_disabled", { copies: ["Moonblade", "Sunshield"] }),
    );
    expect(text).toContain("Moonblade, Sunshield were disabled");
    expect(text).toContain("they come back on their own");
  });

  it("tells the adopter a copy came back without their asking", () => {
    const text = noticeText(
      notice("adopted_copy_restored", { copies: ["Moonblade"] }),
    );
    expect(text).toContain("Moonblade is back");
    expect(text).toContain("nobody had to ask");
  });

  it("tells the sharer the takedown reached copies, and that their takers are not accused", () => {
    const text = noticeText(notice("share_taken_down", { copiesDisabled: 2 }));
    expect(text).toContain("reached 2 copies");
    expect(text).toContain("Nobody who took a copy is accused of anything");
  });
});
