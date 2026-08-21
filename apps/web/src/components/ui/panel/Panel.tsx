import type { HTMLAttributes, ReactNode } from "react";
import { Card } from "@/components/ui/card/Card";
import { cn } from "@/lib/utils";

type PanelVariant = "parchment" | "stone" | "leather" | "veil";

export interface PanelProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  variant?: PanelVariant;
}

const VARIANT_TO_SURFACE = {
  parchment: "parchment",
  stone: "stone",
  leather: "leather",
  veil: "default",
} as const;

export function Panel({
  children,
  variant = "parchment",
  className,
  ...props
}: PanelProps) {
  return (
    <Card
      surface={VARIANT_TO_SURFACE[variant]}
      className={cn("p-6", className)}
      {...props}
    >
      {children}
    </Card>
  );
}
