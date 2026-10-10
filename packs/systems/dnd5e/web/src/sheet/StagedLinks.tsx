import { useActorStagedLinks } from "@thunderforge/host";
import { cardClass, hintClass } from "../components/styles";
import LinkedContent from "./LinkedContent";
import { stagedEntries } from "./stagedEntries";

/**
 * Spec 048: what came in with a sheet that the world does not hold yet. The
 * host's ability and inventory panels list only the world's pieces.
 */
export default function StagedLinks({ actorId }: { actorId: string }) {
  const entries = stagedEntries(useActorStagedLinks(actorId).links);
  if (entries.length === 0) return null;
  return (
    <section
      aria-label="Brought in with a sheet"
      className={`${cardClass} mt-4 grid gap-3`}
    >
      <p className={hintClass}>
        Brought in with a sheet. The GM decides whether the world takes each
        one; until then it cannot be used in play.
      </p>
      <LinkedContent entries={entries} />
    </section>
  );
}
