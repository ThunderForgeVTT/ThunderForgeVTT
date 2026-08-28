import type { CSSProperties, HTMLAttributes, ReactNode } from "react";
import { Card as ShadcnCard } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export interface CardProps extends HTMLAttributes<HTMLElement> {
  children: ReactNode;
  surface?: "default" | "strong" | "parchment" | "stone" | "leather";
  style?: CSSProperties;
}

const SURFACE_CLASSES: Record<NonNullable<CardProps["surface"]>, string> = {
  default: "",
  strong: "bg-secondary",
  parchment: "bg-card",
  stone: "bg-muted",
  leather: "bg-accent",
};

export function Card({
  children,
  className,
  surface = "default",
  ...props
}: CardProps) {
  return (
    <ShadcnCard className={cn(SURFACE_CLASSES[surface], className)} {...props}>
      {children}
    </ShadcnCard>
  );
}
