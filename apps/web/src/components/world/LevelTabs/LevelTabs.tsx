import { useRef, useState } from "react";
import type { SceneLevel } from "@/api/levels";
import { Button } from "@/components/ui/button/Button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { playerSeesLevelName } from "@/engine/world/sync";
import type { SceneLevelActions } from "@/pages/world/useSceneLevels";

export interface LevelTabsProps {
  /** Every level this viewer may read, lowest first. */
  levels: SceneLevel[];
  /** The level on the board. */
  level: SceneLevel | null;
  isGm: boolean;
  /** Tokens selected on the board, for "move selected tokens to…". */
  selectedTokenIds: string[];
  actions: SceneLevelActions;
}

/**
 * The levels of the scene on the board: which one this is, and — for a Game
 * Master — the others, and the means of arranging them.
 *
 * Two different things share this component because they answer one question
 * in one place on screen, "which floor am I looking at":
 *
 * - a **player** is told the name of the level they are on and nothing else.
 *   They cannot switch by hand; their token's feet do that. On a scene with
 *   one level they are shown nothing at all, so a scene made before levels
 *   existed looks exactly as it did.
 * - a **Game Master** gets a tab per level, because they arrange floors nobody
 *   has reached yet, and a panel for the level they are on.
 *
 * It asks for nothing itself. Every change goes through `actions`, which is
 * the page's level state (`useSceneLevels`) — that is also what re-reads the
 * list afterwards, so this never holds a copy of it.
 */
export function LevelTabs({
  levels,
  level,
  isGm,
  selectedTokenIds,
  actions,
}: LevelTabsProps) {
  const [managing, setManaging] = useState(false);

  if (!level) {
    return null;
  }

  if (!isGm) {
    if (!playerSeesLevelName(levels)) {
      return null;
    }
    return (
      <div
        role="status"
        data-testid="level-name"
        data-level-id={level.levelId}
        className="pointer-events-auto rounded-md border border-border bg-background/90 px-3 py-1 text-sm font-semibold text-foreground shadow"
      >
        {level.name}
      </div>
    );
  }

  return (
    <div
      className="pointer-events-auto grid max-w-full gap-2 rounded-md border border-border bg-background/90 p-1.5 text-foreground shadow"
      data-testid="level-tabs"
    >
      <div className="flex flex-wrap items-center gap-1">
        <div
          role="tablist"
          aria-label="Levels"
          className="flex flex-wrap gap-1"
        >
          {levels.map((each) => {
            const selected = each.levelId === level.levelId;
            return (
              <button
                key={each.levelId}
                type="button"
                role="tab"
                aria-selected={selected}
                data-testid="level-tab"
                data-level-id={each.levelId}
                data-entry={each.isEntry ? "true" : "false"}
                onClick={() => actions.select(each.levelId)}
                className={
                  selected
                    ? "rounded bg-primary px-2.5 py-1 text-sm font-semibold text-primary-foreground"
                    : "rounded px-2.5 py-1 text-sm text-foreground hover:bg-muted"
                }
              >
                {each.name}
                {each.isEntry ? (
                  <span className="sr-only"> (the scene opens here)</span>
                ) : null}
                {each.tokenCount !== null ? (
                  <span
                    className="ml-1.5 text-xs opacity-80"
                    data-testid="level-tab-token-count"
                    aria-label={`${each.tokenCount} tokens`}
                  >
                    {each.tokenCount}
                  </span>
                ) : null}
              </button>
            );
          })}
        </div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-expanded={managing}
          data-testid="level-manage-toggle"
          onClick={() => setManaging((open) => !open)}
        >
          {managing ? "Done" : "Levels…"}
        </Button>
      </div>

      {managing ? (
        <LevelManager
          // A panel about one level: its half-typed name belongs to that
          // level and must not follow the Game Master to the next tab.
          key={level.levelId}
          levels={levels}
          level={level}
          selectedTokenIds={selectedTokenIds}
          actions={actions}
        />
      ) : null}
    </div>
  );
}

function LevelManager({
  levels,
  level,
  selectedTokenIds,
  actions,
}: {
  levels: SceneLevel[];
  level: SceneLevel;
  selectedTokenIds: string[];
  actions: SceneLevelActions;
}) {
  const [name, setName] = useState(level.name);
  const [newName, setNewName] = useState("");
  const fileInput = useRef<HTMLInputElement | null>(null);
  // Spec 088 FR-093: a new map is walled at its edges unless this is unticked.
  const [wallEdges, setWallEdges] = useState(true);

  const index = levels.findIndex((each) => each.levelId === level.levelId);
  const others = levels.filter((each) => each.levelId !== level.levelId);
  const renamed = name.trim();
  const added = newName.trim();

  return (
    <div className="grid w-72 max-w-full gap-3" data-testid="level-manager">
      <div className="grid gap-1.5">
        <Label htmlFor="level-name-input">This level</Label>
        <div className="flex gap-1">
          <Input
            id="level-name-input"
            data-testid="level-rename-input"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="level-rename-submit"
            disabled={!renamed || renamed === level.name}
            onClick={() => void actions.rename(level.levelId, renamed)}
          >
            Rename
          </Button>
        </div>
        <div className="flex flex-wrap gap-1">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="level-move-down"
            disabled={index <= 0}
            onClick={() => void actions.shift(level.levelId, -1)}
          >
            Move lower
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="level-move-up"
            disabled={index < 0 || index >= levels.length - 1}
            onClick={() => void actions.shift(level.levelId, 1)}
          >
            Move higher
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="level-make-entry"
            disabled={level.isEntry}
            onClick={() => void actions.makeEntry(level.levelId)}
          >
            {level.isEntry ? "The scene opens here" : "Open the scene here"}
          </Button>
        </div>
        <div className="flex flex-wrap gap-1">
          <input
            ref={fileInput}
            type="file"
            accept="image/*"
            className="hidden"
            data-testid="level-background-input"
            onChange={(event) => {
              const file = event.target.files?.[0];
              // Cleared so choosing the same file twice asks twice.
              event.target.value = "";
              if (file) {
                void actions.setBackground(level.levelId, file, wallEdges);
              }
            }}
          />
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="level-set-background"
            onClick={() => fileInput.current?.click()}
          >
            {level.backgroundUrl ? "Replace map" : "Set map"}
          </Button>
          <label className="flex items-center gap-2 text-xs text-muted-foreground">
            <Checkbox
              checked={wallEdges}
              data-testid="level-wall-edges"
              onCheckedChange={(checked) => setWallEdges(checked === true)}
            />
            Wall the map's edges
          </label>
          {level.backgroundUrl ? (
            <Button
              type="button"
              variant="secondary"
              size="sm"
              data-testid="level-clear-background"
              onClick={() => void actions.clearBackground(level.levelId)}
            >
              Remove map
            </Button>
          ) : null}
          <Button
            type="button"
            variant="danger"
            size="sm"
            data-testid="level-delete"
            // The server refuses these too, and says why. Disabling the two
            // it can know about saves a Game Master the round trip; a level
            // with something still standing on it is left to the server.
            disabled={level.isEntry || levels.length <= 1}
            onClick={() => void actions.remove(level.levelId)}
          >
            Delete level
          </Button>
        </div>
      </div>

      {selectedTokenIds.length > 0 && others.length > 0 ? (
        <div className="grid gap-1.5" data-testid="level-move-tokens">
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Move {selectedTokenIds.length} selected{" "}
            {selectedTokenIds.length === 1 ? "token" : "tokens"} to level…
          </p>
          <div className="flex flex-wrap gap-1">
            {others.map((each) => (
              <Button
                key={each.levelId}
                type="button"
                variant="secondary"
                size="sm"
                data-testid="level-move-tokens-to"
                data-level-id={each.levelId}
                onClick={() =>
                  void actions.moveTokens(selectedTokenIds, each.levelId)
                }
              >
                {each.name}
              </Button>
            ))}
          </div>
        </div>
      ) : null}

      <div className="grid gap-1.5">
        <Label htmlFor="level-new-name-input">New level</Label>
        <div className="flex gap-1">
          <Input
            id="level-new-name-input"
            data-testid="level-add-input"
            value={newName}
            placeholder="e.g. Upstairs"
            onChange={(event) => setNewName(event.target.value)}
          />
          <Button
            type="button"
            size="sm"
            data-testid="level-add-submit"
            disabled={!added}
            onClick={() => {
              setNewName("");
              void actions.add(added);
            }}
          >
            Add
          </Button>
        </div>
      </div>
    </div>
  );
}
