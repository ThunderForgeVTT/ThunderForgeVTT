/** Where the server keeps its copy of `HERO_SPEC_SCHEMA`. */
import { fileURLToPath } from "node:url";

export const SERVER_SCHEMA_PATH = fileURLToPath(
  new URL(
    "../../../src/server/src/heroes/hero_spec_schema.json",
    import.meta.url,
  ),
);
