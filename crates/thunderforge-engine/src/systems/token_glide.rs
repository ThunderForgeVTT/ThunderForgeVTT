//! Snapped tokens glide from square to square (spec 099 US1).
//!
//! A token's `Transform` is where the rules put it, and stays there: walls,
//! sight, reach, the server and every test read it. Only the picture glides.
//! [`track_token_glides`] sees each hop before the transform propagates;
//! [`draw_token_glides`] then draws the token, its nameplate, its bars and
//! anything else it carries `offset` away, the arithmetic being
//! `thunderforge_canvas_core::glide`.

use bevy::prelude::*;
use thunderforge_canvas_core::glide::Glide;

use crate::TokenIdentity;
use crate::resources::{DraggingToken, SceneGrid};

/// Whether the viewer prefers reduced motion: every move jumps (FR-004).
/// `set_reduced_motion` writes it.
#[derive(Resource, Default)]
pub struct ReducedMotion(pub bool);

/// A token's glide and the position it was last seen at.
#[derive(Component, Default)]
pub struct TokenGlide {
    pub glide: Glide,
    last: Vec2,
    /// Drawn away from its transform last frame, so propagation, which only
    /// rewrites what changed, will not put it back: this system does.
    drawn_away: bool,
}

impl TokenGlide {
    /// Where the token is drawn, from where it is.
    pub fn offset(&self) -> Vec2 {
        self.glide.offset
    }
}

/// Starts each token's glide when its position hops. A token seen for the
/// first time has no glide: it appears where it is.
pub(crate) fn track_token_glides(
    mut commands: Commands,
    time: Res<Time>,
    grid: Option<Res<SceneGrid>>,
    reduced: Option<Res<ReducedMotion>>,
    dragging: Option<Res<DraggingToken>>,
    mut tokens: Query<(Entity, &TokenIdentity, &Transform, Option<&mut TokenGlide>)>,
) {
    let square = grid.map_or(0.0, |g| g.size);
    let reduced = reduced.is_some_and(|r| r.0);
    let dt = time.delta_secs();
    let carried = |id: &str| {
        dragging
            .as_ref()
            .is_some_and(|d| d.0.iter().any(|t| t.id == id))
    };
    for (entity, identity, transform, glide) in tokens.iter_mut() {
        let at = transform.translation.truncate();
        let Some(mut glide) = glide else {
            commands.entity(entity).insert(TokenGlide {
                last: at,
                ..default()
            });
            continue;
        };
        // A carried token follows the hand square by square: a picture
        // gliding after it would trail the finger and catch up in jerks.
        if carried(&identity.0) {
            glide.glide = Glide::default();
            glide.last = at;
            continue;
        }
        let delta = at - glide.last;
        if delta != Vec2::ZERO {
            glide.glide.hop(delta, square, reduced);
            glide.last = at;
        }
        if glide.glide.is_gliding() {
            glide.glide.step(dt);
        }
    }
}

/// Draws each gliding token, and everything it carries, at its glide.
pub(crate) fn draw_token_glides(
    mut tokens: Query<
        (Entity, &Transform, &mut TokenGlide, &mut GlobalTransform),
        (With<TokenIdentity>, Without<ChildOf>),
    >,
    mut carried: Query<(&Transform, &mut GlobalTransform), Without<TokenIdentity>>,
    children: Query<&Children>,
) {
    for (entity, transform, mut glide, mut global) in tokens.iter_mut() {
        let gliding = glide.glide.is_gliding();
        if !gliding && !glide.drawn_away {
            continue;
        }
        glide.drawn_away = gliding;
        let mut drawn = *transform;
        drawn.translation += glide.offset().extend(0.0);
        // From the transforms, not from last frame's picture, so nothing
        // drifts; the frame a glide ends draws the token exactly where it is.
        *global = GlobalTransform::from(drawn);
        let parent = *global;
        draw_carried(entity, parent, &children, &mut carried);
    }
}

/// Every frame's gliding tokens, for [`token_glides`].
static GLIDES: std::sync::Mutex<Vec<serde_json::Value>> = std::sync::Mutex::new(Vec::new());

/// Mirrors, every frame, where each gliding token stands and where it is
/// drawn. Only gliding tokens, so a still board costs nothing.
pub(crate) fn mirror_token_glides(tokens: Query<(&TokenIdentity, &Transform, &TokenGlide)>) {
    let list: Vec<_> = tokens
        .iter()
        .filter(|(_, _, glide)| glide.glide.is_gliding())
        .map(|(identity, transform, glide)| {
            let at = transform.translation.truncate();
            let drawn = at + glide.offset();
            serde_json::json!({
                "tokenId": identity.0,
                "x": at.x,
                "y": at.y,
                "drawnX": drawn.x,
                "drawnY": drawn.y,
            })
        })
        .collect();
    if let Ok(mut slot) = GLIDES.lock() {
        *slot = list;
    }
}

/// The tokens gliding this frame: `[{"tokenId","x","y","drawnX","drawnY"}]`
/// in world units; a token not listed is drawn where it stands. Read-only,
/// for tests (spec 099 SC-005).
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn token_glides() -> String {
    let list = GLIDES.lock().map(|l| l.clone()).unwrap_or_default();
    serde_json::Value::from(list).to_string()
}

fn draw_carried(
    entity: Entity,
    parent: GlobalTransform,
    children: &Query<&Children>,
    carried: &mut Query<(&Transform, &mut GlobalTransform), Without<TokenIdentity>>,
) {
    let Ok(list) = children.get(entity) else {
        return;
    };
    for child in list.iter() {
        let Ok((local, mut global)) = carried.get_mut(child) else {
            continue;
        };
        let drawn = parent.mul_transform(*local);
        *global = drawn;
        draw_carried(child, drawn, children, carried);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::DraggedToken;
    use bevy::transform::TransformSystems;
    use thunderforge_canvas_core::grid::GridSpec;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        app.insert_resource(SceneGrid(GridSpec {
            size: 100.0,
            ..default()
        }));
        app.init_resource::<ReducedMotion>();
        app.add_systems(
            PostUpdate,
            (
                track_token_glides.before(TransformSystems::Propagate),
                draw_token_glides.after(TransformSystems::Propagate),
            ),
        );
        app
    }

    #[test]
    fn a_hop_draws_the_token_and_its_nameplate_behind_its_position() {
        let mut app = app();
        let token = app
            .world_mut()
            .spawn((TokenIdentity("hero".into()), Transform::default()))
            .with_children(|p| {
                p.spawn(Transform::from_xyz(0.0, -60.0, 1.0));
            })
            .id();
        app.update();
        app.update();
        app.world_mut()
            .get_mut::<Transform>(token)
            .unwrap()
            .translation
            .x = 100.0;
        app.update();

        let world = app.world();
        let drawn = world.get::<GlobalTransform>(token).unwrap().translation();
        assert!(drawn.x < 100.0, "{drawn}");
        assert_eq!(world.get::<Transform>(token).unwrap().translation.x, 100.0);
        let plate = world.get::<Children>(token).unwrap()[0];
        let plate = world.get::<GlobalTransform>(plate).unwrap().translation();
        assert_eq!((plate.x, plate.y), (drawn.x, -60.0));
    }

    #[test]
    fn reduced_motion_draws_the_token_where_it_is() {
        let mut app = app();
        app.insert_resource(ReducedMotion(true));
        let token = app
            .world_mut()
            .spawn((TokenIdentity("hero".into()), Transform::default()))
            .id();
        app.update();
        app.world_mut()
            .get_mut::<Transform>(token)
            .unwrap()
            .translation
            .x = 100.0;
        app.update();
        let drawn = app
            .world()
            .get::<GlobalTransform>(token)
            .unwrap()
            .translation();
        assert_eq!(drawn.x, 100.0);
    }

    #[test]
    fn a_carried_token_is_drawn_under_the_hand() {
        let mut app = app();
        app.init_resource::<DraggingToken>();
        let token = app
            .world_mut()
            .spawn((TokenIdentity("hero".into()), Transform::default()))
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<DraggingToken>()
            .0
            .push(DraggedToken {
                id: "hero".into(),
                offset: Vec2::ZERO,
                origin: Vec2::ZERO,
            });
        app.world_mut()
            .get_mut::<Transform>(token)
            .unwrap()
            .translation
            .x = 100.0;
        app.update();
        let drawn = app
            .world()
            .get::<GlobalTransform>(token)
            .unwrap()
            .translation();
        assert_eq!(drawn.x, 100.0);
    }
}
