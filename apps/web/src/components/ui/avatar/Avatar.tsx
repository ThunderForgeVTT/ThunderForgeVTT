import { useMemo } from "react";
import {
  Avatar as ShadcnAvatar,
  AvatarImage,
  AvatarFallback,
} from "@/components/ui/avatar";
import { cn } from "@/lib/utils";
import { useAvatar } from "@/hooks/useAvatar";

export interface AvatarProps {
  seed: string;
  name?: string;
  size?: "sm" | "md" | "lg";
  className?: string;
}

const SIZE_CLASSES: Record<NonNullable<AvatarProps["size"]>, string> = {
  sm: "size-6",
  md: "size-8",
  lg: "size-12",
};

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
    <ShadcnAvatar className={cn(SIZE_CLASSES[size], className)}>
      <AvatarImage alt={name ?? seed} src={avatarSvgUrl} />
      <AvatarFallback>{initials}</AvatarFallback>
    </ShadcnAvatar>
  );
}
