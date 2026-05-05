import type { ButtonHTMLAttributes, ReactElement, ReactNode } from "react";
import { Children, cloneElement, isValidElement } from "react";
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
  const resolvedClassName = cn(
    styles.button,
    styles[variant],
    styles[size],
    fullWidth && styles.fullWidth,
    className,
  );

  const content = (
    <>
      {icon && iconPosition === "start" ? (
        <FantasyIcon name={icon} size={18} className={styles.icon} />
      ) : null}
      <span>{children}</span>
      {icon && iconPosition === "end" ? (
        <FantasyIcon name={icon} size={18} className={styles.icon} />
      ) : null}
    </>
  );

  if (asChild) {
    const child = Children.only(children);

    if (!isValidElement(child)) {
      throw new Error(
        "Button with asChild requires a single React element child",
      );
    }

    const childProps = child.props as {
      className?: string;
      children?: ReactNode;
    };

    return cloneElement(
      child as ReactElement,
      {
        ...props,
        className: cn(resolvedClassName, childProps.className),
      },
      content,
    );
  }

  return (
    <button type={type} className={resolvedClassName} {...props}>
      {content}
    </button>
  );
}
