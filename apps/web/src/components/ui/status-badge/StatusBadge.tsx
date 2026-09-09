import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";

type StatusBadgeVariant = "danger" | "info" | "success" | "warning";

export interface StatusBadgeProps {
  children: ReactNode;
  variant?: StatusBadgeVariant;
  className?: string;
  /**
   * Forwarded to the badge element.
   *
   * Named explicitly rather than left to a `...props` spread, because the
   * props this component accepts are deliberately few. Without it, a
   * `data-testid` written on a `StatusBadge` is dropped **silently** — the
   * JSX type-checks, the badge renders, and only the test that goes looking
   * for it fails, several minutes later and in another file. That cost a
   * debugging pass on 2026-09-09, and `LoreRepositoryCard` had been carrying
   * a testid nothing could ever find since it was written.
   */
  "data-testid"?: string;
}

const VARIANT_MAP: Record<
  StatusBadgeVariant,
  "default" | "secondary" | "destructive" | "outline"
> = {
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
  "data-testid": testId,
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
      data-testid={testId}
    >
      <FantasyIcon name={iconName} size={14} />
      <span>{children}</span>
    </Badge>
  );
}
