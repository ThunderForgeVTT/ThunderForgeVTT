import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";

export interface ComingSoonTabProps {
  label: string;
}

/**
 * Spec 011 (FR-008): a placeholder for Compendium tabs whose content type
 * has no data model yet (Items, Abilities). No table, no search, no
 * fabricated example rows — just an honest "not built yet" state.
 */
export function ComingSoonTab({ label }: ComingSoonTabProps) {
  return (
    <Card className="grid gap-2 p-6 opacity-60" data-testid="compendium-coming-soon">
      <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        <FantasyIcon name="quill" size={14} />
        {label} — coming soon
      </p>
      <p className="text-sm text-muted-foreground">
        A dedicated {label.toLowerCase()} section will live here in a future update.
      </p>
    </Card>
  );
}
