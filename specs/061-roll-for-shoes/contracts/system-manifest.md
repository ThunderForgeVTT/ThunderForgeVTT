# Contract: `packs/systems/roll_for_shoes/system.json`

What the pack declares, and why each block is the size it is. The authority
for every key here is `packs/systems/README.md`; this file records the choices,
not the format.

```json
{
  "id": "roll_for_shoes",
  "title": "Roll for Shoes",
  "version": "0.1.0",
  "description": "A whole roleplaying game in six rules. Say what you do, roll a die for each level of a skill you have, and beat the opposition. Fail and you learn something; roll all sixes and you invent a skill nobody has ever had.",
  "author": "Ben Wray",
  "url": "https://rollforshoes.com/",
  "license": "CC0-1.0",

  "compatibility": { "minimum": "…", "verified": "…", "maximum": null },

  "legal": {
    "licenseName": "CC0 1.0 Universal (Public Domain Dedication)",
    "attributionText": "Roll for Shoes by Ben Wray — rollforshoes.com",
    "requiredNotice": null,
    "disclaimer": null,
    "trademarkRestrictions": [],
    "requiredUiPlacement": null,
    "sourceUrl": "https://rollforshoes.com/"
  },

  "packages": { "server": "./server", "web": "./web" },
  "esmodules": ["web/dist/index.js"],
  "styles": ["web/dist/index.css"],

  "data_types": {
    "trait_data": {
      "description": "A Roll for Shoes character's description and skill lineage",
      "properties": {
        "description": { "type": "string", "label": "Description" },
        "skills": { "type": "array", "label": "Skills" }
      },
      "required": []
    },
    "resource_data": {
      "description": "Experience earned from failure",
      "properties": { "xp": { "type": "integer", "label": "XP" } },
      "required": []
    }
  },

  "resources": [
    {
      "id": "xp", "label": "XP", "kind": "counter", "order": 0,
      "allowStacking": false,
      "source": { "slot": "resourceData", "entries": [{ "current": "xp" }] }
    }
  ],

  "sheet": [
    { "id": "description", "label": "Description", "kind": "text",
      "slot": "traitData", "source": "description" }
  ],

  "turnStructure": { "rounds": false },

  "startingSkill": { "name": "Do Anything", "level": 1 }
}
```

## The decisions in it

**`legal` is required and enforced**, and a manifest without compliant legal
metadata is refused rather than served (`packs/systems/README.md:62-77`). So
Roll for Shoes must state an attribution even though CC0 obliges none. The
text credits Ben Wray and the site and **asserts nothing else** — no year, no
forum handle, no website author — because the source states none of those, and
a licence block must not claim what it cannot support (FR-003b).

**No `abilities`.** The game has no attribute array. Fate Core already
declares none and its sheet is correct for it.

**No `movement`, no `vision`, no `combat`, no `appearance`.** There is no
speed, no sight rule, no hit points, no defence, no size and no damage in this
game. Each absence is the ruleset's answer, not an unfinished block.

**`turnStructure.rounds: false`.** There is no initiative and no action
economy, so no round counter is shown.

**No `checks`.** Seven of the eight bundled packs ship this way, and this one
must: a `check` binds to a top-level scalar under a manifest-declared id, and a
Roll for Shoes skill has neither. See research D3.

**`startingSkill` is a key this contract does not describe**, which a manifest
is allowed to carry (`packs/systems/README.md:369-377`). It is read by the
pack's own server crate and by nothing else. The pack's web module states the
same two values as a constant, because the host surface gives a sheet no way
to read a manifest — research D6 records why that duplication is accepted.

**`engine` is absent from `packages`.** Roll for Shoes has no geometry.
