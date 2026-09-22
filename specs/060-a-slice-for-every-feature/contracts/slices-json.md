# Contract: `scripts/e2e/slices.json` and `slice-durations.json`

The entities and their validation rules are in [../data-model.md](../data-model.md).
This file fixes the on-disk shape.

## `scripts/e2e/slices.json` (hand-edited)

```json
{
  "$comment": "One owner per spec; see docs/CONTRIBUTING.md 'Proving a change'. Checked by scripts/check-e2e-slices.mjs.",
  "crossCutting": [
    { "glob": "src/app/schema.graphql", "why": "every client speaks this schema" }
  ],
  "slices": [
    {
      "name": "hero-builder",
      "summary": "Hero builder (specs/044-hero-builder)",
      "own": ["hero-builder-"],
      "neighbours": [
        { "spec": "actor-art.spec.ts", "seam": "writes actor imagery that the art panel shows" }
      ],
      "paths": ["packages/heroes/**", "packages/hero-builder/**", "apps/hero-builder/**",
                "apps/web/src/pages/world/actor/HeroBuilderDialog.tsx", "apps/web/src/pages/world/actor/heroBuilderLazy.ts"],
      "standalone": "pnpm -F @thunderforge/hero-builder-app test:e2e"
    }
  ]
}
```

- Slices are ordered alphabetically by `name`. The check reports a slice
  that is out of order, so diffs stay stable.
- Unknown keys are an error. This catches `neighbors` and other misspellings.

## `scripts/e2e/slice-durations.json` (written by the runner)

```json
{
  "hero-builder": {
    "wallSeconds": 214,
    "specs": 4,
    "passed": 16, "failed": 0, "flaky": 0, "skipped": 0,
    "measuredAt": "2026-09-22",
    "commit": "7928092"
  }
}
```

- Keys are sorted, and the file is written with two-space indentation and a
  trailing newline.
- Only `--slice=<name> --record-durations` writes it. A plain run never
  touches it.
