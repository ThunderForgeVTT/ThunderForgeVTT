import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

export interface LoaderProps {
  label?: string;
  fullScreen?: boolean;
  className?: string;
}

export function Loader({
  label = "Loading...",
  fullScreen = false,
  className,
}: LoaderProps) {
  return (
    <div
      // A page's full-screen wait, which spec 101 keeps to a world's first
      // page in a tab; `world-navigation.spec.ts` watches for it.
      data-testid={fullScreen ? "page-loader" : undefined}
      className={cn(
        "flex items-center justify-center gap-2 text-sm text-muted-foreground",
        fullScreen && "min-h-screen w-full",
        className,
      )}
    >
      <Loader2 className="size-5 animate-spin" aria-hidden="true" />
      <span>{label}</span>
    </div>
  );
}
