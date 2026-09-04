import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card/Card";
import type { SystemManifestLegal } from "@/types/systemManifest";

export interface SystemLegalNoticeProps {
  legal: SystemManifestLegal;
  /** Only affects surrounding chrome/framing — both variants always
   * render the full notice content. FR-006 is satisfied by rendering at
   * every call site unconditionally, not by conditionally hiding either
   * one based on `legal.requiredUiPlacement` (contracts/manifest-legal-schema.md's
   * UI contract). */
  variant: "selection" | "settings";
}

/**
 * Spec 016 (T007, contracts/manifest-legal-schema.md): renders one system
 * pack's required legal/attribution metadata. Used both at the point a GM
 * assigns/changes a world's system (variant="selection") and from the
 * persistent per-world System Settings view (variant="settings") — see
 * WorldSystemSettingsPage.tsx for why both call sites are actually the
 * same page in this app today.
 */
export function SystemLegalNotice({ legal, variant }: SystemLegalNoticeProps) {
  return (
    <Card
      surface={variant === "selection" ? "stone" : "parchment"}
      className="grid gap-3 p-5"
      data-testid="system-legal-notice"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          License &amp; attribution
        </p>
        <Badge variant="secondary">{legal.licenseName}</Badge>
      </div>

      {legal.requiredNotice ? (
        <p
          className="rounded-md border border-primary/40 bg-primary/10 px-3 py-2 text-sm font-medium"
          data-testid="system-legal-required-notice"
        >
          {legal.requiredNotice}
        </p>
      ) : null}

      <p className="text-sm whitespace-pre-wrap">{legal.attributionText}</p>

      {legal.disclaimer ? (
        <p className="text-sm text-muted-foreground italic">
          {legal.disclaimer}
        </p>
      ) : null}

      {legal.trademarkRestrictions && legal.trademarkRestrictions.length > 0 ? (
        <details className="text-sm">
          <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
            Trademark restrictions ({legal.trademarkRestrictions.length})
          </summary>
          <ul className="mt-2 grid list-disc gap-1 pl-5 text-muted-foreground">
            {legal.trademarkRestrictions.map((restriction, index) => (
              <li key={index}>{restriction}</li>
            ))}
          </ul>
        </details>
      ) : null}

      {legal.sourceUrl ? (
        <a
          href={legal.sourceUrl}
          target="_blank"
          rel="noreferrer noopener"
          className="text-xs text-muted-foreground underline underline-offset-2 hover:text-foreground"
        >
          View the license's canonical text
        </a>
      ) : null}
    </Card>
  );
}
