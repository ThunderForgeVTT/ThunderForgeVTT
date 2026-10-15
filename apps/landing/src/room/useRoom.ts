import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { diceCommand } from "@thunderforge/dice-throw";
import {
  chestCommand,
  createReferee,
  GOAL_EVENT,
  HERO_ID,
  rollbackCommand,
  roomTeardown,
  toWorld,
  type Direction,
  type EngineEvent,
  type GoalEventDetail,
  type GoalId,
  type Judged,
  type RoomScript,
  type RoomState,
} from "@thunderforge/encounter";
import type { FrameState, Send } from "../engine/EngineFrame.tsx";
import { listen } from "../engine/dispatch.ts";
import { canvasOf, running, viewAs } from "../engine/host.ts";
import { throwId } from "../dice.ts";
import { pressOnCanvas } from "../engine/keys.ts";
import { PARTY, type PartyHero } from "./party.ts";
import { rollFormula } from "./roll.ts";
import { chestOnBoard, outcomeCommands, parityError, sceneCommands, type Camera } from "./scene.ts";
import { announce } from "./words.ts";

// Spec 091 T034: one referee, answering the DOM room and the engine alike.
//
// The referee's state is the truth. While the engine runs, a step goes to
// the canvas as a key, so the engine's own rule answers first; the referee
// predicted the same step, and the two verdicts are compared (FR-012). A
// move the engine made and the rule refuses is sent back (FR-011). Without
// the engine, the referee answers alone.

/** How long a forwarded step waits for the engine before the rule decides. */
const ANSWER_MS = 1000;
const REDUCE = "(prefers-reduced-motion: reduce)";
/** How long the gold glitters over the opened chest before the art stands alone. */
const GLITTER_MS = 1800;

/** The opened chest's moment on the board: where the sparkle plays, in board pixels. */
export interface Glitter {
  x: number;
  y: number;
  size: number;
  /** Changes on every opening, so a restart's opening plays again. */
  n: number;
}

/** Each goal is announced on the page once, however often the room restarts. */
const told = new Set<GoalId>();

interface Pending {
  dir: Direction;
  predicted: Judged;
  timer: ReturnType<typeof setTimeout>;
}

export interface Said {
  text: string;
  /** Changes on every announcement, so the same words are said again. */
  n: number;
}

export function useRoom(script: RoomScript, frameId: string) {
  const referee = useMemo(() => createReferee(script), [script]);
  const grid = script.map.gridSize;
  const [state, setState] = useState<RoomState>(() => referee.start());
  const [hero, setHero] = useState<PartyHero>(PARTY[0]!);
  const [said, setSaid] = useState<Said | null>(null);
  const [rolling, setRolling] = useState(false);
  const [glitter, setGlitter] = useState<Glitter | null>(null);
  const glitterTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Event handlers read these, never a stale render's copy.
  const now = useRef(state);
  const heroNow = useRef(hero);
  const engine = useRef<{ state: FrameState; send: Send | null }>({ state: "poster", send: null });
  const pending = useRef<Pending[]>([]);

  const say = useCallback((text: string | null) => {
    if (text) setSaid((s) => ({ text, n: (s?.n ?? 0) + 1 }));
  }, []);

  /** Takes the referee's verdict: the state, the words, the goal, the engine. */
  const apply = useCallback(
    (judged: Judged, { quiet = false } = {}) => {
      const { state: next, outcome } = judged;
      now.current = next;
      setState(next);
      if (!quiet || outcome.goal) say(announce(script, next, outcome));
      if (outcome.goal && !told.has(outcome.goal)) {
        told.add(outcome.goal);
        window.dispatchEvent(
          new CustomEvent<GoalEventDetail>(GOAL_EVENT, { detail: { goal: outcome.goal } }),
        );
      }
      const send = engine.current.send;
      if (send) for (const c of outcomeCommands(script, heroNow.current, judged)) send(c);
    },
    [script, say],
  );

  /** The engine never answered: the rule decides, and the engine is told. */
  const settle = useCallback(
    (p: Pending) => {
      clearTimeout(p.timer);
      pending.current = pending.current.filter((q) => q !== p);
      apply(p.predicted);
      if (p.predicted.outcome.accepted) {
        engine.current.send?.(rollbackCommand(heroNow.current, p.predicted.state.hero, grid));
      }
    },
    [apply, grid],
  );

  const step = useCallback(
    (dir: Direction) => {
      const predicted = referee.step(now.current, dir);
      const canvas = canvasOf(frameId);
      if (canvas && engine.current.state === "running") {
        const p: Pending = { dir, predicted, timer: setTimeout(() => settle(p), ANSWER_MS) };
        pending.current.push(p);
        pressOnCanvas(canvas, dir);
        return;
      }
      apply(predicted);
      // A paused table still shows the hero: keep it where the rule put it.
      if (predicted.outcome.accepted) {
        engine.current.send?.(rollbackCommand(heroNow.current, predicted.state.hero, grid));
      }
    },
    [referee, frameId, apply, settle, grid],
  );

  /** FR-012: the rule's prediction against the engine's verdict. */
  const compare = useCallback((engineMoved: boolean) => {
    const p = pending.current.shift();
    if (!p) return;
    clearTimeout(p.timer);
    if (p.predicted.outcome.accepted !== engineMoved) {
      console.error(parityError(p.dir, now.current.hero, p.predicted, engineMoved));
    }
  }, []);

  const onEvent = useCallback(
    (event: EngineEvent) => {
      const state = now.current;
      switch (event.type) {
        case "upsert_token": {
          // The engine lets a press pick any token up, as the client does;
          // there the server refuses a move of a token that is not yours and
          // the board snaps back. Here the room is the server, and it
          // refuses the same way: the chest goes back where it stood.
          if (event.token.id === script.chest.tokenId) {
            engine.current.send?.(chestCommand(script, state.goals.includes("chest-opened")));
            return;
          }
          if (event.token.id !== HERO_ID) return;
          const from = toWorld(state.hero, grid);
          const to = { x: event.token.x, y: event.token.y };
          // The engine telling back a place the room sent it.
          if (from.x === to.x && from.y === to.y && pending.current.length === 0) return;
          compare(true);
          apply(referee.judgeMove(state, from, to, event.path));
          return;
        }
        case "movement_blocked":
          // Only the hero's bumps are the hero's. A dragged chest stopping
          // at a wall is the furniture being put back, not a move refused.
          if (event.tokenId !== undefined && event.tokenId !== HERO_ID) return;
          compare(false);
          apply(referee.refused(state, event.wallId));
          return;
        case "door_icon_pressed":
          apply(referee.doorPressed(state, event.wallId));
          return;
        case "interactionTriggered":
          // The chest put back on its square crosses into its own region.
          if (event.tokenId !== undefined && event.tokenId !== HERO_ID) return;
          // A step into the region already found the chest; say it once.
          apply(referee.entered(state, event.interactiveId), { quiet: true });
          return;
        case "sdkError":
          return;
      }
    },
    [referee, grid, apply, compare],
  );

  const latestEvent = useRef(onEvent);
  latestEvent.current = onEvent;
  useEffect(() => listen(frameId, (e) => latestEvent.current(e)), [frameId]);

  /** The room's scene, from scratch, with the hero where the referee has it. */
  const setup = useCallback(
    (send: Send) => {
      const reduced = matchMedia(REDUCE).matches;
      for (const c of sceneCommands(script, heroNow.current, now.current, reduced)) send(c);
      viewAs(frameId, HERO_ID);
    },
    [script, frameId],
  );

  const teardown = useCallback(
    (send: Send) => {
      for (const p of [...pending.current]) settle(p);
      for (const command of roomTeardown(script)) send(command);
    },
    [script, settle],
  );

  const onState = useCallback(
    (state: FrameState, send: Send | null) => {
      engine.current = { state, send };
      if (state !== "running") for (const p of [...pending.current]) settle(p);
    },
    [settle],
  );

  /** FR-017: the start again, without a reload or a fetch. */
  const startOver = useCallback(() => {
    for (const p of pending.current) clearTimeout(p.timer);
    pending.current = [];
    if (glitterTimer.current) clearTimeout(glitterTimer.current);
    setGlitter(null);
    apply(referee.reset());
    const send = engine.current.send;
    if (send) {
      for (const command of roomTeardown(script)) send(command);
      setup(send);
    }
  }, [referee, script, apply, setup]);

  /** FR-010: a new hero starts the room over. */
  const pick = useCallback(
    (next: PartyHero) => {
      heroNow.current = next;
      setHero(next);
      startOver();
    },
    [startOver],
  );

  const door = useCallback(() => apply(referee.toggleDoor(now.current)), [referee, apply]);

  /**
   * The chest opens on the board: gold glitters over its square for a moment
   * while the art changes to the open chest, and then the art stands alone.
   * Under reduced motion only the art changes. Needs the engine's camera, so
   * only while the table runs.
   */
  const sparkle = useCallback(() => {
    const e = running();
    if (!e || engine.current.state !== "running" || matchMedia(REDUCE).matches) return;
    const cam = JSON.parse(e.camera_state()) as Camera;
    if (glitterTimer.current) clearTimeout(glitterTimer.current);
    setGlitter((g) => ({ ...chestOnBoard(script, cam), n: (g?.n ?? 0) + 1 }));
    glitterTimer.current = setTimeout(() => setGlitter(null), GLITTER_MS);
  }, [script]);
  useEffect(() => () => {
    if (glitterTimer.current) clearTimeout(glitterTimer.current);
  }, []);

  const roll = useCallback(async () => {
    setRolling(true);
    try {
      const resolution = await rollFormula(script.roll.formula);
      const judged = referee.rolled(now.current, resolution);
      apply(judged);
      if (judged.outcome.accepted) {
        const throw_ = { rollerName: heroNow.current.name, id: throwId() };
        engine.current.send?.(diceCommand(resolution, throw_));
      }
      if (judged.outcome.announcement === "roll-succeeded") sparkle();
    } catch {
      say("The dice could not load in this browser.");
    } finally {
      setRolling(false);
    }
  }, [script, referee, apply, say, sparkle]);

  /** What the door button would do now: the referee's answer, untaken. */
  const doorPreview = useMemo(() => referee.toggleDoor(state).outcome, [referee, state]);

  return {
    state,
    hero,
    said,
    rolling,
    glitter,
    doorPreview,
    step,
    door,
    roll,
    pick,
    startOver,
    frame: { setup, teardown, onState },
  };
}
