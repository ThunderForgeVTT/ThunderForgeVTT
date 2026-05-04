import { useCallback, useMemo } from "react";

type DicebearFormat = "svg" | "png";
type DicebearOptions = Record<string, string | number | boolean | undefined>;

const DICEBEAR_BASE = "https://api.dicebear.com/9.x";

function buildDicebearUrl(
  style: string,
  seed: string,
  format: DicebearFormat,
  options: DicebearOptions = {},
) {
  const searchParams = new URLSearchParams({ seed });

  Object.entries(options).forEach(([key, value]) => {
    if (value === undefined) {
      return;
    }

    searchParams.set(key, String(value));
  });

  return `${DICEBEAR_BASE}/${style}/${format}?${searchParams.toString()}`;
}

async function downloadFromUrl(url: string, filename: string) {
  const response = await fetch(url);
  const blob = await response.blob();
  const blobUrl = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = blobUrl;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(blobUrl);
}

export function useAvatar(seed: string) {
  const avatarSvgUrl = useMemo(
    () =>
      buildDicebearUrl("adventurer-neutral", seed, "svg", {
        backgroundType: "gradientLinear",
        backgroundColor: "f5e9ce,c9a25c,5c3b78",
      }),
    [seed],
  );

  const avatarPngUrl = useMemo(
    () =>
      buildDicebearUrl("adventurer-neutral", seed, "png", {
        backgroundType: "gradientLinear",
        backgroundColor: "f5e9ce,c9a25c,5c3b78",
      }),
    [seed],
  );

  const tokenSvgUrl = useMemo(
    () =>
      buildDicebearUrl("lorelei", seed, "svg", {
        backgroundType: "gradientLinear",
        backgroundColor: "274634,5c3b78,120f0b",
        radius: 50,
      }),
    [seed],
  );

  const tokenPngUrl = useMemo(
    () =>
      buildDicebearUrl("lorelei", seed, "png", {
        backgroundType: "gradientLinear",
        backgroundColor: "274634,5c3b78,120f0b",
        radius: 50,
      }),
    [seed],
  );

  const exportAvatar = useCallback(
    async (format: DicebearFormat = "svg") => {
      const url = format === "svg" ? avatarSvgUrl : avatarPngUrl;
      await downloadFromUrl(url, `thunderforge-avatar-${seed}.${format}`);
    },
    [avatarPngUrl, avatarSvgUrl, seed],
  );

  const exportToken = useCallback(
    async (format: DicebearFormat = "png") => {
      const url = format === "svg" ? tokenSvgUrl : tokenPngUrl;
      await downloadFromUrl(url, `thunderforge-token-${seed}.${format}`);
    },
    [seed, tokenPngUrl, tokenSvgUrl],
  );

  return {
    avatarSvgUrl,
    avatarPngUrl,
    tokenSvgUrl,
    tokenPngUrl,
    exportAvatar,
    exportToken,
  };
}
