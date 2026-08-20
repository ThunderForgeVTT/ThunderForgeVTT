import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";

interface WorldPlaceholderPanelProps {
  title: string;
  icon: FantasyIconName;
  copy: string;
  items: readonly string[];
  surface?: "default" | "strong" | "parchment" | "stone" | "leather";
}

export function WorldPlaceholderPanel({
  title,
  icon,
  copy,
  items,
  surface = "stone",
}: WorldPlaceholderPanelProps) {
  return (
    <Card surface={surface} className="grid gap-3 p-5">
      <div className="flex items-center gap-3">
        <span className="inline-grid size-9 shrink-0 place-items-center rounded-full border border-border bg-secondary">
          <FantasyIcon name={icon} size={16} />
        </span>
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Placeholder domain
          </p>
          <h3 className="text-lg font-semibold">{title}</h3>
        </div>
      </div>
      <p className="text-sm text-muted-foreground">{copy}</p>
      {items.length > 0 ? (
        <ul className="grid list-inside list-disc gap-1 text-sm text-muted-foreground">
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      ) : (
        <p className="text-sm text-muted-foreground italic">
          Awaiting a later phase. No persisted records yet.
        </p>
      )}
    </Card>
  );
}
