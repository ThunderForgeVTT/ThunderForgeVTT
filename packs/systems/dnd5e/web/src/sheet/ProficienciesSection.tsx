import { ListField } from "./fields";
import { stringList, type Json } from "./values";

/** Armour, weapons, tools and languages, each a list of lines. */
export default function ProficienciesSection({
  canEdit,
  isPending,
  proficiencyData,
  writeProficiencies,
}: {
  canEdit: boolean;
  isPending: boolean;
  proficiencyData: Json;
  writeProficiencies: (patch: Json) => Promise<void>;
}) {
  return (
    <div className="grid gap-3">
      <ListField
        id="dnd5e-armor-proficiencies"
        label="Armour"
        canEdit={canEdit}
        items={stringList(proficiencyData.armor)}
        isSaving={isPending}
        onSave={(items) => writeProficiencies({ armor: items })}
      />
      <ListField
        id="dnd5e-weapon-proficiencies"
        label="Weapons"
        canEdit={canEdit}
        items={stringList(proficiencyData.weapons)}
        isSaving={isPending}
        onSave={(items) => writeProficiencies({ weapons: items })}
      />
      <ListField
        id="dnd5e-tool-proficiencies"
        label="Tools"
        canEdit={canEdit}
        items={stringList(proficiencyData.tools)}
        isSaving={isPending}
        onSave={(items) => writeProficiencies({ tools: items })}
      />
      <ListField
        id="dnd5e-languages"
        label="Languages"
        canEdit={canEdit}
        items={stringList(proficiencyData.languages)}
        isSaving={isPending}
        onSave={(items) => writeProficiencies({ languages: items })}
      />
    </div>
  );
}
