import { Slot } from "@radix-ui/react-slot";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./Button.module.scss";

type ButtonVariant = "primary" | "secondary" | "success" | "ghost" | "danger";
type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  fullWidth?: boolean;
  asChild?: boolean;
  icon?: FantasyIconName;
  iconPosition?: "start" | "end";
  children: ReactNode;
}

export function Button({
  variant = "primary",
  size = "md",
  fullWidth = false,
  asChild = false,
  icon,
  iconPosition = "start",
  className,
  type = "button",
  children,
  ...props
}: ButtonProps) {
  const Comp = asChild ? Slot : "button";

  return (
    <Comp
      type={asChild ? undefined : type}
      className={cn(
        styles.button,
        styles[variant],
        styles[size],
        fullWidth && styles.fullWidth,
        className,
      )}
      {...props}
    >
      {icon && iconPosition === "start" ? (
        <FantasyIcon name={icon} size={18} className={styles.icon} />
      ) : null}
      <span>{children}</span>
      {icon && iconPosition === "end" ? (
        <FantasyIcon name={icon} size={18} className={styles.icon} />
      ) : null}
    </Comp>
  );
}
