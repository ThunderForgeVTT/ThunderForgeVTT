# Example Space Art (FreeOrion, CC-BY-SA 3.0)

Sci-fi fixtures for exercising the engine with something other than
top-down dungeon battlemaps: nebula backdrops as scene backgrounds, and
ship/star sprites as tokens.

## Attribution — required

> Art from the **FreeOrion** project (<https://www.freeorion.org>),
> published on OpenGameArt by **Rainbow Design** as
> ["Free Orion Art"](https://opengameart.org/content/free-orion-art),
> licensed **CC-BY-SA 3.0**
> (<https://creativecommons.org/licenses/by-sa/3.0/>). Unmodified.

Reproduce that notice anywhere these images are displayed or shipped.

The OpenGameArt submission says "the attribution is not mandatory since
the authors did not specify it". That is the submitter's remark, not a
licence term: CC-BY-SA 3.0 requires attribution regardless, and the
submitter cannot waive a condition on someone else's behalf. Attribute.

### Share-alike, and how that sits with AGPL

CC-BY-SA 3.0 is a copyleft licence, so **modified** versions of these
images must themselves be released under CC-BY-SA 3.0. It does not reach
the code: this repository is AGPL-3.0-or-later, and these files are an
aggregate of separately-licensed assets sitting beside it, not a
derivative work of it. Two consequences worth knowing before leaning on
them:

- Re-cropping, recolouring or compositing one of these into new art makes
  that new art CC-BY-SA 3.0 too.
- The transcode pipeline re-encodes stored art (4096 cap, lossy WebP).
  A re-encode is a modification, so a served copy stays CC-BY-SA and
  still needs the notice above.

Unlike `examples/maps` — which is dev/test-only and **not**
redistributable — this directory is safe to publish and ship, provided
the notice travels with it.

## Provenance

Downloaded 2026-08-26 from
<https://opengameart.org/content/free-orion-art>:

| Archive | SHA-256 |
|---|---|
| `space_0.zip` (48.3MB) | `7dbd356c259ece89cae414b6e5484e66b1fb4cf136ee07f34f04990a726ff4a3` |
| `ships.zip` (64.9MB) | `6091f97a7479ef10e2e5bc4e0464c39fc222ceb44ebcf263d1b2d48ab0f7b82f` |

The two archives are ~113MB and 1,270 files between them. What is checked
in here is a hand-picked 17-file subset (~9MB), chosen to cover the two
things the engine actually consumes; the rest is re-fetchable from the
URL above. The unused `bundle.zip`, `ui.zip` and `sound.zip` were not
downloaded — `bundle.zip` is a superset, and the engine has no UI-skin or
audio path to feed.

## Files

`backgrounds/` — 1024x1024 nebulae, for `set_scene_background`. Each has
a transparent background rather than an opaque frame, so they composite
over the engine's clear colour instead of covering it. Unlike the
`.dd2vtt` fixtures these carry no grid, no walls and no lights: they are
art only, which makes them a clean test of the background path on its
own.

- `nebula4.png`, `nebula10.png`, `nebula14.png`, `nebula17.png`,
  `nebula19.png`

`tokens/ships/` — 2048x924 hull renders, side-on, with alpha. Deliberately
**not** square: they are the fixture for token art whose aspect ratio is
not 1:1, which is the case that a footprint-sized square sprite gets
wrong.

- `basic-medium-hull.png`, `organic_hull.png`, `robotic_hull.png`,
  `asteroid_hull.png`, `solar_hull.png`, `titanic_hull.png`

`tokens/stars/` — 128x128 round sprites with alpha, close to the shape a
VTT token normally is. The cheap case, for comparison against the ships.

- `blackhole1.png`, `blue01.png`, `neutron01.png`, `halo_red01.png`,
  `halo_yellow01.png`, `nova-boom3.png`

## What is not here, and why

`space/planets/*.png` look like the obvious token art and are not: they
are 512x256 equirectangular **UV textures**, meant to be wrapped onto a
sphere by a 3D renderer. Dropped onto a flat sprite they read as a
stretched rectangle of terrain, not a planet. Use `tokens/stars/` for
round celestial tokens instead.
