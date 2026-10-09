# Contract: layouts for the actor view and the world page

Both pages use the same widths, with Tailwind v4's default breakpoints.
They replace `Container` in their own markup with:

```tsx
<div className="mx-auto w-full max-w-[1800px] px-4 sm:px-6 py-6 lg:py-10">
```

`Container` itself, `components/ui/**`, `styles/**` and `routes/**` are
not changed.

| Width | Columns | Notes |
| --- | --- | --- |
| < 768 px | 1 | full width less the 16 px gutter; no sideways scroll at 320 px |
| 768 to 1023 px | 1, with paired small cards side by side | |
| 1024 to 1535 px (`lg`) | 2 | |
| ≥ 1536 px (`2xl`) | 3 | content capped at 1800 px, centred |

## Actor view (`/world/<w>/actor/<a>/view`)

| Column | Under 1024 | 1024 to 1535 | 1536 and up |
| --- | --- | --- | --- |
| A | portrait, name, core stats | portrait, core stats, rolls | portrait, core stats |
| B | sheet sections, in order | sheet sections | sheet sections |
| C | rolls, imagery | (in A) | rolls, imagery, notes |

- The sheet's own section grid may use 2 columns inside B from `xl`.
- Long text (notes, features) is held to `max-w-prose` inside its card,
  so a line never runs the full 1800 px.
- The back control honours `?from=players` (FR-062).

## World page (`/world/<w>`)

| Area | Under 1024 | 1024 to 1535 | 1536 and up |
| --- | --- | --- | --- |
| Header | name, Play, role | the same, one row | the same |
| Scenes card | 1st | column 1 | column 1 |
| Players card (new, FR-033) | 2nd | column 2 | column 2 |
| Session, settings | 3rd | column 2, under Players | column 3 |

The Players card shows the member count, active link count, and a link to
the players page. Its link management lives there now (FR-001).

## How it is checked

`apps/web/e2e/layout-widths.spec.ts` (worlds slice for the world page,
actors slice for the actor view), at 375, 1280 and 2560 px:

- `document.documentElement.scrollWidth <= window.innerWidth`;
- the column count, read from the grid's computed
  `grid-template-columns`;
- at 2560 px, the content box is 1800 px wide, ±1 px.
