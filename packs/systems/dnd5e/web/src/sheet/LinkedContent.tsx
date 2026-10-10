import { hintClass, sectionHeadingClass } from "../components/styles";

/**
 * One spell, feature or item linked to the character. `staged` is set when
 * the piece came in with an imported sheet and the world does not hold it
 * yet: the GM has not looked at it, or has declined it.
 */
export interface LinkedEntry {
  id: string;
  kind: "spell" | "feature" | "species_trait" | "feat" | "item";
  name: string;
  staged?: "pending" | "declined";
}

const GROUPS: { kinds: LinkedEntry["kind"][]; title: string }[] = [
  { kinds: ["spell"], title: "Spells" },
  { kinds: ["feature", "species_trait", "feat"], title: "Features" },
  { kinds: ["item"], title: "Items" },
];

const MARK: Record<NonNullable<LinkedEntry["staged"]>, string> = {
  pending: "awaiting the GM",
  declined: "declined by the GM",
};

/**
 * Spec 048: what the character carries and knows, from the world's
 * compendium or brought in with a sheet. A staged piece is listed with its
 * mark and cannot be used in play until the GM adopts it.
 */
export default function LinkedContent({ entries }: { entries: LinkedEntry[] }) {
  if (entries.length === 0) return null;
  return (
    <div className="grid gap-3" data-testid="dnd5e-linked-content">
      {GROUPS.map(({ kinds, title }) => {
        const group = entries
          .filter((entry) => kinds.includes(entry.kind))
          .sort((a, b) => a.name.localeCompare(b.name));
        if (group.length === 0) return null;
        return (
          <div key={title} className="grid gap-1">
            <div className={sectionHeadingClass}>{title}</div>
            <ul className="grid gap-1">
              {group.map((entry) => (
                <li
                  key={entry.id}
                  className="flex flex-wrap items-baseline gap-x-2 text-sm"
                  data-testid={`dnd5e-linked-${entry.id}`}
                  data-staged={entry.staged ?? "no"}
                  data-kind={entry.kind}
                >
                  <span
                    className={
                      entry.staged === "declined"
                        ? "text-muted-foreground line-through"
                        : entry.staged
                          ? "text-muted-foreground"
                          : undefined
                    }
                  >
                    {entry.name}
                  </span>
                  {entry.staged ? (
                    <span className={hintClass}>{MARK[entry.staged]}</span>
                  ) : null}
                </li>
              ))}
            </ul>
          </div>
        );
      })}
    </div>
  );
}
