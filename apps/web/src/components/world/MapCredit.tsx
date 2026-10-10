import { useState, type FocusEvent } from "react";
import type { MapCredit as Credit } from "@/types/scene";

/**
 * Whose a base map is, wherever one is shown (spec 088 FR-030).
 *
 * The maps are MBRound18's, under CC BY-SA 4.0, and that licence asks for
 * the author, the licence and where the work came from every time it is
 * shown. So the full credit names all three as links and repeats the
 * ShareAlike sentence; nothing here is optional decoration.
 */

const link = "underline underline-offset-2 hover:text-foreground";

export function FullMapCredit({
  credit,
  className,
}: {
  credit: Credit;
  className?: string;
}) {
  return (
    <p className={className} data-testid="map-credit">
      Map by <strong>{credit.author}</strong>, from{" "}
      <a
        className={link}
        href={credit.source}
        target="_blank"
        rel="noreferrer"
        data-testid="map-credit-source"
      >
        {sourceName(credit.source)}
      </a>{" "}
      (
      <a
        className={link}
        href={credit.catalog}
        target="_blank"
        rel="noreferrer"
        data-testid="map-credit-catalog"
      >
        catalog
      </a>
      ), under{" "}
      <a
        className={link}
        href={credit.licenceUrl}
        target="_blank"
        rel="license noreferrer"
        data-testid="map-credit-licence"
      >
        {credit.licence}
      </a>
      . {credit.shareAlike}
    </p>
  );
}

/**
 * The board's credit: one short line that opens into the full credit while
 * it is hovered or anything inside it has focus, so the links are reachable
 * by keyboard and the board is not covered by a paragraph the rest of the
 * time.
 */
export function MapCreditLine({ credit }: { credit: Credit }) {
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const open = hovered || focused;

  const leave = (event: FocusEvent<HTMLDivElement>) => {
    // Moving focus from the line to one of its own links keeps it open.
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
      setFocused(false);
    }
  };

  return (
    <div
      className="pointer-events-auto max-w-sm rounded-md bg-background/80 px-2 py-1 text-xs text-muted-foreground shadow-sm backdrop-blur"
      data-testid="board-map-credit"
      data-open={open ? "true" : "false"}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)}
      onBlur={leave}
    >
      {/*
       * The short line stays mounted while the credit is open. Swapping it
       * out would unmount the element holding focus, and the browser would
       * drop focus to the page and close the credit again.
       */}
      <span
        tabIndex={0}
        role="button"
        aria-expanded={open}
        data-testid="board-map-credit-short"
      >
        Map: {credit.author}, {credit.licence}
      </span>
      {open ? <FullMapCredit credit={credit} className="mt-1" /> : null}
    </div>
  );
}

/** "vtt-maps" from `https://github.com/mbround18/vtt-maps`. */
function sourceName(url: string): string {
  try {
    const parts = new URL(url).pathname.split("/").filter(Boolean);
    return parts.at(-1) ?? url;
  } catch {
    return url;
  }
}
