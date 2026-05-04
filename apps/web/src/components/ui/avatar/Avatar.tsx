import * as RadixAvatar from "@radix-ui/react-avatar";
import { useMemo } from "react";
import { cn } from "@/utils/cn";
import { useAvatar } from "@/hooks/useAvatar";
import styles from "./Avatar.module.scss";

export interface AvatarProps {
  seed: string;
  name?: string;
  size?: "sm" | "md" | "lg";
  className?: string;
}

export function Avatar({ seed, name, size = "md", className }: AvatarProps) {
  const { avatarSvgUrl } = useAvatar(seed);
  const initials = useMemo(
    () =>
      (name ?? seed)
        .split(/\s+/)
        .map((part) => part[0])
        .join("")
        .slice(0, 2)
        .toUpperCase(),
    [name, seed],
  );

  return (
    <RadixAvatar.Root className={cn(styles.avatar, styles[size], className)}>
      <RadixAvatar.Image alt={name ?? seed} className={styles.image} src={avatarSvgUrl} />
      <RadixAvatar.Fallback className={styles.fallback} delayMs={250}>
        {initials}
      </RadixAvatar.Fallback>
    </RadixAvatar.Root>
  );
}
