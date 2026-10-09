# Contract: the render probe on Bevy 0.20

`plugins/render_probe.rs`. The `set_render_probe` world command switches
it on, and it then logs one console line every 60 frames. People use it
to answer "is the board drawing, and how much?". The line keeps its
counts and their meanings. Only where each count is read from changes.

| Count                 | Meaning                                       | 0.19.1 source                                                | 0.20 source                                                                                                    |
| --------------------- | --------------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| sprites (main world)  | Entities with `Sprite`                         | `Query<(), With<Sprite>>`                                    | Unchanged                                                                                                      |
| visible sprites       | Sprites that survived CPU culling for the view | `RenderVisibleEntities::get::<Sprite>().entities_cpu_culling` | The visible set for the sprite meshes' class in 0.20 (sprites are Mesh2d). Find the class at T030          |
| extracted sprites     | Sprites copied into the render world           | `ExtractedSprites` length                                    | The render-world count of sprite mesh instances. `ExtractedSprites`, if it remains, counts `Text2d` only and is logged as text |
| phase items           | Items queued in the transparent 2D phase      | `ViewSortedRenderPhases<Transparent2d>` items, after `PhaseSort` | The same phase, or its 0.20 successor, still after the sort set                                          |

**Rules**

- The probe stays off by default and costs nothing when off.
- `RenderProbeEnabled` still reaches the render world. That uses
  `ExtractResource`, with `#[extract_app(RenderApp)]` if 0.20 requires
  it.
- On the fixed baseline scene, the main-world sprite count and the visible
  count are equal on 0.19.1 and 0.20 (SC-006). The phase item count may
  differ only by the text items that 0.20 queues separately. If it
  differs, the difference is recorded in research.md.
- Any e2e that parses the line keeps parsing it. If a count's label
  changes, that is Open item 5 and needs the owner.
