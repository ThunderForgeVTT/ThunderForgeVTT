import { createElement, useEffect, useMemo, useRef, useState } from "react";
import type { WorldStore } from "@/engine/world/store";
import { useSceneTokens } from "@/engine/world/useSceneTokens";
import { useAuth } from "@/hooks/useAuth";
import { AttackFlow } from "./AttackFlow/AttackFlow";
import { mayEditActor } from "@/pages/world/actor/actorEditRight";
import { resolveActorSheet } from "@/pages/world/actor/systemActorSheets";
import type { WorldActorRecord } from "@/types/actor";
import { cn } from "@/lib/utils";
import { CharacterRollButtons, RollFeedback } from "./CharacterRollButtons";
import type { CharacterRoll } from "./characterRolls";
import { useCharacterRoller } from "./useCharacterRoller";
import { useCharacterRolls } from "./useCharacterRolls";

/**
 * A player's own character, inside the dock, while the table stays live.
 *
 * # Why this exists when a perfectly good actor page already does
 *
 * Spec 031 US2. The actors pane used to link away for everyone, and for a
 * player that is the difference between playing and administering: making a
 * roll cost them the map, the engine tore down, and coming back meant a
 * reload. A Game Master keeps the new tab (FR-002) because they are inspecting
 * one of many characters beside the map; a player has exactly one character
 * and needs it *on* the map's screen. The two halves of View differ on
 * purpose.
 *
 * # Why it renders the system's own sheet
 *
 * `SYSTEM_ACTOR_SHEETS` is the same registry the full actor page mounts from,
 * so this is the sheet the active game system supplies, compacted — not a
 * second, parallel sheet (spec 031 Assumptions). Writing a dock-sized sheet by
 * hand was the obvious alternative and was rejected: it would drift from the
 * full page the first time a pack changed anything, and a player would be
 * looking at a different character to the one their GM sees.
 *
 * # Why the sheet can be edited here, for whoever may edit it
 *
 * The sheet is edited with the viewer's real edit right — `mayEditActor`,
 * the same answer the full actor page gives, so the two cannot disagree. It
 * used to be mounted read-only, on the reasoning that a 22rem column during
 * play is the wrong place to restat a character. That holds for restatting
 * and fails for play: a condition gained, a point spent, a skill just
 * discovered are all changes to the sheet that happen *at the table*, and a
 * player sent to another tab to record one has lost the map — the very thing
 * this component exists to prevent. For a system played without a map at all
 * the dock sheet is the whole of the player's seat, and read-only there meant
 * a character that could not be kept up to date where it is played.
 *
 * Nothing is granted by this. A Viewer still gets a read-only sheet, and the
 * server refuses a write the caller may not make whatever the sheet offered
 * (Principle III).
 *
 * # Why it opens read, with an Edit toggle
 *
 * It used to open straight into its inputs, with no second control whose
 * only job was to unlock the first. The owner's phone try-out (spec 102)
 * turned that round: at the table a sheet is mostly read and rolled from,
 * and a page of boxes is clutter between a player and their numbers. So it
 * opens read, a tap on a number the pack offers rolls it (`onRoll`), and
 * Edit, shown only to whoever may edit, turns the boxes on. The same tap
 * never both rolls and edits, because `onRoll` is withheld while editing.
 *
 * # Why the rolls are separate from the sheet
 *
 * Genie's `CharacterSheet` presents scores; it has no roll callback to hand
 * one to. Rather than reach into a pack to add one — which is spec 032's job,
 * not this one's — the rolls are derived alongside it in `characterRolls.ts`
 * from the same data the sheet is drawing, and go out through `rollDice`.
 *
 * Constitution Principle I: nothing here is canvas state. The roll is decided
 * entirely by the server, and the board animates it from its world event
 * (spec 081 FR-013), so the buttons are `CharacterRollButtons`, shared with
 * the sheet page.
 */

export interface InPaneCharacterSheetProps {
  worldId: string;
  /** The world store, holding the scene's tokens. */
  worldStore: WorldStore;
  /** The scene in play, where this character's token is and its targets are. */
  sceneId?: string | null;
  actor: WorldActorRecord;
  /** Whether the viewer runs the world; it decides the roll picker's choices. */
  isGm?: boolean;
  /**
   * Returns the pane to whatever it was showing before (FR-002/US2 #3). A
   * phone's board sheet closes itself, so it passes none (spec 102).
   */
  onDismiss?: () => void;
  /**
   * `phone` (spec 102) pins the name and a hot bar of rolls, actions first,
   * above the sheet, and lets the sheet run at full size in the board
   * sheet's own scroll instead of a cramped box of its own.
   */
  layout?: "dock" | "phone";
}

/** Actions (an ability's attack, damage, healing) lead the hot bar. */
const actionsFirst = (rolls: CharacterRoll[]) => [
  ...rolls.filter((roll) => roll.key.startsWith("ability-")),
  ...rolls.filter((roll) => !roll.key.startsWith("ability-")),
];

export function InPaneCharacterSheet({
  worldId,
  worldStore,
  sceneId = null,
  actor,
  isGm = false,
  onDismiss,
  layout = "dock",
}: InPaneCharacterSheetProps) {
  const { user } = useAuth();
  const [attacking, setAttacking] = useState<CharacterRoll | null>(null);
  const [editing, setEditing] = useState(false);
  const roller = useCharacterRoller(worldId, isGm);
  const phone = layout === "phone";
  const mayEdit = mayEditActor(actor);
  const sheetRef = useRef<HTMLDivElement>(null);
  const sheet = resolveActorSheet(actor.gameSystemId);

  // Spec 046: an attack is made from this character's token on the scene, at
  // another token there.
  const sceneTokens = useSceneTokens(worldStore, sceneId);

  /** This character's token here: the viewer's own, else the first. */
  const attackerToken = useMemo(() => {
    const mine = sceneTokens.filter((token) => token.actorId === actor.id);
    return (
      mine.find((token) => token.ownerUserId === user?.id) ?? mine[0] ?? null
    );
  }, [sceneTokens, actor.id, user?.id]);

  // The phone's sheet may be scrolled deep into the skills; bring the top,
  // and the attack under it, back into view.
  useEffect(() => {
    if (phone && attacking)
      sheetRef.current?.scrollIntoView({ block: "start" });
  }, [phone, attacking]);

  const derived = useCharacterRolls(worldId, actor);
  const rolls = phone ? actionsFirst(derived) : derived;
  const showRolls = !(sheet && rolls.length === 0);
  const showFeedback =
    rolls.length > 0 || roller.result !== null || roller.rollError !== null;

  /*
    A system that draws its own sheet and rolls from it (Roll for Shoes)
    has nothing here, and an empty box under a working sheet reads as
    something broken. The section is for what the sheet does not offer;
    it says "nothing" only when there is no sheet to have offered it.
  */
  const rollSection = showRolls ? (
    <section className="grid gap-1.5" data-testid="in-pane-sheet-rolls">
      <h3
        className={cn(
          "text-xs font-semibold tracking-widest text-muted-foreground uppercase",
          phone && "sr-only",
        )}
      >
        Rolls
      </h3>
      {rolls.length === 0 ? (
        <p
          className="text-xs text-muted-foreground"
          data-testid="in-pane-sheet-no-rolls"
        >
          Nothing on this character carries a formula to roll yet.
        </p>
      ) : (
        <CharacterRollButtons
          rolls={rolls}
          roller={roller}
          testIdPrefix="in-pane"
          layout={phone ? "hotbar" : "list"}
          onAttack={setAttacking}
          attackUnavailable={
            attackerToken
              ? null
              : `${actor.label} has no token on this scene to attack from`
          }
        />
      )}
      {showFeedback ? (
        <RollFeedback roller={roller} testIdPrefix="in-pane" />
      ) : null}
    </section>
  ) : showFeedback ? (
    <RollFeedback roller={roller} testIdPrefix="in-pane" />
  ) : null;

  const attackFlow =
    attacking?.attackAbilityId && attackerToken ? (
      <AttackFlow
        // A new attack starts clean: which target, reaction or not.
        key={attacking.key}
        worldId={worldId}
        attackerTokenId={attackerToken.tokenId}
        abilityId={attacking.attackAbilityId}
        abilityName={attacking.label.replace(/ \(attack\)$/, "")}
        tokens={sceneTokens}
        onClose={() => {
          // Back to the attack that opened the flow, which stays on the
          // sheet: closing must not drop a keyboard user on the page.
          const key = attacking.key;
          setAttacking(null);
          sheetRef.current
            ?.querySelector<HTMLElement>(
              `[data-testid="in-pane-roll-${CSS.escape(key)}"]`,
            )
            ?.focus();
        }}
      />
    ) : null;

  return (
    <div
      ref={sheetRef}
      className="grid gap-3"
      data-testid="in-pane-character-sheet"
    >
      <div
        className={
          phone
            ? "sticky -top-4 z-10 -mx-4 -mt-4 grid gap-2 border-b border-border bg-background px-4 pt-3 pb-2"
            : "contents"
        }
        data-testid="in-pane-sheet-top"
      >
        <header className="flex items-center gap-2">
          {onDismiss ? (
            <button
              type="button"
              onClick={onDismiss}
              data-testid="in-pane-sheet-dismiss"
              aria-label="Back to actors"
              className="rounded border border-border px-2 py-1 text-xs transition-colors hover:bg-muted"
            >
              ‹ Back
            </button>
          ) : null}
          <span
            className={cn(
              "min-w-0 flex-1 truncate font-semibold",
              phone ? "text-base" : "text-sm",
            )}
          >
            {actor.label}
          </span>
          {sheet && mayEdit ? (
            <button
              type="button"
              onClick={() => setEditing((on) => !on)}
              aria-pressed={editing}
              data-testid="in-pane-sheet-edit"
              className={cn(
                "border border-border font-medium transition-colors hover:bg-muted",
                phone
                  ? "min-h-11 min-w-16 rounded-xl px-4 text-sm"
                  : "rounded px-2 py-1 text-xs",
                editing &&
                  "border-primary bg-primary text-primary-foreground hover:bg-primary/90",
              )}
            >
              {editing ? "Done" : "Edit"}
            </button>
          ) : null}
        </header>
        {phone ? rollSection : null}
      </div>

      {/*
        On the phone the attack opens right under the hot bar that started
        it: at the foot of a full sheet it opened out of sight, and the tap
        read as doing nothing.
      */}
      {phone ? attackFlow : null}

      {/*
        The pack's sheet was drawn for a page, not a 22rem column. Scaling its
        type down and letting it scroll inside its own box keeps it legible
        here without every pack needing a second layout — and without this
        component knowing anything about what that sheet contains. A phone's
        board sheet already scrolls, so there it runs at full size.
      */}
      {sheet ? (
        <div
          className={
            phone
              ? "text-sm"
              : "max-h-[45vh] overflow-y-auto rounded-lg border border-border text-xs [&_h1]:text-base [&_h2]:text-sm [&_h3]:text-sm"
          }
          data-testid="in-pane-sheet-body"
          data-editing={editing ? "true" : "false"}
        >
          {/*
            `createElement` rather than `<Sheet />` with a capitalised local:
            a component *value* chosen at render time is exactly what
            `react-hooks/static-components` exists to catch, and the rule is
            right in general — it just cannot see that this one comes from a
            module-level registry keyed by a string. Writing the call out
            keeps the rule enforced everywhere else rather than disabled here.
          */}
          {createElement(sheet, {
            actor,
            canEdit: editing && mayEdit,
            onRoll: editing
              ? undefined
              : ({ label, formula }) =>
                  void roller.roll(`sheet-${label}`, label, formula),
          })}
        </div>
      ) : (
        /*
          FR-002 still holds when a system ships no sheet: the player stays in
          the pane and is told plainly why it is bare. Falling back to the Game
          Master's new tab was the tempting alternative and is exactly wrong —
          it would take the player away from the map for a page that has no
          more to show them than this does.
        */
        <p
          className="rounded-lg border border-dashed border-border px-2 py-3 text-xs text-muted-foreground"
          data-testid="in-pane-sheet-unavailable"
        >
          {actor.gameSystemId
            ? `${actor.gameSystemId} supplies no character sheet, so there is nothing to draw here.`
            : "This character belongs to no game system, so there is no sheet to draw."}{" "}
          Anything recorded against them can still be rolled below.
        </p>
      )}

      {phone ? null : rollSection}

      {phone ? null : attackFlow}
    </div>
  );
}
