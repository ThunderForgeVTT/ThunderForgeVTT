use glam::Vec2;

/// A single light source (data-model.md's LightSource section). `id` is
/// `None`/local (represented here the same way `Wall` does it — an empty
/// convention isn't used; instead `id` is simply not-yet-server-confirmed
/// until the matching `upsert_light` command arrives) for a locally-placed
/// light that hasn't been confirmed by the server yet, mirroring `Wall`'s
/// id convention exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct LightSource {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub intensity: f32,
    pub color: Option<String>,
    pub attached_token_id: Option<String>,
    pub casts_shadows: bool,
}

impl LightSource {
    /// Static position, ignoring any `attached_token_id`. Callers that care
    /// about token-attached lights resolve the live position from the
    /// attached entity's `Transform` instead (data-model.md: "x/y are
    /// ignored by the engine in favor of the token's live position" when
    /// `attached_token_id` is set) — that resolution is Bevy/ECS-specific
    /// and lives in `thunderforge_engine::systems::lighting`, not here.
    pub fn position(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// Whether this light is currently emitting any illumination at all
    /// (data-model.md: "a light is on whenever intensity > 0").
    pub fn is_on(&self) -> bool {
        self.intensity > 0.0
    }
}

// ---------------------------------------------------------------------------
// What lighting contributes to the interaction seam (spec 030)
// ---------------------------------------------------------------------------

/// The effect id this subsystem owns.
pub const TOGGLE: &str = "light.toggle";

/// What a light reference points at.
pub const LIGHT: &str = "light";

/// The configuration key naming which lights.
pub const LIGHTS_KEY: &str = "lights";

/// The configuration key saying which way.
pub const MODE_KEY: &str = "mode";

/// What lighting contributes to the interaction registry.
///
/// Declared here, beside lights, so the interaction core stays free of them —
/// the same arrangement doors use, and the reason adding a third subsystem is
/// a module rather than an edit to the rules.
pub fn interaction_effects() -> Vec<crate::interaction::EffectDeclaration> {
    use crate::interaction::{
        ChoiceOption, ConfigField, ConfigFieldKind, EffectDeclaration, SubjectKind,
    };

    vec![EffectDeclaration {
        id: TOGGLE.to_string(),
        label: String::from("Turn lights on or off"),
        description: String::from("Switches one or more lights in this scene."),
        subject_kinds: vec![SubjectKind::Prop, SubjectKind::Door, SubjectKind::Region],
        config: vec![
            ConfigField {
                key: LIGHTS_KEY.to_string(),
                label: String::from("Which lights"),
                // A list, because "the lights in this room" is one switch and
                // several lamps. One reference per lamp would mean one lever
                // per lamp, which is not what a wall switch is.
                kind: ConfigFieldKind::ReferenceList {
                    of: LIGHT.to_string(),
                },
                required: true,
            },
            ConfigField {
                key: MODE_KEY.to_string(),
                label: String::from("Set them to"),
                kind: ConfigFieldKind::Choice {
                    options: vec![
                        ChoiceOption {
                            value: String::from("on"),
                            label: String::from("On"),
                        },
                        ChoiceOption {
                            value: String::from("off"),
                            label: String::from("Off"),
                        },
                        ChoiceOption {
                            value: String::from("toggle"),
                            label: String::from("The other way"),
                        },
                    ],
                },
                required: true,
            },
        ],
    }]
}

/// Which lights a configured `light.toggle` names.
pub fn lights_of(config: &serde_json::Value) -> Vec<&str> {
    config
        .get(LIGHTS_KEY)
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(|i| i.as_str()).collect())
        .unwrap_or_default()
}

/// What a light should be after the effect, given where it is now.
///
/// `None` means the configuration asked for nothing recognisable, which is
/// reported rather than guessed at — a switch that did something arbitrary
/// would be worse than one that did nothing.
pub fn requested_on(config: &serde_json::Value, currently_on: bool) -> Option<bool> {
    match config.get(MODE_KEY)?.as_str()? {
        "on" => Some(true),
        "off" => Some(false),
        "toggle" => Some(!currently_on),
        _ => None,
    }
}

/// Where a switched-off light remembers how bright it was.
///
/// A light is off when its intensity is zero, so switching one off would
/// otherwise destroy the only record of how bright it had been — and a lever
/// pulled twice would leave the room at a brightness nobody chose. Stashed in
/// the light's existing metadata rather than in a new column, because it is
/// bookkeeping for one feature and not a property of a light.
pub const PRIOR_INTENSITY_KEY: &str = "priorIntensity";

/// The intensity to restore when switching a light back on.
///
/// Falls back to full rather than to nothing: a light with no remembered
/// intensity has never been switched off by this feature, and turning it on to
/// zero would look exactly like the switch being broken.
pub fn intensity_to_restore(metadata: Option<&serde_json::Value>) -> f32 {
    metadata
        .and_then(|m| m.get(PRIOR_INTENSITY_KEY))
        .and_then(serde_json::Value::as_f64)
        .map(|v| v as f32)
        .filter(|v| *v > 0.0)
        .unwrap_or(1.0)
}

/// One reversible light edit, pushed onto `LightSet`'s undo stack whenever
/// a confirmed edit is applied locally. Undo re-issues the inverse as a
/// normal outbound mutation (research.md §4), mirroring `WallEdit` exactly.
#[derive(Debug, Clone)]
pub enum LightEdit {
    /// A light was moved (drag reposition). Undo re-issues `update_light`
    /// with the prior position.
    Move {
        light_id: String,
        prior_x: f32,
        prior_y: f32,
    },
    /// A light's radius/intensity was changed (resize control). Undo
    /// re-issues `update_light` with the prior radius/intensity.
    Resize {
        light_id: String,
        prior_radius: f32,
        prior_intensity: f32,
    },
    /// A light's `casts_shadows` flag was toggled. Undo re-issues
    /// `update_light` with the prior flag.
    FlagsToggle {
        light_id: String,
        prior_casts_shadows: bool,
    },
    /// A light was deleted. Undo re-issues `create_light` with the light's
    /// full prior state (note: the re-created light gets a *new* server-
    /// assigned id, same caveat as `WallEdit::Delete`).
    Delete { deleted: LightSource },
}

const MAX_UNDO_STACK: usize = 50;

/// Light list + a bounded per-session undo stack (research.md §4), mirroring
/// `WallSet` exactly. A `Vec` is an intentionally simple index here for the
/// same reason `WallSet` uses one — tens to low-hundreds of lights per scene
/// doesn't justify a spatial index.
///
/// Plain data, no Bevy `Resource` derive — `thunderforge_engine` wraps this
/// in a `Resource` newtype (`src/engine/src/resources/lighting.rs`) so
/// Bevy's change-detection still works transparently on `ResMut` access.
#[derive(Debug, Clone, Default)]
pub struct LightSet {
    lights: Vec<LightSource>,
    undo_stack: Vec<LightEdit>,
    /// Set whenever the light list changes, so illumination-recompute
    /// systems can react to `LightSet` changes — cleared by the system
    /// that consumes it.
    pub dirty: bool,
}

impl LightSet {
    pub fn lights(&self) -> &[LightSource] {
        &self.lights
    }

    pub fn get(&self, id: &str) -> Option<&LightSource> {
        self.lights.iter().find(|l| l.id == id)
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.lights.iter().position(|l| l.id == id)
    }

    /// Insert-or-update by id, mirroring `WallSet::upsert`.
    pub fn upsert(&mut self, light: LightSource) {
        if let Some(index) = self.index_of(&light.id) {
            self.lights[index] = light;
        } else {
            self.lights.push(light);
        }
        self.dirty = true;
    }

    /// Removes and returns the light with the given id, if present.
    pub fn remove(&mut self, id: &str) -> Option<LightSource> {
        let index = self.index_of(id)?;
        self.dirty = true;
        Some(self.lights.remove(index))
    }

    /// Drop every light, and the undo history that referred to them.
    ///
    /// The counterpart of `WallSet::clear`, for the same reason and with the
    /// same caveat about the undo stack — see spec 031 FR-018.
    pub fn clear(&mut self) {
        self.lights.clear();
        self.undo_stack.clear();
        self.dirty = true;
    }

    pub fn push_undo(&mut self, edit: LightEdit) {
        self.undo_stack.push(edit);
        if self.undo_stack.len() > MAX_UNDO_STACK {
            self.undo_stack.remove(0);
        }
    }

    pub fn pop_undo(&mut self) -> Option<LightEdit> {
        self.undo_stack.pop()
    }

    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Only shadow-casting lights need occlusion checks (FR-027: a light
    /// with `casts_shadows == false` always illuminates within its radius,
    /// ignoring walls).
    pub fn shadow_casting_lights(&self) -> impl Iterator<Item = &LightSource> {
        self.lights.iter().filter(|l| l.casts_shadows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(id: &str, x: f32, y: f32, radius: f32) -> LightSource {
        LightSource {
            id: id.to_string(),
            x,
            y,
            radius,
            intensity: 1.0,
            color: None,
            attached_token_id: None,
            casts_shadows: true,
        }
    }

    #[test]
    fn light_geometry_helpers() {
        let l = light("l1", 5.0, 10.0, 100.0);
        assert_eq!(l.position(), Vec2::new(5.0, 10.0));
        assert!(l.is_on());
    }

    #[test]
    fn zero_intensity_light_is_off() {
        let mut l = light("l1", 0.0, 0.0, 100.0);
        l.intensity = 0.0;
        assert!(!l.is_on());
    }

    #[test]
    fn upsert_inserts_then_updates_by_id() {
        let mut lights = LightSet::default();
        lights.upsert(light("l1", 0.0, 0.0, 100.0));
        assert_eq!(lights.lights().len(), 1);

        lights.upsert(light("l1", 5.0, 5.0, 50.0));
        assert_eq!(lights.lights().len(), 1);
        assert_eq!(lights.get("l1").unwrap().x, 5.0);
        assert_eq!(lights.get("l1").unwrap().radius, 50.0);
    }

    #[test]
    fn remove_returns_removed_light() {
        let mut lights = LightSet::default();
        lights.upsert(light("l1", 0.0, 0.0, 100.0));

        let removed = lights.remove("l1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "l1");
        assert!(lights.get("l1").is_none());
    }

    #[test]
    fn remove_missing_id_returns_none() {
        let mut lights = LightSet::default();
        assert!(lights.remove("nope").is_none());
    }

    #[test]
    fn dirty_flag_set_on_upsert_and_remove() {
        let mut lights = LightSet::default();
        assert!(!lights.dirty);
        lights.upsert(light("l1", 0.0, 0.0, 100.0));
        assert!(lights.dirty);

        lights.dirty = false;
        lights.remove("l1");
        assert!(lights.dirty);
    }

    #[test]
    fn undo_stack_is_bounded() {
        let mut lights = LightSet::default();
        for i in 0..(MAX_UNDO_STACK + 10) {
            lights.push_undo(LightEdit::FlagsToggle {
                light_id: format!("l{i}"),
                prior_casts_shadows: true,
            });
        }
        assert_eq!(lights.undo_stack_len(), MAX_UNDO_STACK);
    }

    #[test]
    fn undo_stack_pops_most_recent_first() {
        let mut lights = LightSet::default();
        lights.push_undo(LightEdit::FlagsToggle {
            light_id: "first".to_string(),
            prior_casts_shadows: true,
        });
        lights.push_undo(LightEdit::FlagsToggle {
            light_id: "second".to_string(),
            prior_casts_shadows: false,
        });

        match lights.pop_undo() {
            Some(LightEdit::FlagsToggle { light_id, .. }) => assert_eq!(light_id, "second"),
            _ => panic!("expected FlagsToggle edit"),
        }
    }

    #[test]
    fn shadow_casting_lights_filters_non_shadow_casters() {
        let mut lights = LightSet::default();
        let mut ambient = light("l1", 0.0, 0.0, 100.0);
        ambient.casts_shadows = false;
        lights.upsert(ambient);
        lights.upsert(light("l2", 10.0, 10.0, 100.0));

        let ids: Vec<&str> = lights
            .shadow_casting_lights()
            .map(|l| l.id.as_str())
            .collect();
        assert_eq!(ids, vec!["l2"]);
    }

    // --- what lighting contributes ----------------------------------------

    #[test]
    fn the_declaration_is_namespaced_and_assembles() {
        use crate::interaction::EffectRegistry;

        let registry = EffectRegistry::assemble([interaction_effects()]).expect("one contributor");
        assert_eq!(registry.get(TOGGLE).expect("declared").namespace(), "light");
    }

    #[test]
    fn a_switch_names_several_lights_because_a_room_has_several_lamps() {
        let config = serde_json::json!({ "lights": ["a", "b", "c"], "mode": "off" });
        assert_eq!(lights_of(&config), vec!["a", "b", "c"]);
        assert_eq!(lights_of(&serde_json::json!({})), Vec::<&str>::new());
    }

    #[test]
    fn toggle_means_the_other_way_and_an_explicit_mode_does_not() {
        let toggle = serde_json::json!({ "mode": "toggle" });
        assert_eq!(requested_on(&toggle, true), Some(false));
        assert_eq!(requested_on(&toggle, false), Some(true));

        let on = serde_json::json!({ "mode": "on" });
        assert_eq!(requested_on(&on, true), Some(true));
        assert_eq!(requested_on(&on, false), Some(true));
    }

    #[test]
    fn an_unrecognised_mode_asks_for_nothing() {
        assert_eq!(
            requested_on(&serde_json::json!({ "mode": "dim" }), true),
            None
        );
    }

    #[test]
    fn a_light_switched_off_remembers_how_bright_it_was() {
        // Without this, a lever pulled twice leaves the room at a brightness
        // nobody chose — the off state is intensity zero, which destroys the
        // only record there was.
        let metadata = serde_json::json!({ "priorIntensity": 0.6 });
        assert!((intensity_to_restore(Some(&metadata)) - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn a_light_with_nothing_remembered_comes_back_full_rather_than_dark() {
        // Turning a light on to zero looks exactly like the switch being
        // broken, which is the one outcome worth ruling out.
        assert!((intensity_to_restore(None) - 1.0).abs() < f32::EPSILON);
        assert!(
            (intensity_to_restore(Some(&serde_json::json!({ "priorIntensity": 0.0 }))) - 1.0).abs()
                < f32::EPSILON
        );
    }
}

#[cfg(test)]
mod clear_tests {
    use super::*;

    #[test]
    fn clearing_removes_every_light_and_its_undo_history() {
        let mut set = LightSet::default();
        set.upsert(LightSource {
            id: "l1".to_string(),
            x: 0.0,
            y: 0.0,
            radius: 10.0,
            intensity: 1.0,
            color: None,
            attached_token_id: None,
            casts_shadows: true,
        });
        set.push_undo(LightEdit::Move {
            light_id: "l1".to_string(),
            prior_x: 0.0,
            prior_y: 0.0,
        });
        set.dirty = false;

        set.clear();

        assert!(set.lights().is_empty());
        assert_eq!(set.undo_stack_len(), 0);
        // Illumination recomputes off `dirty`; a cleared scene that did not
        // set it would keep lighting the previous map.
        assert!(set.dirty);
    }
}
