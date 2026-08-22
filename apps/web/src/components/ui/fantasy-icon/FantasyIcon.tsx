import type { SVGProps } from "react";
import {
  ArrowLeft,
  Check,
  Compass,
  Copy,
  Crown,
  Feather,
  Flame,
  Globe,
  Image,
  Link as LinkIcon,
  Map as MapIcon,
  Moon,
  Package,
  Settings as SettingsIcon,
  Shapes,
  Shield,
  Skull,
  Sparkles,
  Sun,
  Trash2,
  Users,
  Wand2,
  WandSparkles,
  Zap,
} from "lucide-react";
import { cn } from "@/lib/utils";

export type FantasyIconName =
  | "actors"
  | "arrow-left"
  | "check"
  | "compass"
  | "copy"
  | "crown"
  | "inventory"
  | "link"
  | "map"
  | "moon"
  | "quill"
  | "rune"
  | "scene"
  | "settings"
  | "shield"
  | "skull"
  | "spark"
  | "spells"
  | "sun"
  | "tokens"
  | "torch"
  | "trash"
  | "wand"
  | "worlds";

type IconTone = "default" | "gold" | "violet" | "forest" | "ember";

export interface FantasyIconProps
  extends Omit<SVGProps<SVGSVGElement>, "name"> {
  name: FantasyIconName;
  size?: number;
  tone?: IconTone;
}

const ICON_MAP: Record<FantasyIconName, React.ComponentType<SVGProps<SVGSVGElement>>> = {
  actors: Users,
  "arrow-left": ArrowLeft,
  check: Check,
  compass: Compass,
  copy: Copy,
  crown: Crown,
  inventory: Package,
  link: LinkIcon,
  map: MapIcon,
  moon: Moon,
  quill: Feather,
  rune: Sparkles,
  scene: Image,
  settings: SettingsIcon,
  shield: Shield,
  skull: Skull,
  spark: Zap,
  spells: WandSparkles,
  sun: Sun,
  tokens: Shapes,
  torch: Flame,
  trash: Trash2,
  wand: Wand2,
  worlds: Globe,
};

const TONE_CLASSES: Record<IconTone, string> = {
  default: "text-foreground",
  gold: "text-primary",
  violet: "text-primary",
  forest: "text-muted-foreground",
  ember: "text-destructive",
};

export function FantasyIcon({
  name,
  size = 18,
  tone = "default",
  className,
  ...props
}: FantasyIconProps) {
  const Icon = ICON_MAP[name] ?? Sparkles;

  return (
    <Icon
      width={size}
      height={size}
      className={cn(TONE_CLASSES[tone], className)}
      aria-hidden="true"
      {...props}
    />
  );
}
