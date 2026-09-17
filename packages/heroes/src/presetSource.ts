/**
 * A hero as a `presets.ts` entry, ready to paste (FR-014).
 *
 * Writes `SKIN_TONES.peach` rather than "#f1c6a0" where the skin is a named
 * tone, so a pasted entry reads like its neighbours.
 */
import { minimalSpec } from "./minimal.ts";
import { SKIN_TONES, type HeroSpec } from "./spec.ts";

export function presetSource(slug: string, spec: HeroSpec): string {
  const lines = Object.entries(minimalSpec(spec)).map(([field, value]) => {
    const tone =
      field === "skin" && typeof value === "string"
        ? Object.entries(SKIN_TONES).find(
            ([, hex]) => hex === value.toLowerCase(),
          )?.[0]
        : undefined;
    const written = tone ? `SKIN_TONES.${tone}` : JSON.stringify(value);
    return `      ${field}: ${written},`;
  });
  return `  {
    slug: ${JSON.stringify(slug)},
    spec: {
${lines.join("\n")}
    },
  },
`;
}
