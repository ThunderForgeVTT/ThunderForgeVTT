import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { formatBytes } from "@/services/worldCacheStorage";
import {
  setPeerTransferEnabled,
  subscribeToPeerTransfer,
  type PeerTransferState,
} from "@/services/peerTransfer";

/**
 * Peer transfer: what it is doing, and the switch that stops it
 * (spec 028 FR-049, contracts/peer-protocol.md § Privacy, T092).
 *
 * # Why the setting lives here and the indicator does not
 *
 * FR-049 asks for two different things and they do not belong in the same
 * place. The *setting* is a considered decision about privacy, made rarely,
 * and it belongs where the app already keeps per-user, per-device decisions
 * about what this machine does on its own — `/settings/storage`, alongside
 * `StoragePanel`, which is the existing precedent for exactly that kind of
 * choice. The *indicator* has to reach someone who is not on a settings
 * page at all, because the IP exposure happens while they are playing, so it
 * lives on the world canvas next to the connection status (`PeerIndicator`).
 *
 * A permanent banner would satisfy neither: it is not where a setting can be
 * changed, and a notice that is always on screen is one people stop reading
 * within a session. The indicator therefore appears only while peers are
 * actually connected, and this panel is where it points.
 *
 * # The counters are local and stay local
 *
 * FR-052/FR-054 forbid telemetry. Nothing here is sent anywhere; these are
 * numbers the running client already holds, printed for the person whose
 * machine holds them.
 */

/** Shown when the user asks to turn it off; see `confirmDisable` below. */
function DisableWarningDialog({
  open,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onCancel()}>
      <DialogContent data-testid="peer-disable-dialog" className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Turn off peer transfer?</DialogTitle>
          <DialogDescription asChild>
            <div className="grid gap-3 text-left">
              <p>
                Content will only ever come from the server. Everything you can
                see and do stays exactly the same — some things may simply take
                a little longer to load.
              </p>
              {/*
                FR-057, and the reason this is a dialog rather than a bare
                toggle. Server-isolated play rides the same peer connections,
                so turning peer transfer off takes it away too. Someone who
                found this out later — mid-session, with the server down —
                would have traded away a capability without ever being told.
              */}
              <p data-testid="peer-disable-forfeit">
                <span className="font-medium text-foreground">
                  You will also give up playing through a server outage.
                </span>{" "}
                When the server is unreachable but everyone at the table can
                still reach each other, ThunderForge normally lets you keep
                moving tokens, with the GM&apos;s client deciding what sticks.
                That runs over the same peer connections, so it stops working
                too. You will drop straight to offline instead: your changes are
                kept on this machine and sync when the server returns.
              </p>
              <p>You can turn peer transfer back on here at any time.</p>
            </div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="ghost"
            onClick={onCancel}
            data-testid="peer-disable-cancel"
          >
            Keep it on
          </Button>
          <Button
            variant="destructive"
            onClick={onConfirm}
            data-testid="peer-disable-confirm"
          >
            Turn it off
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function PeerPanel() {
  const [state, setState] = useState<PeerTransferState | null>(null);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => subscribeToPeerTransfer(setState), []);

  const requestChange = useCallback((next: boolean) => {
    // Turning it *on* is a return to the default and gives nothing up, so it
    // takes effect immediately. Only the losing direction is worth a dialog;
    // confirming both would teach people to click through the one that matters.
    if (next) setPeerTransferEnabled(true);
    else setConfirming(true);
  }, []);

  const confirmDisable = useCallback(() => {
    setPeerTransferEnabled(false);
    setConfirming(false);
  }, []);

  if (!state) return null;

  return (
    <section className="grid gap-4" data-testid="peer-panel">
      <div className="grid gap-1">
        <h2 className="text-lg font-semibold">Peer transfer</h2>
        <p className="text-sm text-muted-foreground">
          To load maps and art faster, ThunderForge can fetch them directly from
          other people at your table instead of from the server. A direct
          connection lets those people&apos;s devices see your network address,
          and yours see theirs — the same as any voice or video call. Nothing
          else is shared, and every file is checked against the server&apos;s
          own fingerprint before it is used, so a peer cannot send you something
          different from what you asked for.
        </p>
      </div>

      <div className="flex items-center justify-between gap-4 rounded-md border px-3 py-3">
        <div className="grid gap-0.5">
          <span className="text-sm font-medium">
            Fetch content from other players
          </span>
          <span
            className="text-xs text-muted-foreground"
            data-testid="peer-toggle-state"
          >
            {state.enabled
              ? "On — content may come from peers or the server, whichever is quicker"
              : "Off — content only ever comes from the server"}
          </span>
        </div>
        <Switch
          checked={state.enabled}
          onCheckedChange={requestChange}
          aria-label="Fetch content from other players"
          data-testid="peer-toggle"
        />
      </div>

      {state.enabled ? (
        <div className="grid gap-2" data-testid="peer-activity">
          <p className="text-sm" data-testid="peer-connected-count">
            <span className="font-medium">
              {state.connectedPeers === 0
                ? "No peers connected"
                : `${state.connectedPeers} ${
                    state.connectedPeers === 1 ? "peer" : "peers"
                  } connected`}
            </span>{" "}
            <span className="text-muted-foreground">
              {state.connectedPeers === 0
                ? "right now — everything is coming from the server"
                : "right now, in worlds you have open"}
            </span>
          </p>
          <p className="text-sm text-muted-foreground" data-testid="peer-bytes">
            {formatBytes(state.bytesFromPeers)} received from peers since this
            page was opened.
          </p>
          {/*
            Shown only when non-zero. A permanent "0 rejected" line reads as a
            security scoreboard and invites worry about a number that is
            supposed to sit still; when it moves, it is worth seeing. Either
            way nothing was lost — the client already re-fetched from the
            server (FR-046, FR-048).
          */}
          {state.verificationFailures > 0 && (
            <p
              className="text-sm text-muted-foreground"
              data-testid="peer-verification-failures"
            >
              {state.verificationFailures}{" "}
              {state.verificationFailures === 1 ? "response" : "responses"} from
              peers did not match what was asked for and{" "}
              {state.verificationFailures === 1 ? "was" : "were"} discarded.
              Those files were fetched from the server instead; nothing was
              lost.
            </p>
          )}
        </div>
      ) : (
        <p
          className="text-sm text-muted-foreground"
          data-testid="peer-disabled-note"
        >
          Peer transfer is off. No direct connections are made, so no one at
          your table sees your network address — and playing through a server
          outage is unavailable, because it needs those same connections.
        </p>
      )}

      <DisableWarningDialog
        open={confirming}
        onCancel={() => setConfirming(false)}
        onConfirm={confirmDisable}
      />
    </section>
  );
}

export default PeerPanel;
