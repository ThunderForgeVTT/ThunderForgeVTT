/**
 * Writes `HERO_SPEC_SCHEMA` to the server's copy:
 *
 *   pnpm -F @thunderforge/heroes run schema
 *
 * Run it after changing the catalogue; `schema.test.ts` fails until you do.
 */
import { writeFileSync } from "node:fs";
import { SERVER_SCHEMA_PATH } from "./schemaPath.ts";
import { heroSpecSchemaText } from "./schema.ts";

writeFileSync(SERVER_SCHEMA_PATH, heroSpecSchemaText());
process.stdout.write(`wrote ${SERVER_SCHEMA_PATH}\n`);
