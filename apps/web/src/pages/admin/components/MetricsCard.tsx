import { cn } from "@/lib/utils";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";

interface MetricsCardProps {
  title: string;
  value: string;
  subtitle: string;
  icon: FantasyIconName;
  emphasis?: "gold" | "violet" | "forest";
}

const EMPHASIS_BORDER: Record<NonNullable<MetricsCardProps["emphasis"]>, string> = {
  gold: "border-border",
  violet: "border-primary/30",
  forest: "border-emerald-500/30",
};

export function MetricsCard({
  title,
  value,
  subtitle,
  icon,
  emphasis = "gold",
}: MetricsCardProps) {
  return (
    <article
      className={cn(
        "grid gap-2 rounded-lg border bg-card p-4",
        EMPHASIS_BORDER[emphasis],
      )}
    >
      <div className="inline-flex items-center gap-2">
        <span className="inline-grid size-8 place-items-center rounded-full border border-border bg-secondary text-foreground">
          <FantasyIcon name={icon} size={16} />
        </span>
        <p className="text-xs font-bold tracking-widest text-muted-foreground uppercase">
          {title}
        </p>
      </div>
      <strong className="text-3xl leading-none font-semibold">{value}</strong>
      <span className="text-sm text-muted-foreground">{subtitle}</span>
    </article>
  );
}
