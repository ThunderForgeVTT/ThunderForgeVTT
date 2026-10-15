import { useEffect, useState } from "react";

// Wide enough for the tavern beside the headline. Narrower pages keep the
// marker room on the first screen and the tavern further down.
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
