import { cn } from "@/lib/utils";
import { useAvatar } from "@/hooks/useAvatar";

export interface TokenAvatarProps {
  seed: string;
  label?: string;
  className?: string;
}

export function TokenAvatar({ seed, label, className }: TokenAvatarProps) {
  const { tokenPngUrl } = useAvatar(seed);

  return (
    <div className={cn("flex flex-col items-center gap-1 text-center", className)}>
      <img
        src={tokenPngUrl}
        alt={label ?? seed}
        className="size-12 rounded-full border border-border object-cover shadow-sm"
      />
      {label ? (
        <span className="max-w-16 truncate text-xs text-muted-foreground">
          {label}
        </span>
      ) : null}
    </div>
  );
}
