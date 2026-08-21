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
