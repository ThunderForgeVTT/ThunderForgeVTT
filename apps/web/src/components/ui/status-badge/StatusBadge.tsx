import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";

type StatusBadgeVariant = "danger" | "info" | "success" | "warning";

export interface StatusBadgeProps {
  children: ReactNode;
  variant?: StatusBadgeVariant;
  className?: string;
}

const VARIANT_MAP: Record<StatusBadgeVariant, "default" | "secondary" | "destructive" | "outline"> = {
  success: "default",
  warning: "outline",
  danger: "destructive",
  info: "secondary",
};

const VARIANT_EXTRA_CLASS: Record<StatusBadgeVariant, string> = {
  success: "bg-emerald-600 text-white",
  warning: "border-amber-500 text-amber-600 dark:text-amber-400",
  danger: "",
  info: "",
};

export function StatusBadge({
  children,
  variant = "info",
  className,
}: StatusBadgeProps) {
  const iconName =
    variant === "success"
      ? "shield"
      : variant === "warning"
        ? "torch"
        : variant === "danger"
          ? "skull"
          : "spark";

  return (
    <Badge
      variant={VARIANT_MAP[variant]}
      className={cn("gap-1", VARIANT_EXTRA_CLASS[variant], className)}
    >
      <FantasyIcon name={iconName} size={14} />
      <span>{children}</span>
    </Badge>
  );
}
