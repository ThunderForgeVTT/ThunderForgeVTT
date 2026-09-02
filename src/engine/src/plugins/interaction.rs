//! Things on the canvas that respond, and the seam that dispatches them.
//!
//! Spec 030. This plugin owns placement, hit-testing, entry detection,
//! once-bookkeeping and writing the activation message. It owns **no effect**.
//!
//! # What that means concretely
//!
//! Every effect is performed by the subsystem that owns the thing it changes.
//! Those subsystems contribute their declarations to the registry and add a
//! system that reads [`InteractionActivated`] and handles the identifiers they
//! declared. Nothing in this file knows what any identifier means, and nothing
//! here calls into another plugin.
//!
//! Constitution Principle II names that shape: cross-plugin communication goes
//! through Bevy messages or shared resources, never through direct calls into
//! another plugin's private systems. It is also the only arrangement under
//! which this plugin compiles and runs with every contributor removed —
//! FR-039, which `scripts/verify.mjs` checks *textually* against this file so
//! the violation is greppable rather than a matter of judgement.
//!
//! # The engine is not a second authority
//!
//! Activation is a server mutation, and the server decides. What arrives here
//! has already been permitted; this dispatches it locally so the change is
//! visible immediately instead of a round trip later. A disagreement between
//! the two is a bug in the client, never something to resolve in the client's
//! favour (Principle III, and ADR-054).

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use thunderforge_canvas_core::interaction::{
    EffectDeclaration, EffectRegistry, RegionGeometry, entries_for,
};

// The identity component the command loop actually attaches to a token
// entity. Not `components::Token`, which is a richer shape nothing on the
// canvas currently carries — querying that would compile happily and match no
// entity at all, which is the quietest possible way for entry detection to
// never fire.
use crate::TokenIdentity;

/// One activation, on its way to whichever subsystem performs it.
///
/// Written only after permission has resolved, so a handler never has to ask
/// whether the actor was allowed. It also never learns who they were: an
/// effect that behaved differently per activator would be re-deciding
/// permission, which is not its job.
#[derive(Message, Debug, Clone)]
pub struct InteractionActivated {
    pub interactive_id: String,
    /// The contributed identifier — namespaced, and opaque to this plugin.
    pub effect_id: String,
    /// Configuration, already validated against its own declaration.
    pub config: serde_json::Value,
    /// The token or wall the interactive is attached to, if any.
    pub subject_ref: Option<String>,
}

/// Whether movement counts as play.
///
/// A Game Master dragging a token while preparing a scene and a Game Master
/// dragging one during play are the same gesture, so the distinction cannot be
/// inferred from the movement — the engine has to be told (FR-032).
///
/// Defaults to preparation. A scene that has not said which mode it is in must
/// not fire anything: a trigger that went off while nobody was looking has
/// already spent itself, and there is no undo for that at a table.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScenePlaying(pub bool);

/// The three *spatial* shapes a subject takes.
///
/// Spatial, not semantic, and the distinction is the whole point: a
/// free-standing thing, a segment of the map's own geometry, and a bounded
/// area behave differently to hit-test and to trigger, which is what this
/// plugin needs to know. What any of them *is* in the fiction is the
/// contributing subsystem's business and never this file's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Subject {
    /// A token placed on the map.
    Prop,
    /// A segment of the scene's line geometry.
    Segment,
    /// A bounded area, invisible to players.
    Region,
}

/// One interactive, as the engine holds it.
///
/// Deliberately less than the server stores. A player's client is sent which
/// subjects respond and whether they may activate them — not the effect, not
/// its configuration. The extra fields here are populated only for a Game
/// Master, and the engine treats an absent one as "nothing to dispatch
/// locally" rather than as missing data.
#[derive(Debug, Clone)]
pub struct Interactive {
    pub id: String,
    pub subject: Subject,
    pub subject_ref: Option<String>,
    pub geometry: Option<RegionGeometry>,
    pub effect_id: Option<String>,
    pub config: serde_json::Value,
    /// Whether entry triggers it, as opposed to a click.
    pub on_entry: bool,
    /// Whether this viewer's activation would do anything. A hint for the
    /// cursor, never a permission.
    pub can_activate: bool,
    /// Whether it fires at most once.
    pub once: bool,
    /// Whether it already has.
    pub fired: bool,
}

/// Every interactive on the active scene.
#[derive(Resource, Debug, Default)]
pub struct Interactives {
    entries: BTreeMap<String, Interactive>,
}

impl Interactives {
    pub fn upsert(&mut self, interactive: Interactive) {
        self.entries.insert(interactive.id.clone(), interactive);
    }

    pub fn remove(&mut self, id: &str) -> Option<Interactive> {
        self.entries.remove(id)
    }

    pub fn get(&self, id: &str) -> Option<&Interactive> {
        self.entries.get(id)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Every interactive on the scene.
    ///
    /// Read-only by return type: callers observe, and mutation stays with
    /// `upsert`/`remove`/`clear` so there is one way each change happens.
    pub fn iter(&self) -> impl Iterator<Item = &Interactive> {
        self.entries.values()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every interactive, in id order — stable, so two regions overlapping
    /// fire reproducibly.
    pub fn all(&self) -> impl Iterator<Item = &Interactive> {
        self.entries.values()
    }

    /// The interactive attached to a given subject, if there is one.
    pub fn for_subject(&self, subject_ref: &str) -> Option<&Interactive> {
        self.entries
            .values()
            .find(|i| i.subject_ref.as_deref() == Some(subject_ref))
    }

    /// Which region interactives a move crossed into.
    ///
    /// Entry is a transition, not a state. A token already inside one that
    /// moves a step has entered nothing, and a region firing on every frame of
    /// movement within it reads at the table as the scene stuttering rather
    /// than as a trigger misbehaving (FR-030).
    pub fn entries_crossed(&self, previous: Vec2, current: Vec2) -> Vec<String> {
        let regions: Vec<(&str, &RegionGeometry)> = self
            .entries
            .values()
            .filter(|i| i.on_entry && i.geometry.is_some())
            .map(|i| (i.id.as_str(), i.geometry.as_ref().expect("filtered")))
            .collect();
        entries_for(previous, current, regions)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }
}

/// Where every token was last frame.
///
/// Entry detection needs both endpoints of a move, and a token's `Transform`
/// only carries where it is now. Keeping the previous position here rather
/// than reading a movement message means a jump — a Game Master dropping a
/// token across the map — is still a crossing, which is what a table would
/// expect: the token went through the archway even if it never animated
/// through it.
#[derive(Resource, Debug, Default)]
pub struct PreviousPositions(pub BTreeMap<String, Vec2>);

/// Everything this build can actually perform.
///
/// The union of what was compiled in, and nothing else: each subsystem adds
/// its own declarations through [`contribute`] as its plugin is registered, so
/// this file never learns what any of them are. Remove a plugin and its
/// entries are simply not here.
///
/// # Why the seam holds one of these at all
///
/// A message is fire-and-forget. Nothing can ask it whether anybody listened,
/// so "this build cannot perform that" is not something dispatch can discover
/// after the fact — by then the activation has been spent and the answer is a
/// silence indistinguishable from success (ADR-054, decision 4).
///
/// So it is answered before dispatch, by a lookup. An interactive carrying an
/// identifier nothing here declares is reported unavailable and is not
/// dispatched, not repaired and not deleted: a Game Master who opens a scene
/// in a build missing a subsystem has one thing they cannot use today, not a
/// scene they have lost.
#[derive(Resource, Debug, Default, Clone)]
pub struct AvailableEffects(pub EffectRegistry);

/// Register a subsystem's declarations as this build's.
///
/// Called by a contributing plugin from its own `build`, next to the system
/// that handles what it declares — the declaration and the handler are added
/// and removed together, which is what stops the two drifting apart.
///
/// # Why a collision is a panic
///
/// Two contributors claiming one identifier is a programming error in this
/// build, found at startup, whose fix is a source change. Serving with one of
/// the two quietly dropped would mean an authored interactive stopping work
/// for reasons nothing reports — and the report would arrive at a table,
/// mid-session, from the people least able to act on it.
pub fn contribute(app: &mut App, declarations: Vec<EffectDeclaration>) {
    if !app.world().contains_resource::<AvailableEffects>() {
        app.insert_resource(AvailableEffects::default());
    }
    app.world_mut()
        .resource_mut::<AvailableEffects>()
        .0
        .contribute(declarations)
        .expect("effect declarations collide — a build error, not a runtime one");
}

/// Activations waiting to be written as messages.
///
/// A queue rather than a direct write because the command boundary is outside
/// the ECS: `apply_world_command` runs on whatever thread the browser calls it
/// from, and a message can only be written from a system.
#[derive(Resource, Debug, Default)]
pub struct PendingActivations(pub Vec<InteractionActivated>);

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Interactives>()
            .init_resource::<AvailableEffects>()
            .init_resource::<PreviousPositions>()
            .init_resource::<PendingActivations>()
            .init_resource::<ScenePlaying>()
            .add_message::<InteractionActivated>()
            .add_systems(
                Update,
                (detect_entries, dispatch_pending, publish_snapshot).chain(),
            );
    }
}

/// Fire whatever a token crossed into.
///
/// Runs before dispatch so an entry detected this frame is dispatched this
/// frame, rather than a frame later — at a table the difference is whether the
/// trigger feels attached to the movement.
fn detect_entries(
    playing: Res<ScenePlaying>,
    mut interactives: ResMut<Interactives>,
    mut previous: ResMut<PreviousPositions>,
    tokens: Query<(&TokenIdentity, &Transform)>,
) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut crossed: Vec<String> = Vec::new();

    for (token, transform) in &tokens {
        let current = transform.translation.truncate();
        seen.insert(token.0.clone());
        let before = previous.0.insert(token.0.clone(), current);

        // A token appearing for the first time has not crossed anything. It
        // was placed, and placement is not movement — otherwise loading a
        // scene would fire every region a token happens to be standing in.
        let Some(before) = before else { continue };
        if before == current {
            continue;
        }
        // Preparation moves nothing but the token.
        if !playing.0 {
            continue;
        }
        crossed.extend(interactives.entries_crossed(before, current));
    }

    // Forget tokens that have gone, so a token id reused later does not
    // inherit a stale position and read as an enormous jump.
    previous.0.retain(|id, _| seen.contains(id));

    for id in crossed {
        let Some(interactive) = interactives.get(&id) else {
            continue;
        };
        if interactive.once && interactive.fired {
            continue;
        }

        // Reported, not performed.
        //
        // A crossing is a *trigger*, and the server decides what a trigger
        // does: whether the actor was permitted, whether a `once` has already
        // spent itself, whether it needs approval first. Dispatching here
        // would make the engine a second authority on all three, and the one
        // that disagrees is always the one people believe (Principle III).
        //
        // The application turns this into the same activation a click makes,
        // and the permitted effect comes back through the same command a
        // click's does — so there is one path, not two.
        crate::emit_event(serde_json::json!({
            "type": "interactionTriggered",
            "interactiveId": interactive.id,
            "trigger": "enter",
        }));

        // Marked locally only for a `once`, and only to stop the same crossing
        // being reported twice while the round trip is outstanding. The server
        // holds the real answer and will send it back.
        if interactive.once
            && let Some(entry) = interactives.entries.get_mut(&id)
        {
            entry.fired = true;
        }
    }
}

/// Hand every queued activation to whoever declared its identifier — and
/// report the ones nobody did.
///
/// A message, not a call, so nothing here can tell whether anybody listened.
/// That is why the check comes first: an identifier absent from
/// [`AvailableEffects`] belongs to a subsystem this build does not have, and
/// dispatching it would spend the activation into a silence that reads exactly
/// like success (ADR-054, decision 4, and FR-041).
///
/// The unavailable one is reported outward and dropped. It is not an error and
/// not a reason to change anything the Game Master authored — put the
/// subsystem back and the same interactive works again, with nothing to
/// restore.
///
/// An empty build with no contributors runs this loop, reports each activation
/// unavailable, and dispatches nothing. That is correct rather than broken.
fn dispatch_pending(
    mut pending: ResMut<PendingActivations>,
    available: Res<AvailableEffects>,
    mut writer: MessageWriter<InteractionActivated>,
) {
    for activation in pending.0.drain(..) {
        if !available.0.contains(&activation.effect_id) {
            // Logged on the same channel as a performed one, so a test can
            // tell "reported unavailable" from "vanished" — which is the
            // distinction the whole check exists to make observable.
            crate::dispatched_effects_slot()
                .lock()
                .map(|mut log| {
                    log.push(serde_json::json!({
                        "effectId": activation.effect_id,
                        "interactiveId": activation.interactive_id,
                        "outcome": "unavailable",
                    }));
                })
                .ok();

            crate::emit_event(serde_json::json!({
                "type": "interactionUnavailable",
                "interactiveId": activation.interactive_id,
                "effectId": activation.effect_id,
            }));
            continue;
        }
        writer.write(activation);
    }
}

/// Mirror what the engine holds into the read surface.
///
/// Read-only observation, like `get_token_status`. Kept as a mirror rather
/// than exposing the resource directly because a debugging surface that can
/// also mutate state becomes a way to write tests that pass against situations
/// the application cannot reach.
///
/// Only rewritten when something changed, so an idle canvas serialises
/// nothing.
fn publish_snapshot(interactives: Res<Interactives>) {
    if !interactives.is_changed() {
        return;
    }
    let snapshot: Vec<serde_json::Value> = interactives
        .all()
        .map(|i| {
            serde_json::json!({
                "id": i.id,
                "subject": i.subject,
                "subjectRef": i.subject_ref,
                "effectId": i.effect_id,
                "onEntry": i.on_entry,
                "canActivate": i.can_activate,
                "once": i.once,
                "fired": i.fired,
            })
        })
        .collect();
    if let Ok(mut slot) = crate::interactive_snapshot_slot().lock() {
        *slot = snapshot;
    }
}

/// The interactive at a world point, nearest first, or `None`.
///
/// Hit-testing lives here rather than in the application because the engine
/// owns where things are. `radius` is the caller's tolerance — a click is a
/// blunt instrument and a prop is small.
pub fn hit_test(
    interactives: &Interactives,
    subjects: &BTreeMap<String, Vec2>,
    point: Vec2,
    radius: f32,
) -> Option<String> {
    let mut best: Option<(f32, String)> = None;
    for interactive in interactives.all() {
        let Some(subject_ref) = interactive.subject_ref.as_deref() else {
            continue;
        };
        let Some(position) = subjects.get(subject_ref) else {
            continue;
        };
        let distance = position.distance(point);
        if distance > radius {
            continue;
        }
        match &best {
            Some((closest, _)) if *closest <= distance => {}
            _ => best = Some((distance, interactive.id.clone())),
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    //! Compile-checked only.
    //!
    //! This crate targets `wasm32-unknown-unknown` with no test runner
    //! configured, so nothing below ever executes (Constitution Principle V).
    //! Every rule worth asserting therefore lives in
    //! `thunderforge_canvas_core::interaction`, where `cargo test` runs it —
    //! these exist to keep the types honest as the plugin changes.

    use super::*;

    fn region(id: &str) -> Interactive {
        Interactive {
            id: id.to_string(),
            subject: Subject::Region,
            subject_ref: None,
            geometry: Some(RegionGeometry::Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            effect_id: Some(String::from("thing.do")),
            config: serde_json::Value::Null,
            on_entry: true,
            can_activate: true,
            once: false,
            fired: false,
        }
    }

    #[test]
    fn crossing_in_is_an_entry_and_moving_within_is_not() {
        let mut set = Interactives::default();
        set.upsert(region("a"));

        assert_eq!(
            set.entries_crossed(Vec2::new(-5.0, 5.0), Vec2::new(5.0, 5.0)),
            vec![String::from("a")]
        );
        assert!(
            set.entries_crossed(Vec2::new(2.0, 2.0), Vec2::new(6.0, 6.0))
                .is_empty()
        );
    }
}
