/**
 * Where a Game Master chooses how the table looks.
 *
 * The choice belongs to the world, not to the reader (FR-009), so this is a
 * GM control and a player sees it read-only. What a reader keeps is light or
 * dark: a Game Master picking a look is not picking a time of day for six
 * other people's rooms, and that is the accessibility escape hatch that
 * survived making the look table-wide.
 */
import { useEffect, useState } from "react";

import {
  listInterfacePacks,
  type InterfacePackSummary,
} from "@/api/interfacePacks";
import {
  listGameSystems,
  titleFor,
  type GameSystemSummary,
} from "@/api/gameSystems";
import { updateWorldInterfacePack } from "@/api/world";
import { BASE_PACK_ID } from "@/appearance/appearance-context";
import { Card } from "@/components/ui/card/Card";
import { Field } from "@/components/ui/field/Field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface WorldAppearanceSettingsCardProps {
  worldId: string;
  /** The world's current binding; `null` means the base pack. */
  interfacePackId: string | null;
  /** The world's system, so packs that cannot serve it are not offered. */
  gameSystemId: string | null;
  isGm: boolean;
  onChanged?: (interfacePackId: string | null) => void;
}

/**
 * Who a pack is for, in the reader's terms rather than the manifest's.
 *
 * `targets` holds system *ids* — `dnd5e`, `fate_core` — which is right for a
 * manifest and wrong for a person choosing between three metals. A pack that
 * targets nothing composes against any system, and saying so is the honest
 * version of the base pack's position: not a default that beats the others,
 * just the one that fits everywhere.
 */
function packAudience(
  pack: InterfacePackSummary,
  systems: GameSystemSummary[],
): string {
  if (pack.targets.length === 0) {
    return "Works with any system";
  }
  // `titleFor` falls back to the id, which matters here: a pack may target a
  // system this deployment does not have, and that target must still read as
  // something rather than vanish from the sentence.
  const named = pack.targets.map((id) => titleFor(systems, id)).join(", ");
  return `For ${named}`;
}

export function WorldAppearanceSettingsCard({
  worldId,
  interfacePackId,
  gameSystemId,
  isGm,
  onChanged,
}: WorldAppearanceSettingsCardProps) {
  const [packs, setPacks] = useState<InterfacePackSummary[]>([]);
  /** Only so a pack's `targets` can be shown as titles rather than ids. */
  const [systems, setSystems] = useState<GameSystemSummary[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /**
   * What is showing while a change is in flight.
   *
   * The committed value is the prop, so this is *only* the optimistic
   * in-flight one — mirroring the prop into state and syncing it back with an
   * effect is the shape that goes stale the moment the world updates from
   * anywhere else, which it now does on a world event.
   */
  const [pending, setPending] = useState<string | null>(null);
  const selected = pending ?? interfacePackId ?? BASE_PACK_ID;

  useEffect(() => {
    listGameSystems()
      .then((installed) => setSystems(installed.systems))
      .catch(() => setSystems([]));
  }, []);

  useEffect(() => {
    listInterfacePacks()
      .then(setPacks)
      .catch(() => setPacks([]));
  }, []);

  // A pack that targets nothing composes against any system; one that names
  // targets is only offered where it can actually render. Offering a Game
  // Master something that would show them an empty panel is worse than not
  // offering it.
  const offered = packs.filter(
    (pack) =>
      pack.targets.length === 0 ||
      (gameSystemId !== null && pack.targets.includes(gameSystemId)),
  );

  const active = packs.find(
    (pack) => pack.id === (interfacePackId ?? BASE_PACK_ID),
  );

  const commit = async (packId: string) => {
    setSaving(true);
    setError(null);
    try {
      // The base pack is the default rather than a binding, so choosing it
      // clears rather than stores. A world that has never chosen and a world
      // that chose the default are the same world.
      const stored = packId === BASE_PACK_ID ? null : packId;
      await updateWorldInterfacePack(worldId, stored);
      onChanged?.(stored);
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : "Failed to change the look",
      );
    } finally {
      setPending(null);
      setSaving(false);
    }
  };

  return (
    <Card data-testid="world-appearance-card">
      <h2>Appearance</h2>
      <p>
        How this world looks, for everyone at the table. Light and dark stay
        each person&apos;s own choice.
      </p>

      <Field
        label="Interface pack"
        htmlFor="interface-pack-select"
        // Named, never an empty value: a world that has chosen nothing is
        // drawn in the base pack, so "not yet assigned" would describe a state
        // this product does not have (FR-023).
        hint={
          active
            ? `Currently ${active.title}${active.description ? ` — ${active.description}` : ""}`
            : undefined
        }
      >
        <Select
          value={selected}
          disabled={!isGm || saving}
          onValueChange={(value) => {
            setPending(value);
            void commit(value);
          }}
        >
          <SelectTrigger
            id="interface-pack-select"
            data-testid="interface-pack-select"
            aria-readonly={!isGm}
          >
            <SelectValue placeholder="Choose a look" />
          </SelectTrigger>
          <SelectContent>
            {offered.map((pack) => (
              // In title order, as the server listed them, with no pinned
              // position and no badge for the base pack (FR-007).
              //
              // Each says what it is for. "Forged Steel" is a code name and
              // reads like one: nothing on the screen connected it to 5e, so
              // choosing a look meant picking between three metals and finding
              // out afterwards. The pack already carries both facts — which
              // systems it targets, and a sentence about its arrangement — and
              // they were only ever shown for the pack already in force.
              <SelectItem key={pack.id} value={pack.id}>
                <span className="flex flex-col gap-0.5 py-0.5">
                  <span className="font-medium">{pack.title}</span>
                  <span className="text-xs text-muted-foreground">
                    {packAudience(pack, systems)}
                    {pack.description ? ` · ${pack.description}` : ""}
                  </span>
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>

      {!isGm && (
        <p data-testid="interface-pack-readonly">
          Only this world&apos;s Game Master can change how it looks.
        </p>
      )}
      {error && <p data-testid="interface-pack-error">{error}</p>}
    </Card>
  );
}
