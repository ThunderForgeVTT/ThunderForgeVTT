# Contract: The phone board's UI

What tests and the demo rely on. Desktop test ids (`gm-tool-*`,
`world-dock-tab-*`, `world-dock-panel-*`) are unchanged and do not render in
the phone layout.

## Elements

| Test id | Role / name | Present when |
|---|---|---|
| `phone-board` | region "Board controls"; `data-role` = `gm` \| `player` | phone layout |
| `phone-row` | `nav` "Board" | always, in the phone layout |
| `phone-row-<id>` | `button`, accessible name = label | per R6; `aria-expanded` when its sheet is open |
| `phone-row-tools-badge` | text, count of waiting requests | GM, requests > 0 |
| `board-sheet` | `dialog` (non-modal) named after its content; `data-size` = `half` \| `full`; `data-content` = id | a sheet is open |
| `board-sheet-expand` / `board-sheet-close` | `button` | a sheet is open |
| `phone-group-board` / `phone-group-table` | `group` | Tools sheet (GM) |
| `phone-item-<id>` | `button` | inside Tools / More |
| `tool-chip` | `button` "<Tool> options"; `data-tool` | a non-Select tool is armed |
| `tool-chip-leave` | `button` "Back to Select" | with `tool-chip` |
| `turn-line` | `status`, "Round N · <name>'s turn" | combat running |
| `level-chip` | `button` "Level: <name>" | scene has > 1 level |
| `fullscreen-toggle` | `button` "Full screen" / "Leave full screen" | `document.fullscreenEnabled` |
| `phone-top-stack` | container of every top notice and chip | phone layout |

Row ids: player `me`, `dice`, `sheet`, `chat`, `more`; GM `tools`, `dice`,
`actors`, `chat`, `combat`.

## Measurable guarantees (asserted by the `phone` slice)

- Every visible `button` in `phone-board`, `phone-top-stack` and the demo
  notice: bounding box ≥ 44 × 44; `document.elementFromPoint` at its centre
  is the button or inside it (nothing on top).
- With nothing open: (viewport area − union of control boxes) / viewport
  area ≥ 0.85 (player), ≥ 0.80 (GM).
- `board-sheet[data-size=half]` height ≤ 0.6 × viewport height.
- No `phone-*` element renders at 1280 × 800 or 820 × 1180.

## Manifest (`/manifest.webmanifest`)

`name`, `short_name` "ThunderForge"; `start_url` "/"; `scope` "/";
`display` "standalone"; `theme_color` and `background_color` "#0a1018";
icons: 192 PNG, 512 PNG, 512 PNG `purpose: maskable`, SVG `sizes: any`.
Served as `application/manifest+json`. Linked by `<link rel="manifest">` in
the app only.
