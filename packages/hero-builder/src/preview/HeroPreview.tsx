import { useMemo } from "react";
import type { HeroSpec } from "@thunderforge/heroes";
import { renderHero } from "./renderHero.ts";

export interface HeroPreviewProps {
  spec: HeroSpec;
  idPrefix: string;
  size?: "sm" | "md";
}

/**
 * A portrait and a token side by side. The SVG strings come from
 * `renderHero`, whose every text value is escaped, so mounting them as markup
 * is safe; the spec must already have passed `validateHero`.
 */
export function HeroPreview({ spec, idPrefix, size = "md" }: HeroPreviewProps) {
  const drawn = useMemo(() => renderHero(spec, idPrefix), [spec, idPrefix]);
  return (
    <div className={`tfhb-preview tfhb-preview-${size}`}>
      <div
        className="tfhb-portrait"
        data-testid="hero-preview-portrait"
        dangerouslySetInnerHTML={{ __html: drawn.portrait }}
      />
      <div
        className="tfhb-token"
        data-testid="hero-preview-token"
        dangerouslySetInnerHTML={{ __html: drawn.token }}
      />
    </div>
  );
}
