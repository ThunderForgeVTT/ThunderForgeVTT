import type { ReactNode } from "react";
import {
  DropdownMenu as ShadcnDropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuShortcut,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";

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
    <ShadcnDropdownMenu>
      <DropdownMenuTrigger asChild>{trigger}</DropdownMenuTrigger>
      <DropdownMenuContent align={align} className={cn(className)}>
        {items.map((item) => {
          const body = (
            <>
              {item.icon ? <FantasyIcon name={item.icon} size={16} /> : null}
              <span>{item.label}</span>
              {item.shortcut ? (
                <DropdownMenuShortcut>{item.shortcut}</DropdownMenuShortcut>
              ) : null}
            </>
          );

          const toneClass =
            item.tone === "danger" ? "text-destructive" : undefined;

          if (item.href) {
            return (
              <DropdownMenuItem
                key={item.label}
                asChild
                disabled={item.disabled}
                className={toneClass}
              >
                <a href={item.href}>{body}</a>
              </DropdownMenuItem>
            );
          }

          return (
            <DropdownMenuItem
              key={item.label}
              disabled={item.disabled}
              onSelect={item.onSelect}
              className={toneClass}
            >
              {body}
            </DropdownMenuItem>
          );
        })}
      </DropdownMenuContent>
    </ShadcnDropdownMenu>
  );
}
