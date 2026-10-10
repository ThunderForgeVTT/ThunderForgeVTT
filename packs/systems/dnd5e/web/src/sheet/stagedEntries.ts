import type { LinkedEntry } from "./LinkedContent";

/** The host's `ActorStagedLink`, as much of it as the sheet reads. */
export interface StagedLinkLike {
  id: string;
  kind: string;
  name: string;
  state: "PENDING" | "ADOPTED" | "DECLINED";
}

const KINDS: LinkedEntry["kind"][] = [
  "spell",
  "feature",
  "species_trait",
  "feat",
  "item",
];

/**
 * A staged link as the sheet lists it. An adopted piece is the world's own
 * and shows in the host's panels, so it is not listed here.
 */
export function stagedEntries(links: StagedLinkLike[]): LinkedEntry[] {
  return links.flatMap((link): LinkedEntry[] => {
    if (link.state === "ADOPTED") return [];
    const kind = KINDS.find((k) => k === link.kind) ?? "feature";
    return [
      {
        id: link.id,
        kind,
        name: link.name,
        staged: link.state === "DECLINED" ? "declined" : "pending",
      },
    ];
  });
}
