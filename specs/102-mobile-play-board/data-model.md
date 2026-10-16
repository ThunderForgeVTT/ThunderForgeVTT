# Data Model: A Board That Fits a Phone

No server or stored data changes. These are the client-side UI states the
phone layout reads and holds.

## Arrangement

| Field | Type | Source |
|---|---|---|
| `phone` | boolean | `usePhoneLayout()`: viewport shorter side < 600 |

Changes on rotate or resize. Changing it re-renders the chrome only; every
state below survives (FR-003).

## Viewer role on the board

| Field | Type | Source |
|---|---|---|
| `isGm` | boolean | `useWorldRole` (owner counts as GM) |
| `permittedTools` | `GmToolId[]` | `railTools(...)` (server grants) |
| `sections` | `DockSection[]` | `useDockSections(...)` (already role-filtered) |

Trusted Player = player. A role change re-renders the rows (edge case).

## Phone rows (derived, pure)

`phoneRows(isGm, permittedTools, sections)` →

| Field | Type | Rule |
|---|---|---|
| `row` | 5 × `RowItem` | Player: me, dice, sheet, chat, more. GM: tools, dice, actors, chat, combat |
| `groups` | `{ board: Item[], table: Item[] }` | GM: Tools sheet; player: More sheet (one group) |

`RowItem = { id, label, icon, opens: SheetContent }`. An item whose section or
tool is absent from the inputs is never produced (FR-006, Principle III).

## Armed tool and its options

| Field | Owner | Notes |
|---|---|---|
| `armedTool` | page (`effectiveGmToolId`, existing) | `select` on load |
| `toolOptionsOpen` | PhoneBoard (new, local) | `false` on load (FR-016) |

```
select (no chip) --pick X in Tools--> X armed, options open
X armed, options open --close sheet--> X armed, chip shown
X armed, chip shown --tap chip--> options open
X armed, chip shown --tap ✕--> select (no chip)
X revoked by server --> select (existing revoked notice)
```

## Sheet

| Field | Type | Notes |
|---|---|---|
| `content` | `SheetContent \| null` | a dock section, tool options, dice, sheet, more, tools, character chooser |
| `size` | `"half" \| "full"` | half on open |

One at a time (FR-013): opening another replaces it; an arriving request only
badges the Tools tile. Back: `null` (FR-014). Rotating keeps `content` and
`size`.

## Display mode

| Field | Values | Source |
|---|---|---|
| `display` | `browser` \| `standalone` \| `fullscreen` | `matchMedia("(display-mode: …)")` + `document.fullscreenElement` |

Hides the full-screen control when already full screen; sent as a telemetry
label with `layout`.
