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
//! # What this module does not do
//!
//! It does not gate anything yet. Introducing the state and gating the
//! existing always-on systems are deliberately separate steps: the state is
//! additive and safe, while gating changes the behaviour of every authoring
//! system on the canvas and is done one system at a time, with the canvas
//! end-to-end suite run between each. See spec 031, T008 versus T016.

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

fn tool_is_allowed(mode: AuthoringMode) -> bool {
    allowed_tools_slot()
        .lock()
        .ok()
        .map(|slot| match slot.as_ref() {
            None => true,
            Some(allowed) => allowed.contains(&mode),
        })
        .unwrap_or(true)
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
fn leave_a_forbidden_mode(
    current: Res<State<AuthoringMode>>,
    mut next: ResMut<NextState<AuthoringMode>>,
) {
    if !tool_is_allowed(*current.get()) && *current.get() != AuthoringMode::Select {
        next.set(AuthoringMode::Select);
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
        app.init_state::<AuthoringMode>()
            .add_systems(Update, (apply_requested_mode, leave_a_forbidden_mode).chain());
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
}
