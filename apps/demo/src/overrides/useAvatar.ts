/**
 * Stands in for the web app's `useAvatar` (`vite.config.mts` aliases it
 * here).
 *
 * The real one asks an avatar service on the internet for a picture. The demo
 * asks nobody for anything (SC-003), so it draws its own: a disc in two of
 * the theme's colours, chosen by the seed, as an image the page already
 * holds.
 */
import { useCallback, useMemo } from "react";

const PAIRS = [
  ["#c9a25c", "#5c3b78"],
  ["#274634", "#c9a25c"],
  ["#5c3b78", "#274634"],
  ["#8a3b2e", "#c9a25c"],
  ["#2e5f8a", "#f5e9ce"],
];

function hash(seed: string): number {
  let value = 0;
  for (const character of seed) {
    value = (value * 31 + character.charCodeAt(0)) >>> 0;
  }
  return value;
}

function disc(seed: string, round: boolean): string {
  const picked = hash(seed);
  const [from, to] = PAIRS[picked % PAIRS.length];
  const turn = picked % 360;
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">` +
    `<defs><linearGradient id="g" gradientTransform="rotate(${turn} .5 .5)">` +
    `<stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/>` +
    `</linearGradient></defs>` +
    `<rect width="64" height="64" rx="${round ? 32 : 0}" fill="url(#g)"/>` +
    `<circle cx="32" cy="26" r="11" fill="#120f0b" fill-opacity=".55"/>` +
    `<path d="M10 64a22 20 0 0 1 44 0z" fill="#120f0b" fill-opacity=".55"/>` +
    `</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

/** The picture a token with no art of its own is given. */
export function tokenFallbackArt(seed: string): string {
  return disc(seed, false);
}

function download(url: string, filename: string) {
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
}

export function useAvatar(seed: string) {
  const avatar = useMemo(() => disc(seed, false), [seed]);
  const token = useMemo(() => disc(seed, true), [seed]);

  // One drawing, so one format: what is offered as a PNG elsewhere is the
  // same SVG here.
  const exportAvatar = useCallback(
    async (_format: "svg" | "png" = "svg") => {
      download(avatar, `thunderforge-avatar-${seed}.svg`);
    },
    [avatar, seed],
  );
  const exportToken = useCallback(
    async (_format: "svg" | "png" = "png") => {
      download(token, `thunderforge-token-${seed}.svg`);
    },
    [seed, token],
  );

  return {
    avatarSvgUrl: avatar,
    avatarPngUrl: avatar,
    tokenSvgUrl: token,
    tokenPngUrl: token,
    exportAvatar,
    exportToken,
  };
}
