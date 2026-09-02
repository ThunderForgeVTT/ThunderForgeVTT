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

export function WorldAppearanceSettingsCard({
  worldId,
  interfacePackId,
  gameSystemId,
  isGm,
  onChanged,
}: WorldAppearanceSettingsCardProps) {
  const [packs, setPacks] = useState<InterfacePackSummary[]>([]);
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
              <SelectItem key={pack.id} value={pack.id}>
                {pack.title}
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
