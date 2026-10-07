import { useEffect, useState } from "react";

// The repository's live star count, when GitHub answers (through the
// homepage's own cached proxy, see nginx.conf.template). Never a number we
// typed: on any failure the count is simply not shown.
let pending: Promise<number | null> | null = null;

function fetchStars(): Promise<number | null> {
  pending ??= fetch("/gh/repo")
    .then((r) => (r.ok ? r.json() : null))
    .then((body) =>
      body && typeof body.stargazers_count === "number" ? body.stargazers_count : null,
    )
    .catch(() => null);
  return pending;
}

export function useStars(): number | null {
  const [stars, setStars] = useState<number | null>(null);
  useEffect(() => {
    let live = true;
    void fetchStars().then((n) => live && setStars(n));
    return () => {
      live = false;
    };
  }, []);
  return stars;
}
