import { useEffect, useState } from "react";

// Wide enough for the tavern beside the headline, and for the extras that
// sit beside the page's sections (the campfire, the dungeon on the table).
const WIDE = "(min-width: 1200px)";

export function useWide(): boolean {
  const [wide, setWide] = useState(() => matchMedia(WIDE).matches);
  useEffect(() => {
    const mq = matchMedia(WIDE);
    const change = () => setWide(mq.matches);
    mq.addEventListener("change", change);
    return () => mq.removeEventListener("change", change);
  }, []);
  return wide;
}
