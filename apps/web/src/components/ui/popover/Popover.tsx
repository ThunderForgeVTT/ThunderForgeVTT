import type { ReactNode } from "react";
import {
  Popover as ShadcnPopover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";

export interface PopoverProps {
  trigger: ReactNode;
  children: ReactNode;
  className?: string;
}

export function Popover({ trigger, children, className }: PopoverProps) {
  return (
    <ShadcnPopover>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent className={cn(className)} sideOffset={12}>
        {children}
      </PopoverContent>
    </ShadcnPopover>
  );
}
