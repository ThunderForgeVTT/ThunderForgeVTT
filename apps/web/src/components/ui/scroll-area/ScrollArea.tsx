import type { ReactNode } from "react";
import { ScrollArea as ShadcnScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

export interface ScrollAreaProps {
  children: ReactNode;
  className?: string;
}

export function ScrollArea({ children, className }: ScrollAreaProps) {
  return (
    <ShadcnScrollArea className={cn(className)}>{children}</ShadcnScrollArea>
  );
}
