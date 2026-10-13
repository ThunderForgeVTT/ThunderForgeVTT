import { Button } from "@/components/ui/button/Button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { PlayPausesState } from "./usePlayPauses";

/** The page's three confirmations: pause a world, decide a request, lift a pause. */
export function PlayPauseDialogs({ state }: { state: PlayPausesState }) {
  const {
    target,
    pausing,
    setTarget,
    outcomeRef,
    openerRef,
    confirmPause,
    grounds,
    setGrounds,
    dialogError,
    deciding,
    submittingDecision,
    setDeciding,
    confirmDecision,
    note,
    setNote,
    decisionError,
    lifting,
    submittingLift,
    setLifting,
    confirmLift,
    liftGrounds,
    setLiftGrounds,
    liftError,
  } = state;
  return (
    <>
      <Dialog
        open={target !== null}
        onOpenChange={(open) => {
          if (!open && !pausing) setTarget(null);
        }}
      >
        <DialogContent
          data-testid="play-pause-confirm"
          onCloseAutoFocus={(event) => {
            // After a pause, to the sentence saying what happened: the list
            // the dialog was opened from is re-fetched and re-rendered, so the
            // button that opened it may no longer exist. After a cancel,
            // nothing changed, so back to that button.
            event.preventDefault();
            const back = outcomeRef.current ?? openerRef.current;
            if (back?.isConnected) back.focus();
          }}
        >
          <form className="grid gap-4" onSubmit={(e) => void confirmPause(e)}>
            <DialogHeader>
              <DialogTitle>Pause play in {target?.name}?</DialogTitle>
              <DialogDescription>
                Everyone playing in this world leaves the playfield within
                seconds and is told that an operator has paused play. Nobody can
                play in it again until an operator lifts the pause.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="play-pause-grounds">Grounds</Label>
              <Textarea
                id="play-pause-grounds"
                required
                aria-required="true"
                aria-describedby="play-pause-grounds-hint"
                value={grounds}
                maxLength={5000}
                onChange={(event) => setGrounds(event.target.value)}
              />
              <p
                id="play-pause-grounds-hint"
                className="text-xs text-muted-foreground"
              >
                Required. Recorded with your name, and never shown to anyone in
                the world.
              </p>
              {dialogError ? (
                <p role="alert" className="text-sm text-destructive">
                  {dialogError}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button
                variant="ghost"
                disabled={pausing}
                onClick={() => setTarget(null)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                variant="danger"
                disabled={pausing || grounds.trim() === ""}
                data-testid="play-pause-confirm-submit"
              >
                Pause play
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog
        open={deciding !== null}
        onOpenChange={(open) => {
          if (!open && !submittingDecision) setDeciding(null);
        }}
      >
        <DialogContent
          data-testid="play-pause-decide"
          onCloseAutoFocus={(event) => {
            // As the pause dialog: to what happened after a decision (the
            // request has left the list), back to the opener after a cancel.
            event.preventDefault();
            const back = outcomeRef.current ?? openerRef.current;
            if (back?.isConnected) back.focus();
          }}
        >
          <form
            className="grid gap-4"
            onSubmit={(e) => void confirmDecision(e)}
          >
            <DialogHeader>
              <DialogTitle>
                {deciding?.decision === "APPROVE"
                  ? `Approve, and pause play in ${deciding.request.worldName}?`
                  : `Decline the request to pause ${deciding?.request.worldName ?? ""}?`}
              </DialogTitle>
              <DialogDescription>
                {deciding?.decision === "APPROVE"
                  ? "Everyone playing in this world leaves the playfield within seconds and is told that an operator has paused play. Nobody can play in it again until an operator lifts the pause."
                  : "Play goes on. Nobody in the world is told anything, and the decision is recorded."}
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="play-pause-decision-note">Note</Label>
              <Textarea
                id="play-pause-decision-note"
                required
                aria-required="true"
                aria-describedby="play-pause-decision-note-hint"
                value={note}
                maxLength={5000}
                onChange={(event) => setNote(event.target.value)}
              />
              <p
                id="play-pause-decision-note-hint"
                className="text-xs text-muted-foreground"
              >
                {deciding?.decision === "APPROVE"
                  ? "Required. Recorded with your name as the pause's grounds, and never shown to anyone in the world."
                  : "Required. Recorded with your name, and never shown to anyone in the world."}
              </p>
              {decisionError ? (
                <p role="alert" className="text-sm text-destructive">
                  {decisionError}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button
                variant="ghost"
                disabled={submittingDecision}
                onClick={() => setDeciding(null)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                variant="primary"
                disabled={submittingDecision || note.trim() === ""}
                data-testid="play-pause-decide-submit"
              >
                {deciding?.decision === "APPROVE"
                  ? "Approve and pause play"
                  : "Decline request"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog
        open={lifting !== null}
        onOpenChange={(open) => {
          if (!open && !submittingLift) setLifting(null);
        }}
      >
        <DialogContent
          data-testid="play-pause-lift"
          onCloseAutoFocus={(event) => {
            // As the other dialogs: to what happened after a lift (the pause
            // has left *Active pauses*), back to the opener after a cancel.
            event.preventDefault();
            const back = outcomeRef.current ?? openerRef.current;
            if (back?.isConnected) back.focus();
          }}
        >
          <form className="grid gap-4" onSubmit={(e) => void confirmLift(e)}>
            <DialogHeader>
              <DialogTitle>Lift the pause on {lifting?.worldName}?</DialogTitle>
              <DialogDescription>
                Its Game Master and players can play in this world again.
                Nothing else about the world changes, and anything taken down
                separately stays withheld.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="play-pause-lift-grounds">Grounds</Label>
              <Textarea
                id="play-pause-lift-grounds"
                required
                aria-required="true"
                aria-describedby="play-pause-lift-grounds-hint"
                value={liftGrounds}
                maxLength={5000}
                onChange={(event) => setLiftGrounds(event.target.value)}
              />
              <p
                id="play-pause-lift-grounds-hint"
                className="text-xs text-muted-foreground"
              >
                Required. Recorded with your name, and never shown to anyone in
                the world.
              </p>
              {liftError ? (
                <p role="alert" className="text-sm text-destructive">
                  {liftError}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button
                variant="ghost"
                disabled={submittingLift}
                onClick={() => setLifting(null)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                variant="primary"
                disabled={submittingLift || liftGrounds.trim() === ""}
                data-testid="play-pause-lift-submit"
              >
                Lift pause
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
