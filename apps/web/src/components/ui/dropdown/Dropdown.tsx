import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import type { ReactNode } from "react";
import { cn } from "@/utils/cn";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./Dropdown.module.scss";

export interface DropdownItem {
  label: string;
  icon?: FantasyIconName;
  shortcut?: string;
  href?: string;
  disabled?: boolean;
  tone?: "default" | "danger";
  onSelect?: () => void;
}

export interface DropdownProps {
  trigger: ReactNode;
  items: DropdownItem[];
  align?: "start" | "center" | "end";
  className?: string;
}

export function Dropdown({
  trigger,
  items,
  align = "end",
  className,
}: DropdownProps) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align={align}
          className={cn(styles.content, className)}
          sideOffset={10}
        >
          {items.map((item) => {
            const body = (
              <span
                className={cn(
                  styles.item,
                  item.tone === "danger" && styles.danger,
                )}
              >
                <span className={styles.itemLabel}>
                  {item.icon ? (
                    <FantasyIcon name={item.icon} size={16} />
                  ) : null}
                  {item.label}
                </span>
                {item.shortcut ? (
                  <span className={styles.shortcut}>{item.shortcut}</span>
                ) : null}
              </span>
            );

            if (item.href) {
              return (
                <DropdownMenu.Item
                  key={item.label}
                  asChild
                  disabled={item.disabled}
                >
                  <a className={styles.rootItem} href={item.href}>
                    {body}
                  </a>
                </DropdownMenu.Item>
              );
            }

            return (
              <DropdownMenu.Item
                key={item.label}
                className={styles.rootItem}
                disabled={item.disabled}
                onSelect={item.onSelect}
              >
                {body}
              </DropdownMenu.Item>
            );
          })}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
