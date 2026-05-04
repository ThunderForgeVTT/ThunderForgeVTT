import * as RadixPopover from "@radix-ui/react-popover";
import type { ReactNode } from "react";
import { cn } from "@/utils/cn";
import styles from "./Popover.module.scss";

export interface PopoverProps {
  trigger: ReactNode;
  children: ReactNode;
  className?: string;
}

export function Popover({ trigger, children, className }: PopoverProps) {
  return (
    <RadixPopover.Root>
      <RadixPopover.Trigger asChild>{trigger}</RadixPopover.Trigger>
      <RadixPopover.Portal>
        <RadixPopover.Content
          className={cn(styles.content, className)}
          sideOffset={12}
        >
          {children}
          <RadixPopover.Arrow className={styles.arrow} />
        </RadixPopover.Content>
      </RadixPopover.Portal>
    </RadixPopover.Root>
  );
}
