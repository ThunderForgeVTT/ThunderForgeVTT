import { postGraphQL } from "@/api/graphqlClient";
import type { MapCredit } from "@/types/scene";

/** One map a new world can open on (spec 088 FR-025). */
export interface BaseMap {
  id: string;
  name: string;
  width: number;
  height: number;
  gridSize: number;
  thumbnailUrl: string;
  credit: MapCredit;
}

export interface BaseMapChoices {
  maps: BaseMap[];
  /** What the form starts on, or `null` for **None**. */
  defaultId: string | null;
}

type BaseMapsQuery = {
  baseMaps: BaseMap[];
  defaultBaseMapId: string | null;
};

/**
 * The maps this deployment ships, and the one the create-world form starts
 * on. A deployment with none answers an empty list and a `null` default, and
 * the form then offers only **None**.
 */
export function listBaseMaps(): Promise<BaseMapChoices> {
  return postGraphQL<BaseMapsQuery>(
    `
      query BaseMaps {
        baseMaps {
          id
          name
          width
          height
          gridSize
          thumbnailUrl
          credit { author licence licenceUrl source catalog shareAlike }
        }
        defaultBaseMapId
      }
    `,
  ).then((data) => ({ maps: data.baseMaps, defaultId: data.defaultBaseMapId }));
}
