import { useEffect, useId, useRef, useState } from "react";
import { getWorldAbilities } from "@/api/abilities";
import { getActorAbilities } from "@/api/actorAbilities";
import { getWorldActors } from "@/api/actors";
import {
  getWorldSystemConditions,
  type WorldSystemCondition,
} from "@/api/actorConditions";
import { changeHitPoints } from "@/api/combat";
import {
  createToken,
  deleteToken,
  getTokens,
  setTokenLink,
} from "@/api/tokens";
import { Button } from "@/components/ui/button/Button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { didApply, type TokenControlFacet } from "@/engine/world/facets";
import type { WorldStore } from "@/engine/world/store";
import type { WorldLight, WorldShape, WorldWall } from "@/engine/world/types";
import type { WorldActorRecord } from "@/types/actor";
import type { HitPointChange } from "@/types/combat";
import type { SceneUnits } from "@/types/light";
import type { TokenRecord } from "@/types/token";
import { AttackFlow } from "../PlayDock/AttackFlow/AttackFlow";
import { abilityRolls } from "../PlayDock/characterRolls";
import {
  actionLabel,
  actionTestId,
  attackerTokenOf,
  canvasMenuActions,
  doorMenuActions,
  lightMenuActions,
  shapeMenuActions,
  shapeName,
  wallName,
  type CanvasMenuAction,
  type SheetAttack,
} from "./canvasMenuActions";
import { ConditionsDialog } from "./ConditionsDialog";
import { doorInteractive, performDoorAction } from "./doorActions";
import type { CanvasMenuRequest } from "./useCanvasContextMenu";

export interface CanvasContextMenuProps {
  request: CanvasMenuRequest | null;
  onDone: () => void;
  worldId: string;
  sceneId: string | null;
  worldStore: WorldStore;
  control: TokenControlFacet;
  isGameMaster: boolean;
  userId: string | null;
  /** For a new light's size: a torch's four and eight squares. */
  units?: SceneUnits;
}

/** What the menu found out about the right-click, once it had asked. */
interface Resolved {
  request: CanvasMenuRequest;
  tokens: TokenRecord[];
  target: TokenRecord | null;
  attacker: TokenRecord | null;
  actions: CanvasMenuAction[];
  /** What the world's system declares, for a Game Master on a character. */
  conditions: WorldSystemCondition[];
  /** The wall or door it is about, when it is about one (spec 071). */
  wall: WorldWall | null;
  /** The light or the drawing it is about, when it is about one (spec 073). */
  light: WorldLight | null;
  shape: WorldShape | null;
}

/** A menu about something that is not a token. */
function about(
  request: CanvasMenuRequest,
  actions: CanvasMenuAction[],
  thing: Partial<Pick<Resolved, "wall" | "light" | "shape">>,
): Resolved {
  return {
    request,
    tokens: [],
    target: null,
    attacker: null,
    actions,
    conditions: [],
    wall: null,
    light: null,
    shape: null,
    ...thing,
  };
}

/** What the menu calls what it is about, when that is not a token. */
function thingName(resolved: Resolved): string | null {
  if (resolved.light) return "Light";
  if (resolved.wall) return wallName(resolved.wall);
  if (resolved.shape) return shapeName(resolved.shape);
  return null;
}

/** How long after the menu opens a right-click on it is still the second click. */
const SECOND_CLICK_MS = 500;

type Follow =
  | {
      kind: "attack";
      attack: SheetAttack;
      target: TokenRecord;
      attacker: TokenRecord;
      tokens: TokenRecord[];
    }
  | { kind: "hit-points"; change: HitPointChange; target: TokenRecord }
  | { kind: "remove"; target: TokenRecord }
  | {
      kind: "conditions";
      target: TokenRecord;
      declared: WorldSystemCondition[];
    }
  | { kind: "place-token"; at: { x: number; y: number } };

/** A torch: bright four squares out, dim to eight (20 ft / 40 ft in 5e). */
const LIGHT_BRIGHT_SQUARES = 4;
const LIGHT_DIM_SQUARES = 8;

function nameOf(token: TokenRecord | null, fallback?: string): string {
  return token?.name?.trim() || fallback || "this creature";
}

/** The attacks this character's sheet can make, once each. */
async function sheetAttacks(
  worldId: string,
  actorId: string,
): Promise<SheetAttack[]> {
  const [entries, catalog] = await Promise.all([
    getActorAbilities(actorId),
    getWorldAbilities(worldId),
  ]);
  const seen = new Set<string>();
  const attacks: SheetAttack[] = [];
  for (const roll of abilityRolls(entries, catalog)) {
    if (!roll.attackAbilityId || seen.has(roll.attackAbilityId)) continue;
    seen.add(roll.attackAbilityId);
    attacks.push({
      abilityId: roll.attackAbilityId,
      name: roll.label.replace(/ \(attack\)$/, ""),
    });
  }
  return attacks;
}

/**
 * The play field's right-click menu (spec 031 FR-029, owner decision
 * 2026-09-15).
 *
 * The engine reports a right-click and what is under it; this decides what
 * that viewer may do there (`canvasMenuActions`) and does it through the same
 * calls the rest of chrome makes — the attack flow, `changeHitPoints`,
 * `setTokenLink`, the name toggle, `deleteToken`, `createToken` and a light
 * intent. Nothing here changes the board directly: every result comes back
 * the way any other change does (Constitution Principle I).
 *
 * # Keyboard and focus
 *
 * A Radix menu, so arrow keys move through items, Enter chooses and Escape
 * closes. It opens with focus on its first item and, closed, gives focus back
 * to wherever it was opened from — the canvas, for a right-click. An action
 * that needs more (an amount, a target list, a confirmation) opens a dialog
 * with focus inside it, and that dialog hands focus back the same way.
 *
 * The menu is anchored to an invisible trigger placed at the pointer, which
 * is what names it: "Actions for Goblin", or "Board actions".
 */
export function CanvasContextMenu({
  request,
  onDone,
  worldId,
  sceneId,
  worldStore,
  control,
  isGameMaster,
  userId,
  units,
}: CanvasContextMenuProps) {
  const [resolved, setResolved] = useState<Resolved | null>(null);
  const [follow, setFollow] = useState<Follow | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const followPending = useRef(false);
  // Held apart from `request`, which the page clears as soon as the menu
  // closes — before a dialog an item opened has finished and needs it.
  const returnFocus = useRef<HTMLElement | null>(null);
  const openedAt = useRef(0);

  // Ask what this right-click is before showing anything: the menu must not
  // offer an action and then discover it had no business offering it.
  useEffect(() => {
    if (!request || !sceneId) return;
    returnFocus.current = request.returnFocus;
    let active = true;
    (async () => {
      const viewer = { isGameMaster, userId };
      const light = request.lightId
        ? (worldStore.getState().lights[request.lightId] ?? null)
        : null;
      if (light) {
        const actions = lightMenuActions({ viewer, light });
        if (actions.length > 0) return about(request, actions, { light });
      }
      const shape = request.shapeId
        ? (worldStore.getState().shapes[request.shapeId] ?? null)
        : null;
      if (shape) {
        const actions = shapeMenuActions({ viewer, shape });
        if (actions.length > 0) return about(request, actions, { shape });
      }
      const wall =
        !request.tokenId && request.wallId
          ? (worldStore.getState().walls[request.wallId] ?? null)
          : null;
      if (wall) {
        const isDoor = wall.doorState !== "none";
        if (request.doubled && isGameMaster && isDoor) {
          // A double right-click: the first opened the menu, and this one
          // locks the door as a wall without asking again.
          const problem = await performDoorAction(
            { kind: "door-gm-lock" },
            wall,
            { worldStore, sceneId, isGameMaster },
          );
          if (active && problem) setNotice(problem);
          return null;
        }
        const canOpen =
          !isGameMaster && isDoor && !wall.locked
            ? (await doorInteractive(sceneId, wall.id))?.canActivate === true
            : false;
        const actions = doorMenuActions({ viewer, wall, canOpen });
        if (actions.length > 0) return about(request, actions, { wall });
        // Nothing to offer on it: it is board, as far as this viewer goes.
      }
      const tokens = await getTokens(sceneId);
      const target = request.tokenId
        ? (tokens.find((token) => token.tokenId === request.tokenId) ?? null)
        : null;
      if (request.tokenId && !target) {
        // Right-clicked something with no row this viewer may read.
        return null;
      }
      const attacker = isGameMaster ? null : attackerTokenOf(tokens, userId);
      const attacks =
        !isGameMaster && attacker?.actorId && target
          ? await sheetAttacks(worldId, attacker.actorId)
          : [];
      // Spec 067: only a Game Master sets conditions, and only on a token
      // standing for a character. A system that declares none offers none.
      const conditions =
        isGameMaster && target?.actorId
          ? await getWorldSystemConditions(worldId).catch(() => [])
          : [];
      const nameHidden =
        target !== null &&
        (target.nameVisibleToPlayers === false ||
          worldStore.getState().tokens[target.tokenId]?.nameHidden === true);
      const actions = canvasMenuActions({
        viewer,
        target,
        nameHidden,
        attacker,
        attacks,
        systemHasConditions: conditions.length > 0,
      });
      return {
        request,
        tokens,
        target,
        attacker,
        actions,
        conditions,
        wall: null,
        light: null,
        shape: null,
      };
    })()
      .then((answer) => {
        if (!active) return;
        if (!answer || answer.actions.length === 0) {
          onDone();
          return;
        }
        openedAt.current = performance.now();
        setResolved(answer);
      })
      .catch(() => {
        if (active) onDone();
      });
    return () => {
      active = false;
    };
  }, [request, sceneId, worldId, isGameMaster, userId, worldStore, onDone]);

  // Opened by a gesture on the canvas rather than by its trigger, a Radix menu
  // focuses its container and leaves no item highlighted; a keyboard user
  // would have to press an arrow before anything was chosen. The first item
  // takes focus instead, as a menu opened from its own button would give it.
  useEffect(() => {
    if (!resolved) return;
    const frame = requestAnimationFrame(() => {
      document
        .querySelector<HTMLElement>(
          '[data-testid="canvas-menu"] [role="menuitem"]',
        )
        ?.focus();
    });
    return () => cancelAnimationFrame(frame);
  }, [resolved]);

  const giveFocusBack = () => {
    const back = returnFocus.current;
    if (back && back.isConnected) back.focus();
  };

  const close = () => {
    setResolved(null);
    onDone();
  };

  const choose = async (action: CanvasMenuAction) => {
    if (!resolved) return;
    const { target, attacker, tokens, request: at } = resolved;
    switch (action.kind) {
      case "attack":
        if (target && attacker) {
          followPending.current = true;
          setFollow({
            kind: "attack",
            attack: action.attack,
            target,
            attacker,
            tokens,
          });
        }
        return;
      case "damage":
      case "heal":
        if (target) {
          followPending.current = true;
          setFollow({
            kind: "hit-points",
            change: action.kind === "damage" ? "DAMAGE" : "HEALING",
            target,
          });
        }
        return;
      case "remove":
        if (target) {
          followPending.current = true;
          setFollow({ kind: "remove", target });
        }
        return;
      case "conditions":
        if (target) {
          followPending.current = true;
          setFollow({
            kind: "conditions",
            target,
            declared: resolved.conditions,
          });
        }
        return;
      case "place-token":
        followPending.current = true;
        setFollow({ kind: "place-token", at: at.world });
        return;
      case "link":
        if (target) {
          await setTokenLink(target.tokenId, action.linked).catch(
            (error: unknown) =>
              setNotice(
                error instanceof Error
                  ? error.message
                  : "Changing the link failed",
              ),
          );
        }
        return;
      case "name":
        if (target) {
          const result = await control.setNameHidden(
            target.tokenId,
            action.hidden,
          );
          if (!didApply(result)) {
            setNotice(
              result.status === "rejected"
                ? result.reason
                : "The name could not be changed.",
            );
          }
        }
        return;
      case "add-light": {
        const grid = units?.gridSize ?? 50;
        worldStore.dispatch(
          {
            type: "create_light",
            light: {
              x: at.world.x,
              y: at.world.y,
              radius: grid * LIGHT_DIM_SQUARES,
              brightRadius: grid * LIGHT_BRIGHT_SQUARES,
              intensity: 1,
              color: null,
              attachedTokenId: null,
              castsShadows: true,
            },
            worldId,
          },
          "ui",
        );
        return;
      }
      case "door-state":
      case "door-locked":
      case "door-lock":
      case "door-gm-lock":
      case "door-reveal":
      case "door-designate":
        if (resolved.wall && sceneId) {
          const problem = await performDoorAction(action, resolved.wall, {
            worldStore,
            sceneId,
            isGameMaster,
          });
          if (problem) setNotice(problem);
        }
        return;
      // A light and a drawing are the world store's (AGENTS.md §2): an intent
      // goes in, and what the server made of it comes back to every board.
      case "light-power":
        if (resolved.light) {
          worldStore.dispatch(
            {
              type: "update_light",
              lightId: resolved.light.id,
              changes: { intensity: action.on ? 1 : 0 },
            },
            "ui",
          );
        }
        return;
      case "light-shadows":
        if (resolved.light) {
          worldStore.dispatch(
            {
              type: "update_light",
              lightId: resolved.light.id,
              changes: { castsShadows: action.casts },
            },
            "ui",
          );
        }
        return;
      case "light-remove":
        if (resolved.light) {
          worldStore.dispatch(
            { type: "delete_light", lightId: resolved.light.id },
            "ui",
          );
        }
        return;
      case "shape-visibility":
        if (resolved.shape) {
          worldStore.dispatch(
            {
              type: "update_shape",
              shapeId: resolved.shape.id,
              changes: { visibleToPlayers: action.visible },
            },
            "ui",
          );
        }
        return;
      case "shape-remove":
        if (resolved.shape) {
          worldStore.dispatch(
            { type: "delete_shape", shapeId: resolved.shape.id },
            "ui",
          );
        }
        return;
    }
  };

  const endFollow = () => {
    setFollow(null);
    giveFocusBack();
  };

  const thing = resolved ? thingName(resolved) : null;
  const menuLabel = thing
    ? `${thing} actions`
    : resolved?.target
      ? `Actions for ${nameOf(resolved.target)}`
      : "Board actions";

  return (
    <>
      {resolved ? (
        <DropdownMenu
          open
          // Not modal: a dialog an item opens takes over focus next, and two
          // modal layers handing focus between them leave the page inert.
          modal={false}
          onOpenChange={(open) => {
            if (!open) close();
          }}
        >
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              tabIndex={-1}
              aria-label={menuLabel}
              data-testid="canvas-menu-anchor"
              className="pointer-events-none fixed h-px w-px opacity-0"
              style={{
                left: resolved.request.at.x,
                top: resolved.request.at.y,
              }}
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent
            data-testid="canvas-menu"
            className="w-auto min-w-48"
            onContextMenu={(event) => {
              // Never the browser's menu over ours.
              event.preventDefault();
              // The second half of a double right-click that landed on the
              // menu the first half opened, rather than on the board beside
              // it. `timeStamp` is on the clock `openedAt` was read from.
              if (
                isGameMaster &&
                resolved.wall &&
                resolved.wall.doorState !== "none" &&
                event.timeStamp - openedAt.current <= SECOND_CLICK_MS
              ) {
                void choose({ kind: "door-gm-lock" });
                close();
              }
            }}
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              if (followPending.current) {
                followPending.current = false;
                return;
              }
              giveFocusBack();
            }}
          >
            <DropdownMenuLabel>
              {thing ?? (resolved.target ? nameOf(resolved.target) : "Here")}
            </DropdownMenuLabel>
            {resolved.actions.map((action) => (
              <DropdownMenuItem
                key={actionTestId(action)}
                data-testid={actionTestId(action)}
                variant={
                  action.kind === "remove" ||
                  action.kind === "light-remove" ||
                  action.kind === "shape-remove"
                    ? "destructive"
                    : "default"
                }
                disabled={action.kind === "door-locked"}
                onSelect={() => {
                  void choose(action);
                }}
              >
                {actionLabel(action, nameOf(resolved.target))}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      ) : null}

      {follow?.kind === "attack" ? (
        <Dialog open onOpenChange={(open) => !open && endFollow()}>
          <DialogContent
            data-testid="canvas-menu-attack-dialog"
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              giveFocusBack();
            }}
          >
            <DialogTitle>Attack {nameOf(follow.target)}</DialogTitle>
            <DialogDescription>
              With {follow.attack.name}, from{" "}
              {nameOf(follow.attacker, "your token")}.
            </DialogDescription>
            <AttackFlow
              worldId={worldId}
              attackerTokenId={follow.attacker.tokenId}
              abilityId={follow.attack.abilityId}
              abilityName={follow.attack.name}
              tokens={follow.tokens}
              targetTokenId={follow.target.tokenId}
              onClose={endFollow}
            />
          </DialogContent>
        </Dialog>
      ) : null}

      {follow?.kind === "hit-points" ? (
        <HitPointsDialog
          change={follow.change}
          target={follow.target}
          onClose={endFollow}
          onBack={giveFocusBack}
        />
      ) : null}

      {follow?.kind === "conditions" && follow.target.actorId ? (
        <ConditionsDialog
          name={nameOf(follow.target)}
          actorId={follow.target.actorId}
          declared={follow.declared}
          held={(follow.target.conditions ?? []).map((held) => held.id)}
          onClose={endFollow}
          onBack={giveFocusBack}
        />
      ) : null}

      {follow?.kind === "remove" ? (
        <Dialog open onOpenChange={(open) => !open && endFollow()}>
          <DialogContent
            data-testid="canvas-menu-remove-dialog"
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              giveFocusBack();
            }}
          >
            <DialogTitle>Remove {nameOf(follow.target)}?</DialogTitle>
            <DialogDescription>
              The token leaves the board for everyone. Its actor stays.
            </DialogDescription>
            <div className="flex justify-end gap-2">
              <Button type="button" variant="secondary" onClick={endFollow}>
                Keep it
              </Button>
              <Button
                type="button"
                variant="danger"
                data-testid="canvas-menu-remove-confirm"
                onClick={() => {
                  void deleteToken(follow.target.tokenId)
                    .catch((error: unknown) =>
                      setNotice(
                        error instanceof Error
                          ? error.message
                          : "Removing the token failed",
                      ),
                    )
                    .finally(endFollow);
                }}
              >
                Remove
              </Button>
            </div>
          </DialogContent>
        </Dialog>
      ) : null}

      {follow?.kind === "place-token" && sceneId ? (
        <PlaceTokenDialog
          worldId={worldId}
          sceneId={sceneId}
          at={follow.at}
          onClose={endFollow}
          onBack={giveFocusBack}
        />
      ) : null}

      {notice ? (
        <div
          role="alert"
          data-testid="canvas-menu-notice"
          className="fixed bottom-24 left-1/2 z-[1100] -translate-x-1/2 rounded-lg border border-border bg-background/95 px-4 py-2 text-sm shadow-xl"
        >
          <span>{notice}</span>{" "}
          <button
            type="button"
            className="underline"
            onClick={() => setNotice(null)}
          >
            Dismiss
          </button>
        </div>
      ) : null}
    </>
  );
}

/** How many hit points, then Apply: the Game Master's damage or healing. */
function HitPointsDialog({
  change,
  target,
  onClose,
  onBack,
}: {
  change: HitPointChange;
  target: TokenRecord;
  onClose: () => void;
  onBack: () => void;
}) {
  const [amount, setAmount] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const inputId = useId();
  const parsed = Number.parseInt(amount, 10);
  const valid = Number.isFinite(parsed) && parsed >= 0;
  const verb = change === "DAMAGE" ? "Damage" : "Heal";

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        data-testid="canvas-menu-hit-points-dialog"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          onBack();
        }}
      >
        <DialogTitle>
          {verb} {nameOf(target)}
        </DialogTitle>
        <DialogDescription>
          {change === "DAMAGE"
            ? "Temporary hit points go first; nothing drops below zero."
            : "Nothing rises above the maximum."}
        </DialogDescription>
        <form
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            if (!valid || busy) return;
            setBusy(true);
            setError(null);
            changeHitPoints(target.tokenId, change, parsed)
              .then(onClose)
              .catch((err: unknown) => {
                setError(
                  err instanceof Error
                    ? err.message
                    : "Changing hit points failed",
                );
              })
              .finally(() => setBusy(false));
          }}
        >
          <div className="grid gap-1.5">
            <Label htmlFor={inputId}>Hit points</Label>
            <Input
              id={inputId}
              type="number"
              min={0}
              inputMode="numeric"
              autoFocus
              value={amount}
              onChange={(event) => setAmount(event.target.value)}
              data-testid="canvas-menu-hit-points-amount"
            />
          </div>
          {error ? (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={onClose}>
              Cancel
            </Button>
            <Button
              type="submit"
              aria-disabled={!valid || busy}
              data-testid="canvas-menu-hit-points-apply"
            >
              {verb}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Which actor, then Place: a token where the Game Master right-clicked. */
function PlaceTokenDialog({
  worldId,
  sceneId,
  at,
  onClose,
  onBack,
}: {
  worldId: string;
  sceneId: string;
  at: { x: number; y: number };
  onClose: () => void;
  onBack: () => void;
}) {
  const [actors, setActors] = useState<WorldActorRecord[] | null>(null);
  const [actorId, setActorId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const selectId = useId();

  useEffect(() => {
    let active = true;
    getWorldActors(worldId)
      .then((list) => {
        if (!active) return;
        setActors(list);
        setActorId((current) => current || list[0]?.id || "");
      })
      .catch(() => active && setActors([]));
    return () => {
      active = false;
    };
  }, [worldId]);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        data-testid="canvas-menu-place-token-dialog"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          onBack();
        }}
      >
        <DialogTitle>Place a token here</DialogTitle>
        <DialogDescription>
          A token for one of this world&apos;s actors, where you right-clicked.
        </DialogDescription>
        <form
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            if (!actorId) return;
            createToken({ sceneId, actorId, x: at.x, y: at.y })
              .then(onClose)
              .catch((err: unknown) =>
                setError(
                  err instanceof Error
                    ? err.message
                    : "Placing the token failed",
                ),
              );
          }}
        >
          <div className="grid gap-1.5">
            <Label htmlFor={selectId}>Actor</Label>
            <select
              id={selectId}
              autoFocus
              value={actorId}
              onChange={(event) => setActorId(event.target.value)}
              className="rounded border border-border bg-background px-2 py-1"
              data-testid="canvas-menu-place-token-actor"
            >
              {(actors ?? []).map((actor) => (
                <option key={actor.id} value={actor.id}>
                  {actor.label}
                </option>
              ))}
            </select>
            {actors && actors.length === 0 ? (
              <p className="text-xs text-muted-foreground">
                This world has no actors to place yet.
              </p>
            ) : null}
          </div>
          {error ? (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={onClose}>
              Cancel
            </Button>
            <Button
              type="submit"
              aria-disabled={!actorId}
              data-testid="canvas-menu-place-token-confirm"
            >
              Place
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
