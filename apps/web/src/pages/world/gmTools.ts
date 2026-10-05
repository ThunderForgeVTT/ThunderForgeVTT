/**
 * The Game Master's authoring tools, fetched when one is opened (spec 068
 * FR-006).
 *
 * The board is one page for everybody at the table, and only one seat can
 * open these. Imported the plain way they were part of what every player
 * downloaded to see the map. `GmToolRail` mounts only the open tool's panel,
 * so a component that fetches its own code on first render is all it takes:
 * a player never renders one, and a Game Master pays for Walls when they
 * press Walls.
 *
 * The names are the tools' own, so the board's markup reads as it did.
 */

import { createElement } from "react";
import { onDemand } from "@/components/ui/lazy-boundary/onDemand";
import { Loader } from "@/components/ui/loader/Loader";
import type { WallToolProps } from "@/components/canvas-tools/WallTool";
import type { LightingToolProps } from "@/components/canvas-tools/LightingTool";
import type { ShapeToolProps } from "@/components/canvas-tools/ShapeTool";
import type { TokenToolProps } from "@/components/canvas-tools/TokenTool";
import type { InteractionToolProps } from "@/components/canvas-tools/InteractionTool";
import type { AssetPasteToolProps } from "@/components/canvas-tools/AssetPasteTool";

const opening = createElement(Loader, { label: "Opening the tool" });

export const WallTool = onDemand<WallToolProps>(
  () =>
    import("@/components/canvas-tools/WallTool").then((module) => ({
      default: module.WallTool,
    })),
  "The wall tool",
  opening,
);

export const LightingTool = onDemand<LightingToolProps>(
  () =>
    import("@/components/canvas-tools/LightingTool").then((module) => ({
      default: module.LightingTool,
    })),
  "The lighting tool",
  opening,
);

export const ShapeTool = onDemand<ShapeToolProps>(
  () =>
    import("@/components/canvas-tools/ShapeTool").then((module) => ({
      default: module.ShapeTool,
    })),
  "The shape tool",
  opening,
);

export const TokenTool = onDemand<TokenToolProps>(
  () =>
    import("@/components/canvas-tools/TokenTool").then((module) => ({
      default: module.TokenTool,
    })),
  "The token tool",
  opening,
);

export const InteractionTool = onDemand<InteractionToolProps>(
  () =>
    import("@/components/canvas-tools/InteractionTool").then((module) => ({
      default: module.InteractionTool,
    })),
  "The interaction tool",
  opening,
);

/**
 * Not in the rail: it listens for a paste for as long as a Game Master has a
 * scene open, and draws nothing until one lands. It loads with their board
 * and not with a player's.
 */
export const AssetPasteTool = onDemand<AssetPasteToolProps>(
  () =>
    import("@/components/canvas-tools/AssetPasteTool").then((module) => ({
      default: module.AssetPasteTool,
    })),
  "Pasting an image onto the map",
);
