import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { toast } from "sonner";
import { NOT_IN_DEMO_EVENT } from "./backend/notInDemo";
import { currentViewer, forgetSavedWorld, setViewer } from "./backend/state";
import { MAP_CREDIT } from "./credit";
import { refusalArea } from "./backend/refusalNames";
import { runYourOwnClicked, startedOver, viewSwitched } from "./telemetry";
import { noticeStore } from "./telemetryNotice";

/**
 * FR-006, FR-018: on every page, what this is, how to undo it, and whose
 * maps these are. FR-009: and the one notice for anything the demo does not
 * do. And the view switcher: the same world as its Game Master or as the
 * player who owns the two heroes, rendered by the real client for that member.
 *
 * Spec 100 US6: on a phone the notice is a one-line pill over the board, and
 * a tap opens the rest. From `sm` up it reads in full, as it always has.
 */
export function DemoNotice() {
  const told = useRef(new Set<string>());
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onRefused = (event: Event) => {
      // Named by area, not by the field that asked: "Invite links", never
      // "generateInviteCode".
      const what = refusalArea((event as CustomEvent<string>).detail);
      // Once each: a page that polls should not fill the screen.
      if (told.current.has(what)) return;
      told.current.add(what);
      toast.info("Not part of the demo", {
        description: `${what} needs a real ThunderForge instance.`,
      });
    };
    window.addEventListener(NOT_IN_DEMO_EVENT, onRefused);
    return () => window.removeEventListener(NOT_IN_DEMO_EVENT, onRefused);
  }, []);

  const telemetryLine = useSyncExternalStore(
    noticeStore.subscribe,
    noticeStore.get,
  );
  const asPlayer = currentViewer() === "player";
  const switchView = () => {
    setViewer(asPlayer ? "gm" : "player");
    viewSwitched();
    window.location.reload();
  };

  const startOver = () => {
    forgetSavedWorld();
    startedOver();
    window.location.assign(import.meta.env.BASE_URL);
  };

  return (
    <aside
      aria-label="About this demo"
      data-testid="demo-notice"
      className={`${open ? "rounded-2xl" : "rounded-full"} border-border bg-card/95 text-card-foreground fixed bottom-2 left-1/2 z-50 flex max-w-[calc(100vw-1rem)] -translate-x-1/2 flex-wrap items-center justify-center gap-x-3 gap-y-1 border px-4 py-1.5 text-xs shadow-lg backdrop-blur sm:rounded-full`}
    >
      <button
        type="button"
        aria-expanded={open}
        aria-controls="demo-notice-details"
        onClick={() => setOpen((was) => !was)}
        data-testid="demo-notice-toggle"
        className="min-h-6 font-medium sm:hidden"
      >
        <strong>Demo</strong> · nothing leaves this browser{" "}
        <span aria-hidden="true">{open ? "▾" : "▸"}</span>
      </button>
      <div
        id="demo-notice-details"
        className={`${open ? "flex" : "hidden"} flex-wrap items-center justify-center gap-x-3 gap-y-1 sm:contents`}
      >
        <span>
          <strong>Demo.</strong> Nothing is saved anywhere but this browser.
        </span>
        {telemetryLine && (
          <a
            href={telemetryLine.href}
            data-testid="demo-telemetry-line"
            className="underline underline-offset-2"
          >
            {telemetryLine.text}
          </a>
        )}
        <span data-testid="demo-viewer">
          Viewing as {asPlayer ? "a player" : "the Game Master"}.{" "}
          <button
            type="button"
            onClick={switchView}
            className="text-primary font-medium underline underline-offset-2"
          >
            {asPlayer ? "View as Game Master" : "View as player"}
          </button>
        </span>
        <button
          type="button"
          onClick={startOver}
          className="text-primary font-medium underline underline-offset-2"
        >
          Start over
        </button>
        <a
          href="/#self-host"
          data-cta="run_your_own"
          data-testid="demo-run-your-own"
          onClick={runYourOwnClicked}
          className="text-primary font-medium underline underline-offset-2"
        >
          Run your own
        </a>
        <span className="text-muted-foreground">
          Maps by{" "}
          <a
            href={MAP_CREDIT.source}
            target="_blank"
            rel="noreferrer"
            className="underline underline-offset-2"
          >
            {MAP_CREDIT.author}
          </a>
          ,{" "}
          <a
            href={MAP_CREDIT.licenceUrl}
            target="_blank"
            rel="noreferrer"
            className="underline underline-offset-2"
          >
            {MAP_CREDIT.licence}
          </a>
          {" · "}
          <a
            href={MAP_CREDIT.catalog}
            target="_blank"
            rel="noreferrer"
            className="underline underline-offset-2"
          >
            more maps
          </a>
        </span>
      </div>
    </aside>
  );
}
