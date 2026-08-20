import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Children, cloneElement, isValidElement, type ReactElement } from "react";
import { Button as ShadcnButton } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";

type ButtonVariant = "primary" | "secondary" | "success" | "ghost" | "danger";
type ButtonSize = "sm" | "md" | "lg";

const VARIANT_MAP: Record<
  ButtonVariant,
  "default" | "secondary" | "ghost" | "destructive"
> = {
  primary: "default",
  secondary: "secondary",
  success: "default",
  ghost: "ghost",
  danger: "destructive",
};

const VARIANT_EXTRA_CLASS: Partial<Record<ButtonVariant, string>> = {
  success: "bg-emerald-600 text-white hover:bg-emerald-600/90",
};

const SIZE_MAP: Record<ButtonSize, "sm" | "default" | "lg"> = {
  sm: "sm",
  md: "default",
  lg: "lg",
};

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
  const content = (
    <>
      {icon && iconPosition === "start" ? (
        <FantasyIcon name={icon} size={16} />
      ) : null}
      {children}
      {icon && iconPosition === "end" ? (
        <FantasyIcon name={icon} size={16} />
      ) : null}
    </>
  );

  const resolvedClassName = cn(
    fullWidth && "w-full",
    VARIANT_EXTRA_CLASS[variant],
    className,
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

    return (
      <ShadcnButton
        asChild
        variant={VARIANT_MAP[variant]}
        size={SIZE_MAP[size]}
        className={resolvedClassName}
      >
        {cloneElement<{ className?: string; children?: ReactNode }>(
          child as ReactElement<{ className?: string; children?: ReactNode }>,
          { className: childProps.className },
          content,
        )}
      </ShadcnButton>
    );
  }

  return (
    <ShadcnButton
      type={type}
      variant={VARIANT_MAP[variant]}
      size={SIZE_MAP[size]}
      className={resolvedClassName}
      {...props}
    >
      {content}
    </ShadcnButton>
  );
}
