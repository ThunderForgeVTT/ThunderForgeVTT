# Contract: telemetry for spec 086

The events below go into `packages/telemetry`'s allow-list. Attributes are
bounded enums or buckets. No code, id, name, email, address, host or
setting value is ever sent (constitution VII). `viewport.bucket` is
already a resource attribute (spec 086), so the layout stories need no
event of their own.

If spec 086 has not landed when a story is built, its telemetry task waits
and is marked so in tasks.md.

## Browser events

| Event | Attributes | Sent when |
| --- | --- | --- |
| `world_link.created` | `max_uses`: `1`, `2-5`, `6-50`; `expiry`: `1d`, `7d`, `30d`, `none` | the GM creates a link |
| `world_link.revoked` | none | the GM confirms a revoke |
| `world_link.join` | `outcome`: `joined`, `already_member`, `revoked`, `expired`, `used_up`, `unknown`, `signed_out` | the join page settles |
| `world.created` | `starting_map`: `default`, `other`, `none`; `map_error`: `true`/`false` | `createWorld` returns |
| `rolls.cleared` | none | the GM confirms a clear |
| `map.imported` | `wall_edges`: `true`/`false`; `source`: `uvtt`, `image` | an import or background change succeeds |
| `settings.saved` | `section`: `mail`; `outcome`: `all`, `partial`, `none`; `changed`: `1`, `2-3`, `4+` | a save settles |
| `settings.unsaved_warning` | `action`: `stayed`, `left` | the guard asks and the admin answers |
| `hero.opened_from_players` | `target`: `sheet`, `builder` | a player opens either from their card |

## Server counters

Through `crates/thunderforge-server`'s telemetry API (spec 086):

| Counter | Attributes |
| --- | --- |
| `thunderforge.world_links.joins` | `outcome` (as above) |
| `thunderforge.world_links.oauth_refused` | none: an OAuth sign-in from a join with no account |
| `thunderforge.base_maps.applied` | `outcome`: `ok`, `upload_failed`, `import_failed` |
| `thunderforge.rolls.cleared` | none |
| `thunderforge.map_import.perimeter_walls` | histogram of walls added per import: 0 to 8+ |

The GraphQL root-field spans of 086 FR-011 already time `createWorld`,
`joinWorld`, `clearWorldRolls` and `updateSceneLevel`, so no new span is
added.
