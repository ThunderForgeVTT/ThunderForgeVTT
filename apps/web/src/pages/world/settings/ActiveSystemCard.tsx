import { SystemLegalNotice } from "@/components/game-systems/legal/SystemLegalNotice";
import { Card } from "@/components/ui/card/Card";
import type { SystemManifest } from "@/types/systemManifest";

export interface ActiveSystemCardProps {
  /** The world's system, or `null` while none is assigned. */
  manifest: SystemManifest | null;
}

/**
 * The rules a world runs on and the licence that comes with them (spec 016
 * FR-005).
 *
 * Drawn in two places: the Game Master's settings page, and the Overview's
 * "About this table" for everyone else. A Player is shown a GIF on the
 * settings page rather than the settings, and the licence is an obligation
 * owed to every member, so the notice cannot live only there.
 */
export function ActiveSystemCard({ manifest }: ActiveSystemCardProps) {
  return (
    <Card className="grid gap-4 p-6" data-testid="active-system-card">
      <h3 className="text-lg font-semibold">Active system</h3>
      {manifest ? (
        <div className="grid gap-3">
          <p className="text-sm">
            Currently using <strong>{manifest.title}</strong>.
          </p>
          <SystemLegalNotice legal={manifest.legal} variant="settings" />
        </div>
      ) : (
        <p className="text-sm text-muted-foreground italic">
          No system assigned yet.
        </p>
      )}
    </Card>
  );
}
