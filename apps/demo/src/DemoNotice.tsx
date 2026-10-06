import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { NOT_IN_DEMO_EVENT } from "./backend/notInDemo";
import { currentViewer, forgetSavedWorld, setViewer } from "./backend/state";
import { MAP_CREDIT } from "./credit";

/**
 * FR-006, FR-018: on every page, what this is, how to undo it, and whose
 * maps these are. FR-009: and the one notice for anything the demo does not
 * do. And the view switcher: the same world as its Game Master or as the
 * player who owns the two heroes, rendered by the real client for that member.
 */
export function DemoNotice() {
  const told = useRef(new Set<string>());

  useEffect(() => {
    const onRefused = (event: Event) => {
      const what = (event as CustomEvent<string>).detail;
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

  const asPlayer = currentViewer() === "player";
  const switchView = () => {
    setViewer(asPlayer ? "gm" : "player");
    window.location.reload();
  };

  const startOver = () => {
    forgetSavedWorld();
    window.location.assign(import.meta.env.BASE_URL);
  };

  return (
    <aside
      aria-label="About this demo"
      data-testid="demo-notice"
      className="border-border bg-card/95 text-card-foreground fixed bottom-2 left-1/2 z-50 flex max-w-[calc(100vw-1rem)] -translate-x-1/2 flex-wrap items-center justify-center gap-x-3 gap-y-1 rounded-full border px-4 py-1.5 text-xs shadow-lg backdrop-blur"
    >
      <span>
        <strong>Demo.</strong> Nothing is saved anywhere but this browser.
      </span>
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
    </aside>
  );
}
