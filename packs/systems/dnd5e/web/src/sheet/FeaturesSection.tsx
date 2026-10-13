import { ListField } from "./fields";
import { stringList, type Json } from "./values";

/** Features, traits and feats, each a list of lines. */
export default function FeaturesSection({
  canEdit,
  isPending,
  traitData,
  writeTraits,
}: {
  canEdit: boolean;
  isPending: boolean;
  traitData: Json;
  writeTraits: (patch: Json) => Promise<void>;
}) {
  return (
    <div className="grid gap-3 @md:grid-cols-2">
      <ListField
        id="dnd5e-traits"
        label="Features & traits"
        canEdit={canEdit}
        items={stringList(traitData.traits)}
        rows={8}
        isSaving={isPending}
        onSave={(items) => writeTraits({ traits: items })}
      />
      <ListField
        id="dnd5e-feats"
        label="Feats"
        canEdit={canEdit}
        items={stringList(traitData.feats)}
        rows={8}
        isSaving={isPending}
        onSave={(items) => writeTraits({ feats: items })}
      />
    </div>
  );
}
