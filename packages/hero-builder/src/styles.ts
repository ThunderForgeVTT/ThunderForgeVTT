/**
 * The builder's own styles, scoped under `.tfhb` and mounted with it, so a
 * host needs no stylesheet, bundler rule or Tailwind source to show it. A host
 * may retheme through the custom properties.
 *
 * Every target is at least 44px (FR-020), and at phone width the pictures stay
 * on screen above the controls rather than scrolling away.
 */
export const BUILDER_CSS = `
.tfhb {
  --tfhb-bg: #1d1a24;
  --tfhb-surface: #2a2533;
  --tfhb-line: #4a4257;
  --tfhb-ink: #f2e8d8;
  --tfhb-muted: #b8ad9c;
  --tfhb-accent: #f4c542;
  --tfhb-danger: #ff8a7a;
  box-sizing: border-box;
  color: var(--tfhb-ink);
  background: var(--tfhb-bg);
  font: 15px/1.4 system-ui, sans-serif;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 16px;
  min-width: 0;
  max-width: 100%;
}
.tfhb *, .tfhb *::before, .tfhb *::after { box-sizing: inherit; }
.tfhb-stage {
  position: sticky; top: 0; z-index: 1;
  background: var(--tfhb-bg);
  padding: 8px 0;
}
.tfhb-preview { display: flex; align-items: center; justify-content: center; gap: 12px; }
.tfhb-preview svg { display: block; width: 100%; height: auto; }
.tfhb-portrait { width: min(40vw, 160px); }
.tfhb-token { width: min(26vw, 104px); }
.tfhb-preview-sm .tfhb-portrait { width: 96px; }
.tfhb-preview-sm .tfhb-token { width: 64px; }
.tfhb-panel { display: grid; gap: 12px; min-width: 0; }
@media (min-width: 900px) {
  .tfhb { grid-template-columns: 340px minmax(0, 1fr); align-items: start; }
  .tfhb-stage { top: 16px; padding: 16px; border-radius: 12px; background: var(--tfhb-surface); }
  .tfhb-preview { flex-direction: column; }
  .tfhb-portrait { width: 280px; }
  .tfhb-token { width: 160px; }
}
.tfhb-group {
  margin: 0; padding: 8px 12px 12px; min-width: 0;
  border: 1px solid var(--tfhb-line); border-radius: 10px;
}
.tfhb-legend { display: flex; align-items: center; gap: 8px; padding: 0 4px; font-weight: 600; }
.tfhb-following { font-weight: 400; font-size: 13px; color: var(--tfhb-muted); }
.tfhb-choices { display: flex; flex-wrap: wrap; gap: 6px; min-width: 0; }
.tfhb-chip { position: relative; display: inline-flex; align-items: center; gap: 6px; }
.tfhb-chip > input, .tfhb-swatch > input {
  position: absolute; opacity: 0; width: 1px; height: 1px; margin: 0;
}
.tfhb-chip > span {
  display: inline-flex; align-items: center; min-height: 44px; min-width: 44px;
  padding: 0 14px; border: 1px solid var(--tfhb-line); border-radius: 999px;
  background: var(--tfhb-surface); cursor: pointer; user-select: none;
}
.tfhb-chip > input:checked + span { border-color: var(--tfhb-accent); box-shadow: inset 0 0 0 2px var(--tfhb-accent); }
.tfhb-chip > input:focus-visible + span,
.tfhb-swatch > input:focus-visible + span { outline: 3px solid var(--tfhb-accent); outline-offset: 2px; }
.tfhb-swatch { position: relative; display: inline-block; }
.tfhb-swatch > span {
  display: block; width: 44px; height: 44px; border-radius: 8px;
  border: 2px solid var(--tfhb-line); cursor: pointer;
}
.tfhb-swatch > input:checked + span { border-color: var(--tfhb-ink); box-shadow: 0 0 0 2px var(--tfhb-accent); }
.tfhb-hex-row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 8px; }
.tfhb-hex { display: inline-flex; align-items: center; gap: 8px; }
.tfhb-hex-dot { width: 20px; height: 20px; border-radius: 50%; border: 1px solid var(--tfhb-line); }
.tfhb input[type="text"], .tfhb select {
  min-height: 44px; padding: 0 10px; min-width: 0; max-width: 100%;
  color: var(--tfhb-ink); background: var(--tfhb-surface);
  border: 1px solid var(--tfhb-line); border-radius: 8px; font: inherit;
}
.tfhb-hex input[type="text"] { width: 9ch; font-family: ui-monospace, monospace; }
.tfhb input:focus-visible, .tfhb select:focus-visible, .tfhb button:focus-visible {
  outline: 3px solid var(--tfhb-accent); outline-offset: 2px;
}
.tfhb-button, .tfhb-lock {
  min-height: 44px; min-width: 44px; padding: 0 14px; font: inherit;
  color: var(--tfhb-ink); background: var(--tfhb-surface);
  border: 1px solid var(--tfhb-line); border-radius: 8px; cursor: pointer;
}
.tfhb-lock { padding: 0; margin-left: auto; }
.tfhb-lock[aria-pressed="true"] { border-color: var(--tfhb-accent); }
.tfhb-dice { font-weight: 600; border-color: var(--tfhb-accent); }
.tfhb-roll, .tfhb-seed, .tfhb-texts, .tfhb-flags, .tfhb-actions {
  display: flex; flex-wrap: wrap; align-items: end; gap: 8px; min-width: 0;
}
.tfhb-text { display: grid; gap: 4px; min-width: 0; flex: 1 1 10rem; }
.tfhb-text > span { font-size: 13px; color: var(--tfhb-muted); }
.tfhb-flag { display: inline-flex; align-items: center; gap: 6px; }
.tfhb-flag .tfhb-lock { margin-left: 0; }
.tfhb-problems {
  margin: 0; padding: 8px 12px 8px 28px; color: var(--tfhb-danger);
  border: 1px solid var(--tfhb-danger); border-radius: 8px;
}
.tfhb-actions { padding-top: 8px; border-top: 1px solid var(--tfhb-line); }
.tfhb-visually-hidden {
  position: absolute; width: 1px; height: 1px; overflow: hidden;
  clip: rect(0 0 0 0); white-space: nowrap;
}
`;
