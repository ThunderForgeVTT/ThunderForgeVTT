import type { SVGProps } from "react";
import { cn } from "@/utils/cn";
import styles from "./FantasyIcon.module.scss";

export type FantasyIconName =
  | "actors"
  | "arrow-left"
  | "compass"
  | "crown"
  | "inventory"
  | "map"
  | "quill"
  | "rune"
  | "scene"
  | "settings"
  | "shield"
  | "skull"
  | "spark"
  | "spells"
  | "tokens"
  | "torch"
  | "wand"
  | "worlds";

type IconTone = "default" | "gold" | "violet" | "forest" | "ember";

export interface FantasyIconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  name: FantasyIconName;
  size?: number;
  tone?: IconTone;
}

function iconPath(name: FantasyIconName) {
  switch (name) {
    case "actors":
      return (
        <>
          <circle cx="8" cy="8" r="3" />
          <path d="M3.5 19c1.2-3 3-4.5 4.5-4.5S11.3 16 12.5 19" />
          <circle cx="17" cy="9.5" r="2.5" />
          <path d="M14.5 19c.8-2 2-3.2 3.3-3.2s2.5 1.2 3.2 3.2" />
        </>
      );
    case "arrow-left":
      return <path d="M11 5L4 12l7 7M4 12h16" />;
    case "compass":
      return (
        <>
          <circle cx="12" cy="12" r="8.5" />
          <path d="M9 15l2-6 4-2-2 6-4 2z" />
        </>
      );
    case "crown":
      return <path d="M4 18l1.5-10L12 13l6.5-5L20 18H4zm2-3h12" />;
    case "inventory":
      return <path d="M5 7h14v12H5zM9 7V5h6v2M8.5 12h7" />;
    case "map":
      return <path d="M4 6l5-2 6 2 5-2v14l-5 2-6-2-5 2V6zM9 4v14m6-12v14" />;
    case "quill":
      return <path d="M18 4c-4 1-7 4-9 8l-3 8 8-3c4-2 7-5 8-9-1.3-1.7-2.7-3-4-4zM8 16l-1.5 1.5" />;
    case "rune":
      return <path d="M7 4v16M7 4l10 6-10 4 10 6" />;
    case "scene":
      return <path d="M4 19V5h16v14H4zm3-3l3.5-4 2.5 3 2-2 2.5 3" />;
    case "settings":
      return <path d="M12 8.5A3.5 3.5 0 1 0 12 15.5 3.5 3.5 0 1 0 12 8.5m0-5 1.2 2.1 2.3.4-.8 2.2 1.5 1.8-1.9 1.3.2 2.3-2.5-.4-2 1.2-1-2.1-2.3-.5.8-2.2L5.9 8.2l2-.8-.1-2.4 2.3.4L12 3.5z" />;
    case "shield":
      return <path d="M12 4l7 3v4c0 4.7-2.7 8.2-7 9-4.3-.8-7-4.3-7-9V7l7-3z" />;
    case "skull":
      return <path d="M12 4a6.5 6.5 0 0 0-6.5 6.5c0 2 1 3.6 2.5 4.7V19h2v-2h4v2h2v-3.8c1.5-1.1 2.5-2.7 2.5-4.7A6.5 6.5 0 0 0 12 4zM9 11h.01M15 11h.01" />;
    case "spark":
      return <path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9L12 3z" />;
    case "spells":
      return <path d="M6 5h9l3 3v11H6V5zm9 0v3h3M9 11h6M9 15h4" />;
    case "tokens":
      return (
        <>
          <circle cx="9" cy="12" r="4" />
          <circle cx="15.5" cy="9" r="3.5" />
          <path d="M14 15.5A4.6 4.6 0 0 1 18.5 20" />
        </>
      );
    case "torch":
      return <path d="M14 3c.2 2.1-.6 3.8-2.4 5.2-1.5 1.2-2.5 2.6-2.6 4.8h5c.5-1.4 1.6-2.5 2.8-3.7 1.5-1.5 2.4-3.3 2.2-6.3H14zM10 13h4v7h-4z" />;
    case "wand":
      return <path d="M5 19L19 5M15 3l.8 2.2L18 6l-2.2.8L15 9l-.8-2.2L12 6l2.2-.8L15 3z" />;
    case "worlds":
      return <path d="M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18zm0 0c2.3 2.2 3.7 5.2 3.9 9-1.2 3.7-3 6.7-3.9 9-2.3-2.2-3.7-5.2-3.9-9 .2-3.8 1.6-6.8 3.9-9zm-8.4 9h16.8" />;
    default:
      return <path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9L12 3z" />;
  }
}

export function FantasyIcon({
  name,
  size = 18,
  tone = "default",
  className,
  ...props
}: FantasyIconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={cn(styles.icon, styles[tone], className)}
      aria-hidden="true"
      {...props}
    >
      {iconPath(name)}
    </svg>
  );
}
