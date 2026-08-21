import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

export interface RuneDividerProps {
  label?: string;
  className?: string;
}

export function RuneDivider({
  label = "Arcane sigils",
  className,
}: RuneDividerProps) {
  return (
    <div className={cn("flex items-center gap-3", className)}>
      <Separator className="flex-1" />
      <span className="shrink-0 text-xs font-medium tracking-widest text-muted-foreground uppercase">
        {label}
      </span>
      <Separator className="flex-1" />
    </div>
  );
}
