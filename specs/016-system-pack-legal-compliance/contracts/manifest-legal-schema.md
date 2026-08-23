# Contract: `system.json` Manifest `legal` Field (new)

## Shape (JSON Schema fragment, added to the existing manifest contract governed by ADR 027)

```json
{
  "legal": {
    "type": "object",
    "required": ["licenseName", "attributionText"],
    "properties": {
      "licenseName": { "type": "string", "minLength": 1 },
      "attributionText": { "type": "string", "minLength": 1 },
      "requiredNotice": { "type": ["string", "null"] },
      "disclaimer": { "type": ["string", "null"] },
      "trademarkRestrictions": {
        "type": "array",
        "items": { "type": "string" },
        "default": []
      },
      "requiredUiPlacement": { "type": ["string", "null"] },
      "sourceUrl": { "type": ["string", "null"] }
    }
  }
}
```

`legal` is a **required top-level key** of `system.json`, sibling to `id`, `title`, `version`, `license`, `skills`, `abilities`, `data_types`. It does not replace the existing free-text `license` string (kept for backward-compat / short display); `legal` is the structured, render-ready expansion of it.

## TypeScript contract (`apps/web/src/contexts/GameSystemContext.tsx`)

```typescript
export type SystemManifestLegal = {
  licenseName: string;
  attributionText: string;
  requiredNotice?: string | null;
  disclaimer?: string | null;
  trademarkRestrictions?: string[];
  requiredUiPlacement?: string | null;
  sourceUrl?: string | null;
};

export type SystemManifest = {
  id: string;
  title: string;
  version: string;
  legal: SystemManifestLegal;
  [key: string]: any;
};
```

## Rust contract (`src/server/src/systems.rs`)

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemManifestLegal {
    pub license_name: String,
    pub attribution_text: String,
    #[serde(default)]
    pub required_notice: Option<String>,
    #[serde(default)]
    pub disclaimer: Option<String>,
    #[serde(default)]
    pub trademark_restrictions: Vec<String>,
    #[serde(default)]
    pub required_ui_placement: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
}
```

Field naming: the wire format (`system.json`) uses camelCase (`licenseName`) per existing manifest convention (`esmodules`, `spellSlots`); the Rust struct uses `#[serde(rename_all = "camelCase")]` on the containing struct (matching whatever convention `systems.rs` already applies to `data_types` et al.) so `snake_case` stays idiomatic Rust internally.

## Validator contract (extends existing pack validation, e.g. `packs/systems/dnd5e/server/src/validators.rs`)

```rust
pub fn validate_legal(manifest: &SystemManifestJson) -> Result<(), ValidationError> {
    let legal = manifest.legal.as_ref()
        .ok_or_else(|| ValidationError::MissingField("legal"))?;
    if legal.license_name.trim().is_empty() {
        return Err(ValidationError::EmptyField("legal.licenseName"));
    }
    if legal.attribution_text.trim().is_empty() {
        return Err(ValidationError::EmptyField("legal.attributionText"));
    }
    Ok(())
}
```

A manifest failing this check is rejected the same way an existing structurally-invalid manifest is rejected (fail closed, per Constitution Principle III and V) — never loaded and served to a GM with silently-missing attribution.

## UI contract (`SystemLegalNotice` component)

- **Props**: `{ legal: SystemManifestLegal; variant: "selection" | "settings" }`
- **Renders**: license name, attribution text, `requiredNotice` (if present, visually emphasized — this is the Cypher-badge/ORC-Notice case), `disclaimer` (if present), and `trademarkRestrictions` (if non-empty, as a collapsed/expandable list — informational for the platform, not typically GM-facing prose).
- **Both variants render the full content** — `variant` only affects framing/placement chrome (e.g. `"selection"` renders inline in a confirmation step; `"settings"` renders inside `SystemSettingsPanel`), per research.md R3: FR-006's stricter placement is satisfied by *always* showing at both call sites, not by conditionally hiding one.
