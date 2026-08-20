use bevy::prelude::*;

/// The canvas's explicit, ordered render layers (FR-016). Every capability
/// added by the native-canvas-authoring feature (walls, lighting, shapes,
/// map import's background art) renders into one of these instead of
/// picking its own ad hoc z-order — this generalizes the layer stack
/// sketched in ADR-032 into a first-class resource other plugins consume.
///
/// Z-order is fixed for v1 (not user-reorderable): lower values render
/// first (further back).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanvasLayer {
    Background,
    Grid,
    Walls,
    Lighting,
    Shapes,
    Tokens,
    Fog,
}

impl CanvasLayer {
    /// Fixed render order, lowest first. Kept as a method (rather than a
    /// derived enum discriminant) so the order is one readable place, not
    /// implied by declaration order alone.
    pub const ORDER: [CanvasLayer; 7] = [
        CanvasLayer::Background,
        CanvasLayer::Grid,
        CanvasLayer::Walls,
        CanvasLayer::Lighting,
        CanvasLayer::Shapes,
        CanvasLayer::Tokens,
        CanvasLayer::Fog,
    ];

    /// A z-translation for sprites/meshes on this layer. Spaced widely so
    /// each layer has room for its own internal ordering (e.g. selected
    /// vs. unselected tokens) without colliding with the next layer.
    pub fn z(self) -> f32 {
        Self::ORDER
            .iter()
            .position(|layer| *layer == self)
            .map(|index| index as f32 * 10.0)
            .unwrap_or(0.0)
    }

    /// Whether this layer's *editing* affordances (handles, tool cursors)
    /// are GM-only. This is about authoring UI, not the rendered effect —
    /// e.g. Lighting's illumination is visible to players, but a light's
    /// drag handle is not (data-model.md's Canvas Layer section).
    pub fn editing_is_gm_only(self) -> bool {
        !matches!(self, CanvasLayer::Tokens)
    }
}

/// Per-layer visibility toggles (e.g. a GM hiding the grid overlay).
/// Defaults to every layer visible.
#[derive(Resource, Debug, Clone)]
pub struct CanvasLayers {
    background: bool,
    grid: bool,
    walls: bool,
    lighting: bool,
    shapes: bool,
    tokens: bool,
    fog: bool,
}

impl Default for CanvasLayers {
    fn default() -> Self {
        Self {
            background: true,
            grid: true,
            walls: true,
            lighting: true,
            shapes: true,
            tokens: true,
            fog: true,
        }
    }
}

impl CanvasLayers {
    pub fn is_visible(&self, layer: CanvasLayer) -> bool {
        match layer {
            CanvasLayer::Background => self.background,
            CanvasLayer::Grid => self.grid,
            CanvasLayer::Walls => self.walls,
            CanvasLayer::Lighting => self.lighting,
            CanvasLayer::Shapes => self.shapes,
            CanvasLayer::Tokens => self.tokens,
            CanvasLayer::Fog => self.fog,
        }
    }

    pub fn set_visible(&mut self, layer: CanvasLayer, visible: bool) {
        let field = match layer {
            CanvasLayer::Background => &mut self.background,
            CanvasLayer::Grid => &mut self.grid,
            CanvasLayer::Walls => &mut self.walls,
            CanvasLayer::Lighting => &mut self.lighting,
            CanvasLayer::Shapes => &mut self.shapes,
            CanvasLayer::Tokens => &mut self.tokens,
            CanvasLayer::Fog => &mut self.fog,
        };
        *field = visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_order_is_stable_and_increasing() {
        let mut previous = -1.0;
        for layer in CanvasLayer::ORDER {
            let z = layer.z();
            assert!(z > previous, "layer z-order must strictly increase");
            previous = z;
        }
    }

    #[test]
    fn all_layers_visible_by_default() {
        let layers = CanvasLayers::default();
        for layer in CanvasLayer::ORDER {
            assert!(layers.is_visible(layer));
        }
    }

    #[test]
    fn set_visible_only_affects_the_targeted_layer() {
        let mut layers = CanvasLayers::default();
        layers.set_visible(CanvasLayer::Grid, false);

        assert!(!layers.is_visible(CanvasLayer::Grid));
        assert!(layers.is_visible(CanvasLayer::Walls));
        assert!(layers.is_visible(CanvasLayer::Tokens));
    }

    #[test]
    fn tokens_editing_is_not_gm_only_others_are() {
        assert!(!CanvasLayer::Tokens.editing_is_gm_only());
        assert!(CanvasLayer::Walls.editing_is_gm_only());
        assert!(CanvasLayer::Lighting.editing_is_gm_only());
        assert!(CanvasLayer::Shapes.editing_is_gm_only());
    }
}
