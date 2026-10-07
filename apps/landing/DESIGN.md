---
name: ThunderForge
description: A wet-erase battle mat where the story is drawn in marker and the tokens really move.
colors:
  mat: "#eceee9"
  mat-deep: "#e2e5df"
  paper: "#fbfbf8"
  grid-line: "rgb(120 142 168 / 0.26)"
  grid-major: "rgb(96 118 146 / 0.42)"
  ink-black: "#1f2a2e"
  ink-blue: "#2a62c9"
  ink-red: "#c4352a"
  ink-green: "#2f9a54"
  text-soft: "#4a5759"
  text-blue: "#2457b5"
  text-red: "#b72f25"
  text-green: "#1d6e3b"
typography:
  display:
    fontFamily: "Permanent Marker, Comic Sans MS, cursive"
    fontSize: "clamp(3rem, 2rem + 4.6vw, 6rem)"
    fontWeight: 400
    lineHeight: 1
    letterSpacing: "0.005em"
  headline:
    fontFamily: "Permanent Marker, Comic Sans MS, cursive"
    fontSize: "clamp(2.25rem, 1.6rem + 2.6vw, 3.75rem)"
    fontWeight: 400
    lineHeight: 1.05
  title:
    fontFamily: "Permanent Marker, Comic Sans MS, cursive"
    fontSize: "1.7rem"
    fontWeight: 400
    lineHeight: 1.15
  lede:
    fontFamily: "Atkinson Hyperlegible Next, system-ui, sans-serif"
    fontSize: "clamp(1.1rem, 1rem + 0.3vw, 1.25rem)"
    fontWeight: 400
    lineHeight: 1.6
  body:
    fontFamily: "Atkinson Hyperlegible Next, system-ui, sans-serif"
    fontSize: "1.125rem"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "Atkinson Hyperlegible Next, system-ui, sans-serif"
    fontSize: "1.05rem"
    fontWeight: 700
    lineHeight: 1.2
  mono:
    fontFamily: "Atkinson Hyperlegible Mono, ui-monospace, monospace"
    fontSize: "0.95rem"
    fontWeight: 400
    lineHeight: 1.7
  figure:
    fontFamily: "Atkinson Hyperlegible Mono, ui-monospace, monospace"
    fontSize: "clamp(2.75rem, 2rem + 3vw, 4.5rem)"
    fontWeight: 400
    lineHeight: 1
    letterSpacing: "-0.02em"
rounded:
  tag: "3px"
  field: "4px"
  hand-sm: "4px 6px 5px 3px / 5px 3px 6px 4px"
  hand-md: "5px 7px 6px 4px / 6px 4px 7px 5px"
  hand-frame: "6px 9px 5px 8px"
  hand-tray: "10px 14px 8px 12px / 12px 8px 14px 10px"
spacing:
  cell: "48px"
  cell-tablet: "40px"
  cell-phone: "32px"
  section-y: "calc(var(--cell) * 3)"
  column-gap: "calc(var(--cell) * 2)"
  control-gap: "0.85rem"
components:
  ink-btn-blue:
    backgroundColor: "{colors.text-blue}"
    textColor: "{colors.paper}"
    typography: "{typography.label}"
    rounded: "{rounded.hand-md}"
    padding: "0.6em 1.15em"
    height: "48px"
  ink-btn-red:
    backgroundColor: "{colors.text-red}"
    textColor: "{colors.paper}"
    typography: "{typography.label}"
    rounded: "{rounded.hand-md}"
    padding: "0.6em 1.15em"
    height: "48px"
  ink-btn-black:
    backgroundColor: "{colors.ink-black}"
    textColor: "{colors.paper}"
    typography: "{typography.label}"
    rounded: "{rounded.hand-md}"
    padding: "0.6em 1.15em"
    height: "48px"
  ink-btn-line:
    backgroundColor: "transparent"
    textColor: "{colors.ink-black}"
    typography: "{typography.label}"
    rounded: "{rounded.hand-md}"
    padding: "0.6em 1.15em"
    height: "48px"
  ink-btn-line-hover:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-black}"
  ink-btn-red-line:
    backgroundColor: "transparent"
    textColor: "{colors.text-red}"
    rounded: "{rounded.hand-md}"
    padding: "0.6em 1.15em"
    height: "48px"
  ink-btn-small:
    padding: "0.35em 0.9em"
    height: "40px"
  chip:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-black}"
    rounded: "{rounded.hand-sm}"
    padding: "0.45rem 0.8rem"
    height: "48px"
  input-formula:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-black}"
    typography: "{typography.mono}"
    rounded: "{rounded.field}"
    padding: "0 0.9rem"
    height: "48px"
  legend-card:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-black}"
    rounded: "{rounded.field}"
    padding: "1.25rem 1.4rem 1.5rem"
  token-name:
    backgroundColor: "{colors.ink-black}"
    textColor: "{colors.paper}"
    rounded: "{rounded.tag}"
    padding: "0 0.35rem"
  ruler-label:
    backgroundColor: "{colors.text-green}"
    textColor: "{colors.paper}"
    typography: "{typography.mono}"
    rounded: "{rounded.field}"
    padding: "0.1rem 0.45rem"
---

# Design System: ThunderForge

## Overview

**Creative North Star: "The Battle Mat"**

The page is the table. Every surface sits on a cool vinyl wet-erase mat printed with a pale blue-grey grid, one square per foot-scale cell and every fifth line darker. Everything that is not reading text is drawn on that mat in four translucent marker inks, the way a game master sketches a room before the players arrive: walls in black, the call to play in blue, the call to give in red, the things you can measure and the notes in the margin in green. Real objects (hero tokens, dice) sit on the mat and cast small physical shadows; drawn things never do.

Density is generous and gridded. Measures, gaps and section rhythm are counted in cells, so a heading, a room wall and a draggable token all land on the same printed lines. The voice of the type is split cleanly: a felt-tip marker for anything a hand would write on the mat, a hyperlegible sans for anything a person must read, and a hyperlegible mono only where the page reports a formula, a measurement or a command.

The world refuses the category default of dark map art, glowing calls to action and feature-card grids. Controls are inked squares and outlines with slightly uneven corners, never stock pills. Motion is one gesture: the room's walls draw on once at load, and tokens snap to squares as they are dragged.

**Key Characteristics:**
- Light, cool vinyl ground (#eceee9) with a printed 1px grid at `--cell` and a darker line every five cells.
- Four marker inks, translucent and multiplied onto the mat, with round caps and overshot, slightly bowed strokes.
- Permanent Marker for the drawn voice; Atkinson Hyperlegible Next for reading; Atkinson Hyperlegible Mono for numbers and commands.
- Layout counted in cells (48 / 40 / 32px by breakpoint).
- Hand-drawn asymmetric corner radii on every ink control and frame.
- Real objects are draggable and snap to the grid; marker drawings stay flat.

## Colors

A near-neutral vinyl ground carrying four saturated marker inks, each with a harder-pressed text strength for legibility.

### Primary
- **Wet-Erase Blue** (#2a62c9, `--ink-blue`): the ink of the main invitation. Strokes, light rings on the map, the focus ring, the hover underline in the nav, selection tint (28% alpha). As filled text and button ground it is pressed harder to **Deep Pen Blue** (#2457b5, `--text-blue`): the primary "Play the demo world" button, links, the signed name, the "fantasy." in the headline.

### Secondary
- **Wet-Erase Red** (#c4352a, `--ink-red`): the ink of giving and of refusal. Donation actions, strikes through what went wrong, doors on the map, blocked ruler readings, link hover underline. Text strength **Deep Pen Red** (#b72f25, `--text-red`) for filled Donate buttons, foe token names and blocked ruler labels.

### Tertiary
- **Wet-Erase Green** (#2f9a54, `--ink-green`): the ink of measurement and margin notes. Rulers, tick marks, the rings around field-tested systems, map grid overlay. At 3.06:1 on the mat it is a stroke color only; any green words use **Deep Pen Green** (#1d6e3b, `--text-green`): the drag note, ruler labels, "field test first" tags, secondary measures.

### Neutral
- **Vinyl Mat** (#eceee9, `--mat`): the page ground everywhere; also the theme-color.
- **Shaded Vinyl** (#e2e5df, `--mat-deep`): the scrollbar track and the backing behind the map frame.
- **Grid Print** (`rgb(120 142 168 / 0.26)`, `--grid-line`) and **Grid Print, Major** (`rgb(96 118 146 / 0.42)`, `--grid-major`): the printed one-cell and five-cell lines, anchored at the page origin.
- **Card Paper** (#fbfbf8, `--paper`): the surface of things laid on the mat (legend card, chips, inputs, line-button hover) and the text color on filled inks.
- **Black Marker** (#1f2a2e, `--ink-black` / `--text`): body text, headings, walls, borders, the terminal ground. 12.6:1 on the mat.
- **Faded Marker** (#4a5759, `--text-soft`): secondary text, sources, captions, struck-out lines. 6.4:1 on the mat.

### Named Rules
**The Pressed Harder Rule.** Every ink has two strengths. The `--ink-*` value draws strokes, fills and outlines; the `--text-*` value is the same pen pressed harder and is the only one used for words or as a filled button ground. Wet-Erase Green never carries text.

**The Four Pens Rule.** The palette is the four inks plus the vinyl and the paper. A new color needs a new pen on the table, not a tint for a section.

## Typography

**Display Font:** Permanent Marker (with Comic Sans MS, cursive)
**Body Font:** Atkinson Hyperlegible Next (with system-ui, sans-serif), weights 400 and 700
**Label/Mono Font:** Atkinson Hyperlegible Mono (with ui-monospace, monospace), weight 400

**Character:** A felt-tip hand against a sans built for legibility. The marker is the game master writing on the mat; the sans is the page speaking plainly. Both are self-hosted through Fontsource.

### Hierarchy
- **Display** (Permanent Marker 400, `clamp(3rem, 2rem + 4.6vw, 6rem)`, line-height 1): the one hero headline inside the drawn room; balanced wrap.
- **Headline** (Permanent Marker 400, `clamp(2.25rem, 1.6rem + 2.6vw, 3.75rem)`, 1.05): every section heading (`.marker-head`). A big variant (`clamp(2.75rem, 1.8rem + 4vw, 5.25rem)`) closes the page.
- **Title** (Permanent Marker 400, 1.6 to 1.7rem, 1.15): sub-heads within a section, such as the legend title and the three support ways.
- **Annotation** (Permanent Marker 400, 1.05 to 2rem, in a `--text-*` ink): margin notes written on the mat: the drag invitation, the signature, the field-test tag, the closing thanks, empty and error states.
- **Lede** (Atkinson Hyperlegible Next 400, `clamp(1.1rem, 1rem + 0.3vw, 1.25rem)`, 1.6, max 38em): the opening paragraph under each headline.
- **Body** (Atkinson Hyperlegible Next 400, 1.125rem, 1.6, max 68ch): all running text. List items in emphasis use 700 at 1.15rem.
- **Label** (Atkinson Hyperlegible Next 700, 1.05rem, 1.2): button and chip text, nav links (1rem), form labels.
- **Mono** (Atkinson Hyperlegible Mono 400, 0.85 to 1.1rem): dice formulas, roll history, measurement sources, commands, counts inside buttons.
- **Figure** (Atkinson Hyperlegible Mono 400, `clamp(2.75rem, 2rem + 3vw, 4.5rem)`, 1, -0.02em): measured numbers and the dice total.

### Named Rules
**The Hand and the Page Rule.** Permanent Marker is for what a hand writes on the mat: headings, notes, signatures. It never sets a paragraph, a button label or a form field.

**The Instrument Rule.** Mono appears only where the page reports something exact: a formula, a measurement, a count, a command. Never as decoration.

## Layout

The spatial unit is the grid cell, `--cell`: 48px at 900px and up, 40px from 560 to 899px, 32px below 560px. The body background prints the grid from the page origin (`background-position: 0 0`), and the hero's room and tokens are laid out from that same origin so walls and tokens sit exactly on the printed lines.

Sections are centered at `min(100% - 2 * var(--cell), 75rem)` (phones: `100% - 32px`) with three cells of vertical padding (2.5 on phones). Most sections are a two-column grid with a column gap of 1.5 to 2 cells and asymmetric fractions (7fr/5fr in the hero, 1.1fr/1fr, 0.8fr/1.2fr, 1.55fr/1fr for the map legend), collapsing to one column below 900px (1024px for the map). The nav is two cells tall with one cell of side padding. Reading measure is capped at 68ch for body and 38em for ledes.

The hero is a full-bleed mat at least one viewport (or 16 cells) tall; below 900px it stacks and drops the minimum height, and below 560px the hero actions become full-width rows.

The numbers section carries no decorative rulers: a bar on the mat would read as a measurement, so only real measurements get one.

## Elevation & Depth

The mat is flat. Depth comes from two sources only: ink multiplied onto the vinyl (`mix-blend-mode: multiply`, 0.9 opacity on every marker stroke), and physical objects that sit on top of it. Drawn things never cast shadows; objects do.

### Shadow Vocabulary
- **Token at rest** (`filter: drop-shadow(0 2px 1.5px rgb(31 42 46 / 0.35))`): a hero or monster token lying on the mat.
- **Token lifted** (`filter: drop-shadow(0 8px 6px rgb(31 42 46 / 0.35))` with `scale(1.08)`): the token while it is being dragged.

### Named Rules
**The Objects Cast, Ink Doesn't Rule.** Only real tabletop objects (tokens, dice) get a shadow, and it is soft and short. Marker drawings, frames, cards and controls stay flat on the vinyl.

## Shapes

Corners are hand-drawn: every inked control and frame uses an asymmetric elliptical radius so no two corners match (buttons `5px 7px 6px 4px / 6px 4px 7px 5px`, chips `4px 6px 5px 3px / 5px 3px 6px 4px`, the map and terminal frame `6px 9px 5px 8px`, the dice tray `10px 14px 8px 12px / 12px 8px 14px 10px`). Plain functional surfaces (inputs, the legend card) use a quiet 4px; small tags 3px.

Strokes are the other half of the form language. Generated marker lines (`src/ink/marker.ts`) overshoot both ends by a seeded amount and bow slightly in the middle, with round caps and joins; a seed makes every redraw identical. Stroke weights are `--marker-w` (7px, 5.5px on phones) for walls and frames, `--marker-thin` (3.5px) for doors, rulers, checks and rings, and 1.5 to 2px hairlines for door swings. Hand-drawn primitives are reused across the page: the tick (`Check`) and the loose ring (`Ring`). Struck text is an inline `.strike` span with a red marker band and `box-decoration-break: clone`, so every wrapped line is struck.

Dividers inside a section are dashed 1 to 2px lines in Black Marker at 25 to 30% alpha; structural edges (legend split, footer top, system list rows) are solid 2 to 2.5px black. Tokens and ruler endpoints are the only true circles.

## Components

### Buttons (ink buttons)
Inked squares on the mat: a solid swatch of one pen, or that pen's outline.
- **Shape:** hand-drawn corners (`5px 7px 6px 4px / 6px 4px 7px 5px`), 3px border in the same ink, minimum 48px tall.
- **Filled:** blue (Deep Pen Blue, the single primary "Play the demo world"), red (Deep Pen Red, donations), black (Star on GitHub). Paper text, Atkinson 700 at 1.05rem, padding `0.6em 1.15em`, 0.55em gap to an inline SVG mark.
- **Line:** transparent ground, text and border in the ink (`--line`, `--red-line`, `--blue-line`); on hover the ground becomes Card Paper.
- **Hover / Focus:** a 2px lift with a 2px ring in the button's own ink (a heavier stroke, never an offset shadow) over 160ms `cubic-bezier(0.22, 1, 0.36, 1)`; pressing returns it flat. Focus is the global 3px Wet-Erase Blue outline at 3px offset.
- **Small:** 40px minimum, `0.35em 0.9em`, 0.95rem, used once in the nav.
- **Count slot:** a star count sits after a 2px divider in the ink color, set in mono 400.

### Chips (dice presets)
- **Style:** Card Paper ground, 2.5px Black Marker border, hand-drawn small corners, two lines: a 700 label over a mono formula in Deep Pen Blue.
- **State:** hover lifts 2px like the buttons; disabled at 50% opacity with a progress cursor.

### Cards / Containers
- **Legend card:** Card Paper, 2.5px Black Marker border, 4px corners, padding `1.25rem 1.4rem 1.5rem`, title in marker. Layer toggles are full-width 48px rows with dashed dividers; a toggled-off layer fades its swatch to 30% and is struck through in red.
- **Dice tray:** a felt drawn in blue marker: `--marker-w` border at 88% Wet-Erase Blue over a 6% blue wash, hand-drawn tray corners.
- **Map frame:** the map sits in a `--marker-w` Black Marker frame on Shaded Vinyl, hand-drawn frame corners. The frame is fixed at 7:4 for every map, so a square or tall map sits centred on the vinyl and the page never jumps when the map changes. The map's thumbnail stands in until the full image arrives. Each legend row carries the current map's count in a 1.5px outlined mono tag, dashed when the map has none.
- **Atlas cards:** every example map in the demo, as a row of Card Paper cards dealt onto the mat at -0.8, 0.9 and -0.4 degrees, 2.5px Black Marker border. A card holds the map's thumbnail (4:3, cropped), its name in bold and its counts in mono. Hover straightens and lifts the card 2px with a 1.5px blue ring; the chosen card sits straight with a 3px blue ring and a blue name. Below 1024px the cards become a horizontally scrolling strip, placed between the map and the legend.
- No generic shadowed card grid exists; groups of options are split by dashed rules instead (the three support ways).

### Inputs / Fields
- **Style:** Card Paper, 2.5px Black Marker border, 4px corners, 48px tall, mono 1.1rem.
- **Focus:** the global blue outline pulled in to a 1px offset.

### Navigation
- Two cells tall on the mat, brand mark (40px) and wordmark in Atkinson 700 at 1.2rem on the left. Section links in Black Marker 700 at 1rem, no underline at rest; a 3px Wet-Erase Blue marker line grows under the link on hover (200ms). Below 900px the section links hide, leaving the GitHub star link and the small red Donate button.

### Marker Room (signature)
The hero's room is drawn as SVG over the mat: wall segments in Black Marker at `--marker-w`, a door in `--marker-thin` with a dotted swing arc. Walls draw on once at load (520ms each, staggered 150ms, after 180ms), the swing fades in to 55%; reduced motion shows them drawn.

### Grid Tokens (signature)
Portrait tokens sit on cells and drag with pointer or keyboard, snapping to squares and blocked by walls. While dragging, a dashed marker ruler runs from the start square: green when the move is legal, red into a wall, an occupied square or the edge, with a mono label in Deep Pen Green or Deep Pen Red. A name tag (Black Marker, or Deep Pen Red for foes) appears on hover, focus and drag.

### Measures
A figure in mono at figure size, its unit in Atkinson 700, with no decorative ruler. The source file and date follow in Faded Marker mono.

### Terminal
The self-host commands sit on a Black Marker slab with hand-drawn frame corners, a marker caption, and a light Copy button. Inside the slab the brand teal (#56e0c7) is used for focus, the scrollbar and selection, the only place the brand mark's color enters the page.

## Do's and Don'ts

### Do:
- **Do** count space in cells (`--cell`: 48 / 40 / 32px) and lay positioned art from the page origin so it lands on the printed grid.
- **Do** draw with the four inks only, at `--marker-w` (7px) or `--marker-thin` (3.5px), round caps, 0.9 opacity, multiplied onto the mat.
- **Do** use the `--text-*` strength of an ink for any words or filled button grounds; keep Wet-Erase Green (#2f9a54) for strokes.
- **Do** generate hand strokes through `stroke()` in `src/ink/marker.ts` with a fixed seed, and reuse `Check` and `Ring` for ticks and circles; strike text with `.strike`.
- **Do** give every ink control and frame a hand-drawn asymmetric radius and a 48px minimum target.
- **Do** keep one blue filled button per view as the primary; red is for giving, black and outlines for the rest.
- **Do** set every reported number, formula and command in Atkinson Hyperlegible Mono, and only those.
- **Do** honor `prefers-reduced-motion` by showing drawn-on strokes and settled dice in their final state.

### Don't:
- **Don't** use stock pill buttons or fully rounded controls; controls are inked squares and outlines.
- **Don't** set paragraphs, button labels or form text in Permanent Marker.
- **Don't** put a shadow on a drawing, card or frame; only tokens and other tabletop objects cast one.
- **Don't** introduce dark map-art backgrounds, glowing calls to action or a feature-card grid.
- **Don't** add a color outside the four inks, the vinyl and the paper (the brand teal stays inside the terminal and the brand mark).
