//! Serving the interface packs on disk.
//!
//! Mirrors `crate::systems` deliberately: a manifest is a static document read
//! from a directory and served whole, and routing it through the GraphQL
//! schema would gain nothing while putting a JSON blob in a typed graph.
//!
//! # Why discovery is a directory listing
//!
//! There is no `interface_packs` table and no upload flow. A system pack has a
//! table because it can be installed by an administrator, and installation is
//! a fact that needs recording; nothing about choosing a world's look requires
//! a pack to arrive at runtime. Building the install half of a marketplace now
//! would also make the DMCA guardrail and ADR-029 live questions, which this
//! increment is deliberately not answering.
//!
//! It also gives FR-007 for free: Forge is present because it is in the
//! directory, on exactly the same footing as anything else there — no
//! privileged case, no pinned position, nothing to keep in step.

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use pack_system_spec::interface::{InterfaceManifest, validate};
use serde::Serialize;
use serde_json::json;

use crate::state::AppState;

/// The pack that applies when nothing else does, and the base every other
/// pack's omissions fall through to.
pub const BASE_PACK_ID: &str = "forge";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_packs))
        .route("/{id}/manifest.json", get(get_pack_manifest))
}

/// What a Game Master choosing a pack needs to see in a list.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterfacePackSummary {
    pub id: String,
    pub title: String,
    pub version: String,
    pub description: String,
    /// Empty means the pack composes against any system.
    pub targets: Vec<String>,
}

type Failure = (StatusCode, Json<serde_json::Value>);

fn read_manifest(packs_dir: &str, id: &str) -> Result<InterfaceManifest, String> {
    // Containment: a pack id is a directory name and nothing else. The same
    // guard `crate::mapforge`'s source loader applies, for the same reason —
    // this is the only thing between a path parameter and the rest of the
    // filesystem.
    if id.is_empty() || id.contains("..") || id.contains('/') || id.contains('\\') {
        return Err(format!("{id:?} is not a pack id"));
    }

    let path = std::path::Path::new(packs_dir)
        .join(id)
        .join("interface.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{id}: {e}"))
}

/// The base pack, which every other pack is validated and resolved against.
pub fn base_pack(packs_dir: &str) -> Result<InterfaceManifest, String> {
    read_manifest(packs_dir, BASE_PACK_ID)
}

/// Read a pack and check it, or say why not.
///
/// Fails closed, the way `get_system_manifest` does for `system.json`: a pack
/// that has drifted out of compliance never reaches a browser. A pack is
/// checked against the base rather than alone because contrast is a property
/// of what a reader sees, and what a reader sees is this pack's declarations
/// over the base's.
pub fn load_validated(packs_dir: &str, id: &str) -> Result<InterfaceManifest, Vec<String>> {
    let base = base_pack(packs_dir).map_err(|e| vec![e])?;
    let manifest = read_manifest(packs_dir, id).map_err(|e| vec![e])?;
    validate(&manifest, id, &base)?;
    Ok(manifest)
}

/// Every pack in the directory, in title order.
///
/// No special position for Forge (FR-007, US1 scenario 6). A pack that fails
/// to parse is omitted rather than listed — offering a Game Master something
/// that cannot be applied is worse than not offering it.
pub fn list_installed(packs_dir: &str) -> Vec<InterfacePackSummary> {
    let Ok(entries) = std::fs::read_dir(packs_dir) else {
        return Vec::new();
    };

    let mut out: Vec<InterfacePackSummary> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            let manifest = read_manifest(packs_dir, &id).ok()?;
            Some(InterfacePackSummary {
                id: manifest.id,
                title: manifest.title,
                version: manifest.version,
                description: manifest.description,
                targets: manifest.targets,
            })
        })
        .collect();

    out.sort_by(|a, b| a.title.cmp(&b.title));
    out
}

async fn list_packs(State(state): State<AppState>) -> Json<Vec<InterfacePackSummary>> {
    Json(list_installed(&state.directories.interface_packs_dir))
}

async fn get_pack_manifest(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<InterfaceManifest>, Failure> {
    match load_validated(&state.directories.interface_packs_dir, &id) {
        Ok(manifest) => Ok(Json(manifest)),
        Err(findings) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": format!("Interface pack '{id}' is not usable"),
                "findings": findings,
            })),
        )),
    }
}

#[cfg(test)]
#[path = "interface_packs_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "interface_packs_integration_tests.rs"]
mod integration_tests;

/// The pack to bind a new world to, given the system it is being created on.
///
/// # Why a world gets one at all
///
/// A world created on 5e came out bound to nothing, so it opened in the base
/// pack while `forged-steel` sat installed and unused — a pack written for
/// that ruleset, offered nowhere the person creating the world was looking.
/// The binding is theirs to change from the moment it exists; what this
/// removes is the step of discovering that it should.
///
/// # Why only when the choice is unambiguous
///
/// Exactly one targeting pack is an answer. Two is a **question**, and
/// answering it here — by title order, by directory order, by whatever
/// happens to be first — would be shared code forming a preference between
/// two packs on a table's behalf. So two or more leaves the world on the base
/// pack, which is generic, correct for every system, and visibly a default
/// rather than a silent pick.
///
/// Names no system, in keeping with FR-029: it asks each installed pack what
/// it targets rather than knowing anything itself.
pub fn pack_targeting(packs_dir: &str, system_id: &str) -> Option<String> {
    let mut targeting = list_installed(packs_dir)
        .into_iter()
        .filter(|pack| pack.targets.iter().any(|target| target == system_id));

    let first = targeting.next()?;
    match targeting.next() {
        None => Some(first.id),
        Some(_) => None,
    }
}

/// What a world's pack binding should be after its system changes.
///
/// `current` is what the world is bound to now; the answer is what it should
/// be bound to for `system_id`, with `None` meaning the base pack.
///
/// # Why a binding cannot simply be left alone
///
/// Because a pack that targets a ruleset is *written against it*. Forged
/// Steel's layout names 5e's identifiers — `strengthMod`, `passivePerception`
/// — so a world that moved to Fate while keeping it renders a sheet built for
/// a character it no longer has: almost nothing, with no explanation. Worse,
/// the settings picker only offers packs that target the world's system, so
/// the stale binding is *invisible* there — bound, breaking the sheet, and
/// impossible to see or change from the screen that owns it.
///
/// # What survives, and what does not
///
/// A pack that targets nothing composes against any system, so a GM who chose
/// the base pack, or any other generic one, keeps it: that choice is still
/// true after the change. A pack that names the new system keeps it too, for
/// the same reason.
///
/// Only a pack written for a *different* ruleset is replaced, and then by the
/// same rule that binds one at creation — the pack that targets the new
/// system when exactly one does, and otherwise nothing, because two is a
/// question this is not entitled to answer.
pub fn pack_after_system_change(
    packs_dir: &str,
    current: Option<&str>,
    system_id: &str,
) -> Option<String> {
    if let Some(current) = current {
        let still_fits = list_installed(packs_dir).into_iter().any(|pack| {
            pack.id == current
                && (pack.targets.is_empty()
                    || pack.targets.iter().any(|target| target == system_id))
        });
        if still_fits {
            return Some(current.to_string());
        }
    }

    pack_targeting(packs_dir, system_id)
}
