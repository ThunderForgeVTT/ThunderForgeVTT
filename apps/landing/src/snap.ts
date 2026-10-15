// The page's blocks start and end on grid lines.
//
// The engine's grid and the printed grid both run from the page's corner,
// a cell apart. A block's height comes from its words, which know nothing
// of cells. Inside each section, every piece (a heading, a tray, a row of
// cards) is pushed down to the next line and made whole cells tall; each
// section is then topped up to the next line with a bottom margin, so the
// next one starts on a line too.
//
// Content boxes are watched, and every correction lies outside one: a
// margin, or padding under a piece's last line. A piece that grows or
// shrinks is seen, and a correction is never mistaken for a change. (A
// min-height would hold a piece at the size it had while loading.)

const BLOCKS = ".hero, main > *, .footer";

const inFlow = (el: HTMLElement) => {
  const cs = getComputedStyle(el);
  return cs.display !== "none" && cs.display !== "contents" && cs.position !== "absolute" && cs.position !== "fixed";
};

/** Starts each of `block`'s pieces on a line and ends it on one. */
function snapPieces(block: HTMLElement, cell: number, seen: ResizeObserver) {
  const pieces = [...block.children].filter(
    (el): el is HTMLElement => el instanceof HTMLElement && inFlow(el),
  );
  for (const el of pieces) {
    seen.observe(el);
    el.style.marginTop = "";
    el.style.paddingBottom = "";
  }
  const flow = /^(block|flow-root)/.test(getComputedStyle(block).display);
  const pads = pieces.map((el) => parseFloat(getComputedStyle(el).paddingBottom) || 0);
  for (const [i, el] of pieces.entries()) {
    // Read after the last piece moved: each push moves every piece below it.
    const r = el.getBoundingClientRect();
    if (!r.height) continue;
    const top = r.top + scrollY;
    const lift = Math.ceil(top / cell - 0.01) * cell - top;
    if (lift > 0.01) {
      // In block flow the whole gap above is set: it only grows, so it wins
      // any margin collapse. In a grid or a flex row margins never
      // collapse, and the piece's own margin grows.
      const above = el.previousElementSibling?.getBoundingClientRect().bottom;
      const gap =
        flow && above !== undefined && above <= r.top + 0.5
          ? r.top - above
          : parseFloat(getComputedStyle(el).marginTop) || 0;
      el.style.marginTop = `${gap + lift}px`;
    }
    const short = Math.ceil(r.height / cell - 0.01) * cell - r.height;
    if (short > 0.01) el.style.paddingBottom = `${pads[i] + short}px`;
  }
}

export function snapToGrid() {
  const root = document.documentElement;
  let queued = false;

  const pass = () => {
    queued = false;
    const cell = parseFloat(getComputedStyle(root).getPropertyValue("--cell"));
    if (!(cell > 0)) return;
    for (const el of document.querySelectorAll<HTMLElement>(BLOCKS)) {
      seen.observe(el);
      if (el.classList.contains("section")) snapPieces(el, cell, seen);
      const r = el.getBoundingClientRect();
      if (!r.height) continue;
      // Measured from the page's top, so whatever sits above is absorbed too.
      const bottom = r.bottom + scrollY;
      // Half a pixel of slack: subpixel layout should not cost a whole cell.
      const extra = Math.ceil(bottom / cell - 0.01) * cell - bottom;
      const next = `${Math.max(0, Math.round(extra * 100) / 100)}px`;
      if (el.style.marginBottom !== next) el.style.marginBottom = next;
    }
  };
  const queue = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(pass);
  };

  const seen = new ResizeObserver(queue);
  seen.observe(document.body);
  addEventListener("resize", queue);
  document.fonts?.ready.then(queue);
  queue();
}
