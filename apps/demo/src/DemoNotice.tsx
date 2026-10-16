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
      data-open={open ? "true" : "false"}
      // Spec 102 R8: on a phone the bottom belongs to the board's row, so the
      // pill is a chip at the top left, clear of the centred top stack. Open,
      // it drops a card below the chip: the chip stays where it was tapped,
      // so the same tap closes it.
      className={`${open ? "w-[min(20rem,calc(100vw-1rem))] rounded-2xl pb-3" : "rounded-full"} border-border bg-card/95 text-card-foreground fixed top-[max(8px,env(safe-area-inset-top))] left-2 z-[1075] flex flex-col border px-3 text-xs shadow-lg backdrop-blur sm:top-auto sm:bottom-2 sm:left-1/2 sm:z-50 sm:w-auto sm:max-w-[calc(100vw-1rem)] sm:-translate-x-1/2 sm:flex-row sm:flex-wrap sm:items-center sm:justify-center sm:gap-x-3 sm:gap-y-1 sm:rounded-full sm:px-4 sm:py-1.5`}
    >
      <button
        type="button"
        aria-expanded={open}
        aria-controls="demo-notice-details"
        onClick={() => setOpen((was) => !was)}
        data-testid="demo-notice-toggle"
        className="flex min-h-11 min-w-11 items-center justify-between gap-2 font-medium whitespace-nowrap sm:hidden"
      >
        <strong>Demo</strong>
        <span
          aria-hidden="true"
          className={`transition-transform motion-reduce:transition-none ${open ? "rotate-180" : ""}`}
        >
          ▾
        </span>
        <span className="sr-only">: nothing leaves this browser</span>
      </button>
      <div
        id="demo-notice-details"
        className={`${open ? "flex" : "hidden"} flex-col gap-3 sm:contents`}
      >
        <span className="text-muted-foreground sm:text-card-foreground">
          <strong className="hidden sm:inline">Demo. </strong>
          Nothing is saved anywhere but this browser.
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
        <span data-testid="demo-viewer" className="grid gap-1.5 sm:inline">
          <span className="text-muted-foreground sm:text-card-foreground">
            Viewing as {asPlayer ? "a player" : "the Game Master"}.
          </span>{" "}
          <button
            type="button"
            onClick={switchView}
            className="bg-primary text-primary-foreground min-h-11 rounded-xl px-4 text-sm font-semibold sm:min-h-0 sm:rounded-none sm:bg-transparent sm:p-0 sm:text-xs sm:font-medium sm:text-primary sm:underline sm:underline-offset-2"
          >
            {asPlayer ? "View as Game Master" : "View as player"}
          </button>
        </span>
        <div className="grid grid-cols-2 gap-2 sm:contents">
          <button
            type="button"
            onClick={startOver}
            className="border-border min-h-11 rounded-xl border px-3 font-medium sm:min-h-0 sm:rounded-none sm:border-0 sm:p-0 sm:text-primary sm:underline sm:underline-offset-2"
          >
            Start over
          </button>
          <a
            href="/#self-host"
            data-cta="run_your_own"
            data-testid="demo-run-your-own"
            onClick={runYourOwnClicked}
            className="border-border flex min-h-11 items-center justify-center rounded-xl border px-3 font-medium sm:inline sm:min-h-0 sm:rounded-none sm:border-0 sm:p-0 sm:text-primary sm:underline sm:underline-offset-2"
          >
            Run your own
          </a>
        </div>
        <span className="text-muted-foreground border-border border-t pt-3 sm:border-0 sm:pt-0">
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
