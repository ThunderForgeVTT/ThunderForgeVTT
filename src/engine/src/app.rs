//! The systems that run inside the app each frame.

use super::*;

pub(crate) fn setup_scene(mut commands: Commands, mut token_entities: ResMut<TokenEntities>) {
    // NO camera is spawned here. `CameraPlugin` (plugins/camera.rs) owns the
    // one and only camera — it is the one `CameraManager` drives for pan and
    // zoom. This function used to spawn a second `Camera2d` as well, leaving
    // two active cameras with the same order (0) on the same render target.
    // Bevy warned about that every frame ("Camera order ambiguities
    // detected ..."), and the consequence is not cosmetic: each camera
    // clears the target on its own pass, so with an undefined order between
    // them one pass can wipe the other's output.
    //
    // The warning was invisible until `bevy_log` was added to this crate's
    // features; see the note there. Removing the duplicate silences it and
    // leaves exactly one active camera.
    //
    // This was a real bug but not the cause of the "canvas renders nothing
    // but the clear colour" symptom — that was the missing `*_render`
    // features in Cargo.toml. Both are fixed; they were independent.
    let player_entity = commands
        .spawn((
            Sprite::from_color(Color::srgb(0.851, 0.278, 0.306), TOKEN_SIZE),
            Transform::from_xyz(-180.0, 0.0, 0.0),
            PlayerToken,
            TokenIdentity("player".to_string()),
            PlayerControlled,
        ))
        .id();

    token_entities.0.insert("player".to_string(), player_entity);

    let npc_entity = commands
        .spawn((
            Sprite::from_color(Color::srgb(0.282, 0.565, 0.996), TOKEN_SIZE),
            Transform::from_xyz(180.0, 0.0, 0.0),
            TokenIdentity("npc".to_string()),
        ))
        .id();

    token_entities.0.insert("npc".to_string(), npc_entity);
}

pub(crate) fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player: Single<&mut Transform, With<PlayerToken>>,
    time: Res<Time>,
) {
    let mut direction = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }

    if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }

    if direction == Vec2::ZERO {
        return;
    }

    let delta = direction.normalize() * PLAYER_SPEED * time.delta_secs();
    let half_bounds = Vec2::new(ARENA_WIDTH / 2.0, ARENA_HEIGHT / 2.0) - (TOKEN_SIZE / 2.0);
    let translation = &mut player.translation;

    translation.x = (translation.x + delta.x).clamp(-half_bounds.x, half_bounds.x);
    translation.y = (translation.y + delta.y).clamp(-half_bounds.y, half_bounds.y);
}

pub(crate) fn emit_player_state(
    mut last_sent: ResMut<LastPlayerSent>,
    active_world: Res<ActiveWorld>,
    player: Single<(&Transform, &TokenIdentity), With<PlayerToken>>,
) {
    let (transform, token_identity) = *player;
    let current = transform.translation.truncate();

    if current.distance(last_sent.0) < 0.5 {
        return;
    }

    last_sent.0 = current;

    emit_event(json!({
        "type": "upsert_token",
        "token": {
            "id": token_identity.0,
            "x": transform.translation.x,
            "y": transform.translation.y,
            "z": transform.translation.z,
            "label": "Player"
        },
        "worldId": active_world.0,
    }));
}

/// Scene-level resources the command loop writes to.
///
/// Grouped into one `SystemParam` because Bevy caps a system at 16 parameters
/// and this loop had reached it. Every field is `Option` for the same reason
/// the loose parameters were: each belongs to a plugin that may not be
/// registered, and a command for an absent plugin is dropped rather than
/// panicking (Constitution Principle II).
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct SceneParams<'w, 's> {
    grid: Option<ResMut<'w, SceneGrid>>,
    grid_visible: Option<ResMut<'w, GridVisible>>,
    ambient: Option<ResMut<'w, SceneAmbient>>,
    lighting_overlay: Option<ResMut<'w, LightingOverlay>>,
    camera: Option<ResMut<'w, CameraManager>>,
    grid_snap: Option<ResMut<'w, GridSnapEnabled>>,
    units: Option<ResMut<'w, crate::systems::token_move::SceneUnits>>,
    camera_viewport: Query<'w, 's, &'static Camera, With<Camera2d>>,
}

/// The interaction plugin's resources, grouped.
///
/// Grouped for the same reason `SceneParams` is: Bevy caps a system at 16
/// parameters and this loop is at the limit. Every field is `Option` because
/// `InteractionPlugin` is independently addable, and a command for an absent
/// plugin is dropped rather than panicking (Constitution Principle II).
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct InteractionParams<'w> {
    interactives: Option<ResMut<'w, plugins::Interactives>>,
    pending_activations: Option<ResMut<'w, plugins::interaction::PendingActivations>>,
    scene_playing: Option<ResMut<'w, plugins::interaction::ScenePlaying>>,
}

// One parameter per resource the command dispatch can touch, which is what a
// Bevy system looks like when it is the single entry point for every external
// command. clippy.toml already raises the threshold to 10 for ordinary
// systems; this one is past it by being the seam itself, and the related
// parameters are already bundled into `SystemParam` structs like
// `InteractionParams`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_external_commands(
    mut commands: Commands,
    mut active_world: ResMut<ActiveWorld>,
    mut token_entities: ResMut<TokenEntities>,
    mut token_query: Query<(Entity, &mut Transform, &TokenIdentity, &mut Sprite)>,
    // `WallSet` only exists once `WallPlugin` is registered (Constitution
    // Principle II: plugins are independently addable) — `Option` so this
    // core command loop degrades gracefully (wall commands are simply
    // dropped) if the wall plugin isn't present.
    wall_set: Option<ResMut<WallSet>>,
    // Same rationale as `wall_set`, for `LightingPlugin`/`LightSet`.
    light_set: Option<ResMut<LightSet>>,
    // `ShapeSet` only exists once `ShapePlugin` is registered, same
    // graceful-degradation rationale as `wall_set` above.
    shape_set: Option<ResMut<ShapeSet>>,
    // `SceneBackground` only exists once `BackgroundPlugin` is registered,
    // same graceful-degradation rationale as `wall_set` above.
    background: Option<ResMut<SceneBackground>>,
    // `PlacedCanvasImages` only exists once `BackgroundPlugin` is
    // registered (spec 002 added it alongside `SceneBackground` in that
    // same plugin), same graceful-degradation rationale as `wall_set`.
    placed_canvas_images: Option<ResMut<PlacedCanvasImages>>,
    // `IsGameMaster` exists once either `WallPlugin` or `ShapePlugin` is
    // registered (both `init_resource` it idempotently) — same
    // graceful-degradation rationale as `wall_set` above.
    is_game_master: Option<ResMut<IsGameMaster>>,
    // `RenderProbeEnabled` only exists once `RenderProbePlugin` is
    // registered, same graceful-degradation rationale as `wall_set` above.
    mut render_probe: Option<ResMut<RenderProbeEnabled>>,
    mut scene: SceneParams,
    // `PendingDiceRoll` only exists once `DiceRollPlugin` is registered,
    // same graceful-degradation rationale as `wall_set` above.
    pending_dice_roll: Option<ResMut<plugins::dice_roll::PendingDiceRoll>>,
    // For token art (`upsert_token`'s optional `image`). Not `Option`: the
    // asset server is part of `DefaultPlugins`, not a plugin this crate can
    // choose to leave out.
    asset_server: Res<AssetServer>,
    // `Appearance` only exists once `StatusDisplayPlugin` is registered, same
    // graceful-degradation rationale as `wall_set` above. An appearance
    // command with no status plugin to apply it to is a no-op rather than a
    // fault: nothing is being displayed for it to affect.
    appearance: Option<ResMut<plugins::status_display::Appearance>>,
    mut interaction: InteractionParams,
) {
    let drained = if let Ok(mut queue) = external_command_queue().lock() {
        queue.drain(..).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut wall_set = wall_set;
    let mut appearance = appearance;
    let mut light_set = light_set;
    let mut shape_set = shape_set;
    let mut background = background;
    let mut placed_canvas_images = placed_canvas_images;
    let mut is_game_master = is_game_master;
    let mut pending_dice_roll = pending_dice_roll;
    let InteractionParams {
        interactives,
        pending_activations,
        scene_playing,
    } = &mut interaction;

    for command in drained {
        match command {
            ExternalCommand::SetWorld { world_id } => {
                active_world.0 = world_id;
            }
            ExternalCommand::UpsertToken { token } => {
                if let Some(existing_entity) = token_entities.0.get(&token.id).copied() {
                    if let Ok((_, mut transform, _, mut sprite)) =
                        token_query.get_mut(existing_entity)
                    {
                        transform.translation.x = token.x;
                        transform.translation.y = token.y;
                        transform.translation.z = token.z;
                        // Spec 004 (US2): apply scale/rotation only when
                        // present — `None` leaves the entity's current
                        // Transform.scale/rotation untouched, matching the
                        // "don't touch what wasn't sent" partial-update
                        // semantics `WorldTokenPayload`'s doc comment
                        // describes.
                        if let Some(scale) = token.scale {
                            transform.scale = Vec3::splat(scale);
                        }
                        if let Some(rotation) = token.rotation {
                            transform.rotation = Quat::from_rotation_z(rotation);
                        }

                        // Same partial-update rule for the art: `None`
                        // leaves whatever the token already shows. Guarded
                        // on the handle rather than assigned outright,
                        // because `Sprite` is change-detected and assigning
                        // an identical handle would re-extract the token to
                        // the render world for nothing. Clearing the size
                        // hands it back to `size_tokens_to_grid`, which
                        // re-fits it once the new art's dimensions are
                        // known — the old art's aspect must not stick.
                        if let Some(path) = token.photo_url.clone() {
                            let handle = asset_server.load(path);
                            if sprite.image != handle {
                                sprite.image = handle;
                                sprite.custom_size = None;
                            }
                        }
                    }
                    continue;
                }

                let mut transform = Transform::from_xyz(token.x, token.y, token.z);
                if let Some(scale) = token.scale {
                    transform.scale = Vec3::splat(scale);
                }
                if let Some(rotation) = token.rotation {
                    transform.rotation = Quat::from_rotation_z(rotation);
                }

                let sprite = match token.photo_url.clone() {
                    Some(path) => Sprite {
                        // Owned: `AssetServer::load` borrows for `'static`,
                        // and `token` is dropped at the end of this arm.
                        image: asset_server.load(path),
                        // Left for `size_tokens_to_grid` to set once the
                        // image's real dimensions are known. Guessing here
                        // would just be overwritten a frame later.
                        custom_size: None,
                        ..default()
                    },
                    None => Sprite::from_color(token_kind_color(&token.token_type), TOKEN_SIZE),
                };

                // The `Token` and `DerivedStats` components go on here, and
                // this is the first time in the project's history that they
                // have.
                //
                // `calculate_derived_stats` queries `(&Token, &mut
                // DerivedStats)` and has been registered in the frame loop the
                // whole time, matching nothing — no spawned entity carried
                // `Token`, and the only construction of that type anywhere was
                // a unit test. It recomputed nothing, every frame, for nobody.
                //
                // Spec 029 is the first consumer of what it computes, so
                // attaching the components and drawing the result are one
                // piece of work: doing either alone leaves the dead end where
                // it is.
                let kind = token
                    .token_type
                    .as_deref()
                    .and_then(TokenKind::from_stored)
                    .unwrap_or_default();
                let (r, g, b) = kind.fill();

                let entity = commands
                    .spawn((
                        sprite,
                        transform,
                        TokenIdentity(token.id.clone()),
                        Token {
                            id: token.id.clone(),
                            world_id: String::new(),
                            scene_id: String::new(),
                            token_type: kind.as_stored().to_string(),
                            label: token.label.clone(),
                            base_x: token.x as i32,
                            base_y: token.y as i32,
                            size_x: 1,
                            size_y: 1,
                            color: Color::srgb(r, g, b),
                            is_visible: true,
                            health: token.health,
                            max_health: token.max_health,
                            // Populated from the payload where the server
                            // sent them, empty where it did not. Empty means
                            // "this sheet is not filled in", which is a
                            // different claim from a sheet of zeroes — and in
                            // every system shipping here a zero is a real and
                            // punishing score.
                            attributes: TokenAttributes(
                                token
                                    .attributes
                                    .clone()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .collect(),
                            ),
                            schema_version: 1,
                            is_selected: false,
                            is_hovered: false,
                        },
                        DerivedStats::default(),
                    ))
                    .id();

                // A token arriving after its status adopts it here, which is
                // the other half of the ordering fix above.
                if let Ok(slot) = token_status_slot().lock()
                    && let Some(resources) = slot.get(&token.id)
                {
                    commands.entity(entity).insert(TokenStatus {
                        resources: resources
                            .iter()
                            .map(|r| ResolvedResource {
                                definition: r.definition.clone(),
                                disclosed: r.disclosed.clone(),
                            })
                            .collect(),
                    });
                }

                token_entities.0.insert(token.id, entity);
            }
            ExternalCommand::RemoveToken { token_id } => {
                if let Some(entity) = token_entities.0.remove(&token_id) {
                    commands.entity(entity).despawn();
                }
            }
            ExternalCommand::SetTokenStatus {
                token_id,
                resources,
            } => {
                // Setting the component is the whole application step; the
                // plugin's `Changed<TokenStatus>` system redraws from there.
                // Recorded first, and unconditionally.
                //
                // Status routinely arrives before the token it describes: the
                // client fetches it as the scene opens while tokens are still
                // being loaded. Dropping it when the entity is missing made
                // bars appear or not depending on which request won, which is
                // the kind of bug that reproduces once in ten runs and gets
                // called flaky. The slot is the record; the component is a
                // projection of it, applied when there is something to apply
                // it to (see `apply_pending_token_status`).
                if let Ok(mut slot) = token_status_slot().lock() {
                    slot.insert(token_id.clone(), resources.clone());
                }

                if let Some(&entity) = token_entities.0.get(&token_id) {
                    commands.entity(entity).insert(TokenStatus {
                        resources: resources
                            .into_iter()
                            .map(|r| ResolvedResource {
                                definition: r.definition,
                                disclosed: r.disclosed,
                            })
                            .collect(),
                    });
                }
            }
            ExternalCommand::ClearTokenStatus { token_id } => {
                // An empty set rather than removing the component: the
                // plugin's change detection is what clears the drawn geometry,
                // and removing the component would leave the last bars on
                // screen with nothing to trigger their removal.
                if let Some(&entity) = token_entities.0.get(&token_id) {
                    if let Ok(mut slot) = token_status_slot().lock() {
                        slot.remove(&token_id);
                    }
                    commands.entity(entity).insert(TokenStatus::default());
                }
            }
            ExternalCommand::SetDisplayAppearance { override_values } => {
                // Folded onto whatever is current, not onto the defaults —
                // so two overrides in a row accumulate rather than the second
                // silently discarding the first.
                if let Some(appearance) = appearance.as_deref_mut() {
                    let mut next = appearance.0.clone();
                    override_values.apply_to(&mut next);
                    appearance.0 = next;
                }
            }
            ExternalCommand::UpsertWall { wall } => {
                if let Some(wall_set) = wall_set.as_deref_mut() {
                    wall_set.upsert(EngineWall {
                        id: wall.id,
                        x1: wall.x1,
                        y1: wall.y1,
                        x2: wall.x2,
                        y2: wall.y2,
                        blocks_vision: wall.blocks_vision,
                        blocks_movement: wall.blocks_movement,
                        door_state: DoorState::from_str_loose(&wall.door_state),
                        locked: wall.locked,
                        secret: wall.secret,
                    });
                }
            }
            ExternalCommand::RemoveWall { wall_id } => {
                if let Some(wall_set) = wall_set.as_deref_mut() {
                    wall_set.remove(&wall_id);
                }
            }
            ExternalCommand::UpsertInteractive { interactive } => {
                if let Some(interactives) = interactives.as_deref_mut() {
                    interactives.upsert(to_engine_interactive(interactive));
                }
            }
            ExternalCommand::RemoveInteractive { interactive_id } => {
                if let Some(interactives) = interactives.as_deref_mut() {
                    interactives.remove(&interactive_id);
                }
            }
            ExternalCommand::DispatchInteraction {
                interactive_id,
                effect_id,
                config,
            } => {
                // Queued rather than written directly: a message can only be
                // written from a system, and this loop is one — but the
                // interaction plugin owns the writing, so that dispatch has
                // exactly one path whether it came from a click or from a
                // region being crossed.
                if let Some(pending) = pending_activations.as_deref_mut() {
                    let subject_ref = interactives
                        .as_deref()
                        .and_then(|set| set.get(&interactive_id))
                        .and_then(|i| i.subject_ref.clone());
                    pending.0.push(plugins::InteractionActivated {
                        interactive_id,
                        effect_id,
                        config,
                        subject_ref,
                    });
                }
            }
            ExternalCommand::SetScenePlaying { playing } => {
                if let Some(scene_playing) = scene_playing.as_deref_mut() {
                    scene_playing.0 = playing;
                }
            }
            ExternalCommand::UpsertLight { light } => {
                if let Some(light_set) = light_set.as_deref_mut() {
                    light_set.upsert(EngineLight {
                        id: light.id,
                        x: light.x,
                        y: light.y,
                        radius: light.radius,
                        intensity: light.intensity,
                        color: light.color,
                        attached_token_id: light.attached_token_id,
                        casts_shadows: light.casts_shadows,
                    });
                }
            }
            ExternalCommand::RemoveLight { light_id } => {
                if let Some(light_set) = light_set.as_deref_mut() {
                    light_set.remove(&light_id);
                }
            }
            ExternalCommand::UpsertShape { shape } => {
                if let Some(shape_set) = shape_set.as_deref_mut() {
                    shape_set.upsert(EngineShape {
                        id: shape.id,
                        kind: ShapeKind::from_str_loose(&shape.kind),
                        geometry: shape.geometry,
                        text: shape.text,
                        style: shape.style,
                        visible_to_players: shape.visible_to_players,
                    });
                }
            }
            ExternalCommand::RemoveShape { shape_id } => {
                if let Some(shape_set) = shape_set.as_deref_mut() {
                    shape_set.remove(&shape_id);
                }
            }
            ExternalCommand::SetSceneBackground {
                path,
                width,
                height,
            } => {
                if let Some(background) = background.as_deref_mut() {
                    // Bug fix: writing through `ResMut::deref_mut` trips
                    // Bevy's change detection unconditionally, even when
                    // `path`/`width`/`height` are identical to the current
                    // value — every repeat dispatch of an already-applied
                    // background (WorldPage.tsx's effect can legitimately
                    // re-run with an equivalent `selectedScene` object,
                    // e.g. after an unrelated scene-list refetch) then made
                    // `sync_scene_background` (systems/background.rs) see
                    // `is_changed() == true` again, despawning the sprite
                    // and re-issuing `asset_server.load(&path)` — dropping
                    // the previous `Handle<Image>` cancels that in-flight
                    // load (found live: a real imported background's fetch
                    // reliably got `net::ERR_ABORTED` moments after
                    // starting, leaving Play's canvas permanently blank).
                    // Comparing first keeps the write — and the spurious
                    // respawn/reload cycle — from happening at all when
                    // nothing actually changed.
                    let unchanged = background.path == path
                        && background.width == width
                        && background.height == height;
                    if !unchanged {
                        background.path = path;
                        background.width = width;
                        background.height = height;
                    }
                }
            }
            ExternalCommand::SetIsGameMaster {
                is_game_master: value,
            } => {
                if let Some(is_game_master) = is_game_master.as_deref_mut() {
                    is_game_master.0 = value;
                }
            }
            ExternalCommand::SetSceneGrid {
                grid_type,
                size,
                map_size,
                origin_x,
                origin_y,
                visible,
            } => {
                if let Some(scene_grid) = scene.grid.as_deref_mut() {
                    *scene_grid = match map_size {
                        Some(map_size) => SceneGrid::anchored_to_map(&grid_type, size, map_size),
                        None => {
                            SceneGrid::from_server(&grid_type, size, Vec2::new(origin_x, origin_y))
                        }
                    };
                    info!(
                        target: "grid",
                        "grid: {:?} size={} origin={:?} visible={visible}",
                        scene_grid.kind,
                        scene_grid.size,
                        scene_grid.origin,
                    );
                }
                if let Some(grid_visible) = scene.grid_visible.as_deref_mut() {
                    grid_visible.0 = visible;
                }
            }
            ExternalCommand::SetTokenGrid {
                token_id,
                footprint,
                snap,
            } => {
                if let Some(&entity) = token_entities.0.get(&token_id) {
                    let behaviour = TokenGridBehaviour {
                        footprint: Footprint::new(footprint),
                        snap,
                    };
                    commands.entity(entity).insert(behaviour);
                    info!(
                        target: "grid",
                        "token {token_id}: {} cells, snap={snap}",
                        behaviour.footprint.cells(),
                    );
                } else {
                    warn!(target: "grid", "set_token_grid: no token {token_id}");
                }
            }
            ExternalCommand::SetGridUnits { per_cell, label } => {
                if let Some(units) = scene.units.as_deref_mut() {
                    units.0 = GridUnits::new(per_cell, label);
                    info!(target: "grid", "units: 1 cell = {}", units.format(1.0));
                }
            }
            ExternalCommand::SetGridSnap { enabled } => {
                if let Some(snap) = scene.grid_snap.as_deref_mut() {
                    snap.0 = enabled;
                    info!(target: "grid", "grid snapping {}", if enabled { "on" } else { "off" });
                }
            }
            ExternalCommand::SetCamera { x, y, zoom } => {
                if let Some(camera_mgr) = scene.camera.as_deref_mut() {
                    if let Some(x) = x {
                        camera_mgr.translation.x = x;
                    }
                    if let Some(y) = y {
                        camera_mgr.translation.y = y;
                    }
                    if let Some(zoom) = zoom {
                        camera_mgr.set_zoom(zoom);
                    }
                }
            }
            ExternalCommand::FitCameraTo {
                center_x,
                center_y,
                width,
                height,
            } => {
                if let Some(camera_mgr) = scene.camera.as_deref_mut() {
                    // The viewport in *world units at 1:1* is just its pixel
                    // size, since one world unit is one pixel at scale 1.
                    let viewport = scene
                        .camera_viewport
                        .single()
                        .ok()
                        .and_then(|camera| camera.logical_viewport_size())
                        .unwrap_or(Vec2::new(1280.0, 720.0));
                    camera_mgr.fit_to(
                        Vec2::new(center_x, center_y),
                        Vec2::new(width, height),
                        viewport,
                    );
                    info!(
                        target: "camera",
                        "fit {width}x{height} into {viewport:?} -> zoom {}",
                        camera_mgr.scale,
                    );
                }
            }
            ExternalCommand::SetTokenVision {
                token_id,
                darkvision,
                facing,
                fov,
                max_range,
            } => {
                if let Some(&entity) = token_entities.0.get(&token_id) {
                    commands.entity(entity).insert(TokenVision(VisionProfile {
                        darkvision,
                        facing,
                        fov,
                        max_range,
                    }));
                    info!(
                        target: "lighting",
                        "vision: {token_id} darkvision={darkvision} facing={facing:?} fov={fov}",
                    );
                } else {
                    // Worth saying rather than dropping: a mistyped id would
                    // otherwise look like the vision setting simply had no
                    // effect.
                    warn!(target: "lighting", "set_token_vision: no token {token_id}");
                }
            }
            ExternalCommand::SetAmbientLight { level, color } => {
                if let Some(ambient) = scene.ambient.as_deref_mut() {
                    ambient.level = match level.trim().to_ascii_lowercase().as_str() {
                        "dark" | "dark_ness" | "darkness" | "unlit" => Illumination::Dark,
                        "dim" => Illumination::Dim,
                        // Unknown values read as bright rather than plunging a
                        // scene into darkness on a typo.
                        _ => Illumination::Bright,
                    };
                    ambient.color = color.as_deref().and_then(Rgb::parse_hex);
                    info!(target: "lighting", "ambient: {:?}", ambient.level);
                }
            }
            ExternalCommand::SetLightingOverlay { enabled } => {
                if let Some(overlay) = scene.lighting_overlay.as_deref_mut() {
                    overlay.0 = enabled;
                }
            }
            ExternalCommand::SetRenderProbe { enabled } => {
                if let Some(render_probe) = render_probe.as_deref_mut() {
                    render_probe.0 = enabled;
                    info!(
                        "render probe {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
            }
            ExternalCommand::UpsertCanvasImageAsset {
                asset_id,
                path,
                x,
                y,
                width,
                height,
            } => {
                if let Some(placed_canvas_images) = placed_canvas_images.as_deref_mut() {
                    // Same fix as `SetSceneBackground` above: skip the
                    // write (and the spurious despawn/respawn/reload it
                    // would trigger in `sync_placed_canvas_images`) when a
                    // repeat dispatch carries an identical value.
                    let new_image = PlacedCanvasImage {
                        path,
                        x,
                        y,
                        width,
                        height,
                    };
                    if placed_canvas_images.0.get(&asset_id) != Some(&new_image) {
                        placed_canvas_images.0.insert(asset_id, new_image);
                    }
                }
            }
            ExternalCommand::RemoveCanvasImageAsset { asset_id } => {
                if let Some(placed_canvas_images) = placed_canvas_images.as_deref_mut() {
                    placed_canvas_images.0.remove(&asset_id);
                }
            }
            ExternalCommand::TriggerDiceRoll { dice } => {
                if let Some(pending_dice_roll) = pending_dice_roll.as_deref_mut() {
                    pending_dice_roll.0 = Some(
                        dice.into_iter()
                            .map(|d| plugins::dice_roll::DiceRollDie {
                                final_value: d.final_value,
                            })
                            .collect(),
                    );
                }
            }
        }
    }
}
