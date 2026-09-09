import { useCallback, useEffect, useState } from "react";
import {
  getSystemChecks,
  rollCheck,
  type CheckResolution,
  type SystemCheck,
} from "@/api/systemChecks";
import { GraphQLRequestError } from "@/api/graphqlClient";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 036 US3b (FR-036, FR-037): rolling a check from a character sheet.
 *
 * # The button is all this is
 *
 * It sends a check's **id**. Not a formula, not a modifier, not a total. The
 * server looks the check up in the system's own manifest, fills the formula's
 * placeholders from the actor's published values, and rolls it on the one path
 * ADR-044 permits to produce a result — the same call, with the same record,
 * as a roll made at the table (FR-036). That is why this file contains no
 * arithmetic: any it contained would be one system's rules living in shared
 * presentation code, which is what spec 029 spent a feature removing.
 *
 * # A system that declares no checks gets no buttons
 *
 * FR-037, and it renders nothing at all — not a disabled button, not an
 * explanation. Seven of the eight bundled packs declare none, and a sheet
 * telling a Blades player that their system "has no checks" would be noise
 * on every sheet in the product for the sake of one.
 *
 * # When the server is gone (FR-039, FR-040)
 *
 * A check is an adjudicated action: only the server may decide it. So a sheet
 * that has lost the server **refuses**, says it has lost the server, and names
 * the play field as where to go — and it records nothing. Nothing local,
 * nothing queued for replay, nothing at the table. A queued roll would be a
 * result decided at a time nobody chose and delivered to a table that had
 * moved on, and a locally-shown one would be a number the table never saw.
 *
 * The refusal is the transport failure, surfaced honestly. `GraphQLRequestError`
 * carries `transport` for exactly this: "the server never heard you" and "the
 * server heard you and said no" are different sentences, and only one of them
 * should send somebody to another screen.
 *
 * # Why the answer is small
 *
 * The roll goes to the table; it is a world event and every client sees it.
 * What is shown here is a confirmation that the click landed, not a second
 * home for the result — a result that lived in two places could disagree with
 * itself, and the table is the one that counts.
 */
export function SystemChecksPanel({
  worldId,
  actorId,
}: {
  worldId: string;
  actorId: string;
}) {
  const [checks, setChecks] = useState<SystemCheck[] | null>(null);
  const [rolling, setRolling] = useState<string | null>(null);
  const [last, setLast] = useState<{
    label: string;
    resolution: CheckResolution;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getSystemChecks(worldId)
      .then(setChecks)
      // A world whose checks could not be read shows no buttons rather than
      // broken ones. The sheet's subject is the character, and failing to
      // offer a roll must not take the numbers down with it.
      .catch(() => setChecks([]));
  }, [worldId]);

  useEffect(load, [load]);

  const roll = async (check: SystemCheck) => {
    setRolling(check.id);
    setError(null);
    try {
      const resolution = await rollCheck({
        worldId,
        actorId,
        checkId: check.id,
      });
      setLast({ label: check.label, resolution });
    } catch (caught) {
      // FR-039. Nothing is queued and nothing is stored on either branch —
      // `setError` is the whole of what happens, which is what makes FR-040
      // true rather than merely intended.
      if (caught instanceof GraphQLRequestError && caught.transport) {
        setError(
          "This roll needs the server, and it cannot be reached from here. " +
            "Nothing was rolled and nothing was saved — try it again from the " +
            "play field once you are back.",
        );
      } else {
        // The likeliest server refusal is "this character has no value for X
        // to check against" — an unfilled sheet, which is something the
        // person can go and fix, so it is repeated as the server worded it.
        setError(
          caught instanceof Error ? caught.message : "That roll was refused.",
        );
      }
    } finally {
      setRolling(null);
    }
  };

  if (!checks || checks.length === 0) return null;

  // Grouped as the system grouped them, in the order it declared them. The
  // groups are the pack's ("abilities", "skills"); this only preserves them.
  const groups = new Map<string, SystemCheck[]>();
  for (const check of checks) {
    const key = check.group ?? "";
    const existing = groups.get(key);
    if (existing) existing.push(check);
    else groups.set(key, [check]);
  }

  return (
    <section className="grid gap-3" data-testid="system-checks">
      <h3 className="font-semibold">Checks</h3>

      {[...groups.entries()].map(([group, entries]) => (
        <div key={group || "ungrouped"} className="grid gap-1.5">
          {group ? (
            <p className="text-xs uppercase tracking-wide text-muted-foreground">
              {group}
            </p>
          ) : null}
          <div className="flex flex-wrap gap-2">
            {entries.map((check) => (
              <Button
                key={check.id}
                type="button"
                variant="secondary"
                size="sm"
                disabled={rolling !== null}
                onClick={() => void roll(check)}
                data-testid={`system-check-${check.id}`}
              >
                {rolling === check.id ? "Rolling..." : check.label}
              </Button>
            ))}
          </div>
        </div>
      ))}

      {last ? (
        <StatusBadge variant="info" data-testid="system-check-result">
          {last.label}: {last.resolution.resultValue}
          {last.resolution.resultKind === "SUCCESS_COUNT" ? " successes" : ""}
        </StatusBadge>
      ) : null}

      {error ? (
        <StatusBadge variant="danger" data-testid="system-check-error">
          {error}
        </StatusBadge>
      ) : null}
    </section>
  );
}
