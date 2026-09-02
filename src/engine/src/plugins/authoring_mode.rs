//! Which authoring tool the Game Master is working with.
//!
//! # Why the engine owns this
//!
//! It did not, and that is the bug this exists to close. "Which tool is
//! active" lived entirely in React state (`WorldPage`'s `openGmToolId`), where
//! it chose which flyout to render and *nothing else*. It was never sent here.
//!
//! So every engine-side authoring system was armed at once. `handle_wall_input`
//! is gated by `IsGameMaster` alone; so is token dragging. For a Game Master,
//! a single click on the canvas was offered simultaneously to wall drawing,
//! token dragging and shape placement, and whichever claimed it won.
//!
//! Spec 031 records what that looks like from the outside: switching tools
//! places a stray marker on the map — for every tool *except* text, which is
//! the one sub-tool handled in the DOM and therefore the only one that stops
//! listening when its panel unmounts. The exception is the tell.
//!
//! # Why a state rather than a resource
//!
//! A resource holding the current tool is the same machine with its
//! transitions left implicit, and the transitions are where the bugs are.
//! `OnEnter`/`OnExit` give each mode exactly one place to arm and disarm, and
//! `in_state` gates a system without every system re-deriving "am I on?".
//!
//! # Two gates, and why both
//!
//! The mode says *what* a click means. Which tools a person may use at all is
//! a separate question, answered by the server and pushed in through
//! [`set_allowed_authoring_tools`] — and answered again here, because chrome
//! that hides a button is presentation, not enforcement (FR-047, SC-012). A
//! request arriving from a console, a stale tab, or a client that has not
//! caught up with a revocation is refused by [`set_authoring_mode`]; a viewer
//! already holding a tool they have just lost is moved off it by
//! `leave_a_forbidden_mode`, and their in-flight gesture dies with the
//! `OnExit` every other tool change runs.

use bevy::prelude::*;

/// The authoring tool currently armed.
///
/// Mirrors the Game Master's tool rail one-for-one. `Select` is the default
/// for the same reason the rail opens on it: plain selection is what a GM is
/// doing most of the time, and starting with nothing armed makes "no tool" an
/// unlabelled state reachable only by closing something.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AuthoringMode {
    #[default]
    Select,
    Walls,
    Lights,
    Shapes,
    Tokens,
    Interactions,
}

impl AuthoringMode {
    /// Parse the identifier the web app uses for a tool.
    ///
    /// The strings are the rail's own `GmToolId` values, so the two surfaces
    /// name the same things. An unrecognised value yields `None` and the
    /// caller leaves the mode alone — an unknown tool must not silently rearm
    /// the canvas to something the user did not pick.
    pub fn from_tool_id(id: &str) -> Option<Self> {
        match id {
            "select" => Some(Self::Select),
            "walls" => Some(Self::Walls),
            "lights" => Some(Self::Lights),
            "shapes" => Some(Self::Shapes),
            "tokens" => Some(Self::Tokens),
            "interactions" => Some(Self::Interactions),
            _ => None,
        }
    }

    /// The identifier the web app uses, for reporting the mode back out.
    pub fn as_tool_id(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Walls => "walls",
            Self::Lights => "lights",
            Self::Shapes => "shapes",
            Self::Tokens => "tokens",
            Self::Interactions => "interactions",
        }
    }
}

/// What the web app has most recently asked the mode to become.
///
/// A slot rather than a direct write, for the reason every other boundary here
/// uses one: `App::run()` owns the `World` and never returns on wasm, so there
/// is no handle to set a state from outside the schedule. A system inside the
/// schedule drains this.
static REQUESTED_MODE: std::sync::OnceLock<std::sync::Mutex<Option<AuthoringMode>>> =
    std::sync::OnceLock::new();

fn requested_mode_slot() -> &'static std::sync::Mutex<Option<AuthoringMode>> {
    REQUESTED_MODE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Ask the engine to arm a different authoring tool.
///
/// `tool_id` is the web app's own `GmToolId` string. Returns whether it was
/// recognised: `false` leaves the current mode untouched, because an
/// unrecognised tool must not silently disarm whatever the Game Master had
/// chosen. The caller can treat `false` as "this build does not have that
/// tool", which is a supported way to run rather than an error.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_authoring_mode(tool_id: &str) -> bool {
    let Some(mode) = AuthoringMode::from_tool_id(tool_id) else {
        return false;
    };
    // Refused here, not merely hidden in the rail. FR-047: a tool the viewer
    // may not use must be unusable even when the request arrives directly —
    // from a console, a stale tab, or chrome that has not caught up with a
    // permission that just changed.
    if !tool_is_allowed(mode) {
        return false;
    }
    if let Ok(mut slot) = requested_mode_slot().lock() {
        *slot = Some(mode);
    }
    true
}

/// The mode the engine currently has armed, as the web app's tool id.
///
/// Exists so the boundary is observable from a test or a probe rather than
/// inferred from behaviour — the same reason `engine_stats` exists.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn authoring_mode() -> String {
    CURRENT_MODE
        .get()
        .and_then(|slot| slot.lock().ok().map(|mode| *mode))
        .unwrap_or_default()
        .as_tool_id()
        .to_string()
}

/// Mirrors the live state outward, for `authoring_mode()` above.
static CURRENT_MODE: std::sync::OnceLock<std::sync::Mutex<AuthoringMode>> =
    std::sync::OnceLock::new();

/// Which tools this viewer is allowed to use, or `None` for "no restriction".
///
/// `None` is the default and means the engine imposes no tool-level limit —
/// which is today's behaviour, where `IsGameMaster` alone decides whether a
/// person may author at all. Spec 031 FR-045 requires exactly that default, so
/// existing worlds are unchanged until a Game Master grants something.
///
/// When the set is present it is authoritative here regardless of what chrome
/// is showing. FR-047 is explicit that hiding a tool is not a permission check:
/// a request made directly must be refused too, and this is where that happens.
static ALLOWED_TOOLS: std::sync::OnceLock<std::sync::Mutex<Option<Vec<AuthoringMode>>>> =
    std::sync::OnceLock::new();

fn allowed_tools_slot() -> &'static std::sync::Mutex<Option<Vec<AuthoringMode>>> {
    ALLOWED_TOOLS.get_or_init(|| std::sync::Mutex::new(None))
}

/// Whether this viewer may use `mode` right now.
///
/// Public because it is the check, not a detail of one: `set_authoring_mode`
/// consults it for a request, and [`authoring_tool_allowed`] hands the same
/// answer to the input systems as a run condition. Two spellings of "may I"
/// is how a bypass gets in.
pub fn tool_is_allowed(mode: AuthoringMode) -> bool {
    allowed_tools_slot()
        .lock()
        .ok()
        .map(|slot| match slot.as_ref() {
            None => true,
            Some(allowed) => allowed.contains(&mode),
        })
        .unwrap_or(true)
}

/// A run condition arming a system only while its tool is permitted.
///
/// Gating the input systems as well as the mode request is not belt and
/// braces. `leave_a_forbidden_mode` runs in `Update`, so the state it sets
/// does not take effect until the next frame's `StateTransition` — leaving one
/// whole frame in which `in_state(AuthoringMode::Walls)` is still true for a
/// tool the viewer has just lost. A click landing in that frame would draw.
/// SC-012 says "by any route, in 100% of attempts", and a 16ms window is not
/// 100%.
pub fn authoring_tool_allowed(mode: AuthoringMode) -> impl FnMut() -> bool + Clone {
    move || tool_is_allowed(mode)
}

/// Declare which tools this viewer may use.
///
/// Takes the web app's tool ids, comma-separated. An empty string means "no
/// tools", which is a legitimate state for a player who has been granted none.
/// Unrecognised ids are ignored rather than rejecting the whole list — a build
/// that does not have a tool cannot grant it, and refusing everything because
/// of one unknown name would take away tools the person legitimately has.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_allowed_authoring_tools(tool_ids: &str) {
    let allowed: Vec<AuthoringMode> = tool_ids
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .filter_map(AuthoringMode::from_tool_id)
        .collect();

    if let Ok(mut slot) = allowed_tools_slot().lock() {
        *slot = Some(allowed);
    }
}

/// Remove any tool restriction, returning to the unrestricted default.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn clear_allowed_authoring_tools() {
    if let Ok(mut slot) = allowed_tools_slot().lock() {
        *slot = None;
    }
}

/// Leave a tool the viewer is no longer allowed to use.
///
/// Permission can be revoked while someone is holding the tool. FR-047 says an
/// unusable tool must not be usable; the spec's edge case adds that a gesture
/// in flight must not complete. Dropping back to `Select` does both — `OnExit`
/// on the tool being left abandons its unfinished work, which is the same path
/// a deliberate tool change takes.
///
/// `Select` rather than "no mode": there is no unarmed state, and inventing one
/// here would make a revocation behave differently from every other way of
/// leaving a tool.
/// `Select` rather than "no mode": there is no unarmed state, and inventing one
/// here would make a revocation behave differently from every other way of
/// leaving a tool.
///
/// # Why it also reports
///
/// The edge case spec 031 records is not "the gesture must be discarded" — the
/// `OnExit` handlers already do that — it is that the loss must be *legible*.
/// Dropping silently to `Select` is exactly the failure it names: the tool
/// stops responding and the person is left clicking at a map that has quietly
/// stopped listening, with the rail still showing their tool as armed. So the
/// engine says what it did. Chrome decides how to show it (Principle I); what
/// chrome must not do is have to infer it by polling `authoring_mode()`.
fn leave_a_forbidden_mode(
    current: Res<State<AuthoringMode>>,
    mut next: ResMut<NextState<AuthoringMode>>,
) {
    let mode = *current.get();
    if !tool_is_allowed(mode) && mode != AuthoringMode::Select {
        next.set(AuthoringMode::Select);
        crate::emit_event(serde_json::json!({
            "type": "authoringToolRevoked",
            "tool": mode.as_tool_id(),
        }));
    }
}

/// Applies whatever the web app asked for, once per frame.
fn apply_requested_mode(
    current: Res<State<AuthoringMode>>,
    mut next: ResMut<NextState<AuthoringMode>>,
) {
    let requested = requested_mode_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());

    if let Some(mode) = requested
        && mode != *current.get()
    {
        next.set(mode);
    }

    // Mirror out unconditionally: the readable value should follow the state
    // even when it changed for a reason other than a request.
    if let Ok(mut mirror) = CURRENT_MODE
        .get_or_init(|| std::sync::Mutex::new(AuthoringMode::default()))
        .lock()
    {
        *mirror = *current.get();
    }
}

/// Registers the authoring mode so other plugins can gate on it.
pub struct AuthoringModePlugin;

impl Plugin for AuthoringModePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AuthoringMode>().add_systems(
            Update,
            (apply_requested_mode, leave_a_forbidden_mode).chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These compile under wasm32 and never execute — the crate's constraint,
    // recorded in the constitution. Kept because they document the mapping and
    // do run under a native `cargo test` if this module is ever exercised from
    // one; the rules with real consequences live in `thunderforge-canvas-core`.

    #[test]
    fn tool_ids_round_trip() {
        for mode in [
            AuthoringMode::Select,
            AuthoringMode::Walls,
            AuthoringMode::Lights,
            AuthoringMode::Shapes,
            AuthoringMode::Tokens,
            AuthoringMode::Interactions,
        ] {
            assert_eq!(AuthoringMode::from_tool_id(mode.as_tool_id()), Some(mode));
        }
    }

    #[test]
    fn an_unknown_tool_id_is_rejected_rather_than_defaulted() {
        // Defaulting to Select here would silently disarm whatever the user
        // had chosen, which is worse than ignoring the instruction.
        assert_eq!(AuthoringMode::from_tool_id("wombat"), None);
        assert_eq!(AuthoringMode::from_tool_id(""), None);
    }

    #[test]
    fn select_is_the_default() {
        assert_eq!(AuthoringMode::default(), AuthoringMode::Select);
    }

    /// The default is *unrestricted*, and that is deliberate: the engine is
    /// the second gate, not the first. A build nobody has told about tool
    /// permissions must behave exactly as it did before they existed —
    /// FR-045's "existing worlds are unchanged", seen from this side.
    #[test]
    fn no_declaration_restricts_nothing() {
        clear_allowed_authoring_tools();
        for mode in [
            AuthoringMode::Select,
            AuthoringMode::Walls,
            AuthoringMode::Tokens,
        ] {
            assert!(tool_is_allowed(mode));
        }
    }

    /// The whole of SC-012 at this boundary: a request made directly, with no
    /// rail involved, is refused for a tool the viewer does not hold.
    #[test]
    fn a_direct_request_for_a_forbidden_tool_is_refused() {
        set_allowed_authoring_tools("select,walls");

        assert!(set_authoring_mode("walls"), "a granted tool is accepted");
        assert!(
            !set_authoring_mode("lights"),
            "a tool the viewer does not hold must be refused even when asked for directly"
        );

        clear_allowed_authoring_tools();
    }

    /// An empty grant is a real answer, not a missing one. A player whose Game
    /// Master has granted nothing holds nothing — the reading that treats an
    /// empty list as "no restriction" is the one that would hand every tool to
    /// everybody.
    #[test]
    fn granting_nothing_forbids_everything() {
        set_allowed_authoring_tools("");
        assert!(!tool_is_allowed(AuthoringMode::Walls));
        assert!(!tool_is_allowed(AuthoringMode::Select));
        assert!(!set_authoring_mode("walls"));
        clear_allowed_authoring_tools();
    }

    /// One unknown name must not cost a person the tools they do hold — a
    /// build that lacks a tool cannot grant it, and refusing the whole list
    /// would fail *open* on the wrong side by disarming a legitimate grant.
    #[test]
    fn an_unknown_name_in_a_grant_does_not_void_the_rest() {
        set_allowed_authoring_tools("walls, wombat ,shapes");
        assert!(tool_is_allowed(AuthoringMode::Walls));
        assert!(tool_is_allowed(AuthoringMode::Shapes));
        assert!(!tool_is_allowed(AuthoringMode::Lights));
        clear_allowed_authoring_tools();
    }
}
