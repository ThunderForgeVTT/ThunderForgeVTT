/**
 * Routes keyboard input to the engine canvas from the window, so keys keep
 * working after the user touches any UI control.
 *
 * Bevy's winit web backend listens for `keydown`/`keyup` on the canvas
 * element itself, so it only ever sees keys while that element holds DOM
 * focus. Clicking any button — a dock tab, a tool, anything — moves focus
 * off the canvas and keyboard movement silently stops until the user
 * clicks the map again. Nothing errors; the events simply go to whatever
 * the browser focused instead.
 *
 * That is not just a user-facing annoyance. `e2e/canvas-authoring.spec.ts`
 * documents the same thing from the other side: its `waitForEngineReady`
 * has to click a deliberately-neutral corner of the canvas purely to focus
 * it, because a keyboard-only interaction on a fresh page load is dropped
 * outright.
 *
 * The fix is to stop depending on canvas focus at all: listen at the
 * window, and re-dispatch a copy of each event at the canvas, which is
 * where winit's listeners already are. Focus then decides only whether the
 * user is *typing* (see `isTextEntry`), which is the distinction that
 * actually matters now that the dock has a chat box in it.
 */

/** Events this module synthesised, so it never re-forwards its own. */
const synthetic = new WeakSet<KeyboardEvent>();

/**
 * Keys whose browser default would fight the canvas: arrows, space and the
 * page keys all scroll. The original event's default is suppressed only for
 * these — never blanket-mirrored from the copy, because winit's own
 * `prevent_default` is on by default and would otherwise swallow browser
 * shortcuts (Ctrl+R, Ctrl+F) the app has no business taking.
 */
const SCROLL_KEYS = new Set([
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Space",
  "PageUp",
  "PageDown",
  "Home",
  "End",
]);

const TEXT_ENTRY_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);

/**
 * True when the user is typing rather than playing. Movement keys are
 * ordinary letters (WASD), so forwarding while a text field has focus would
 * walk a token across the map every time someone wrote a chat message.
 */
function isTextEntry(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.isContentEditable) {
    return true;
  }
  return TEXT_ENTRY_TAGS.has(target.tagName);
}

/**
 * Begins routing window key events to the engine canvas. Returns a cleanup
 * function; safe to call before the canvas exists, since the element is
 * resolved per event rather than captured up front (the engine mounts its
 * canvas into `<body>` asynchronously on wasm start).
 */
export function startCanvasKeyboardRouting(): () => void {
  const forward = (event: KeyboardEvent) => {
    // Already ours, or already where winit is listening: leaving these
    // alone is what keeps this from feeding itself. A copy dispatched at
    // the canvas still travels the capture phase from the window down,
    // so this listener does see it.
    if (synthetic.has(event) || isTextEntry(event.target)) {
      return;
    }

    const canvas = document.querySelector<HTMLCanvasElement>("canvas");
    if (!canvas || event.target === canvas || canvas.style.display === "none") {
      return;
    }

    const copy = new KeyboardEvent(event.type, {
      key: event.key,
      code: event.code,
      location: event.location,
      repeat: event.repeat,
      isComposing: event.isComposing,
      ctrlKey: event.ctrlKey,
      shiftKey: event.shiftKey,
      altKey: event.altKey,
      metaKey: event.metaKey,
      // Not bubbling keeps the copy from reaching any listener above the
      // canvas; the capture-phase guard above covers the way down.
      bubbles: false,
      cancelable: true,
    });
    synthetic.add(copy);
    canvas.dispatchEvent(copy);

    if (SCROLL_KEYS.has(event.code)) {
      event.preventDefault();
    }
  };

  // Capture, so a UI control that calls `stopPropagation` on its own
  // keyboard handling cannot also stop the canvas from hearing the key.
  window.addEventListener("keydown", forward, { capture: true });
  window.addEventListener("keyup", forward, { capture: true });

  return () => {
    window.removeEventListener("keydown", forward, { capture: true });
    window.removeEventListener("keyup", forward, { capture: true });
  };
}
