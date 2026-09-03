use crate::auth_middleware::AuthenticatedUser;
use crate::models::NewGameSystem;
use crate::schema::game_systems;
use crate::state::AppState;
use axum::{
    Router,
    extract::{Extension, Multipart, Path, State},
    http::{StatusCode, header},
    response::{Json, Response},
    routing::{get, post},
};
use diesel::prelude::*;
use pack_system_spec::{SystemManifest, validate_system_manifest};
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_systems))
        .route("/{slug}/manifest.json", get(get_system_manifest))
        .route("/{slug}/download", get(download_system_package))
        .route("/{slug}/{*path}", get(serve_system_file))
}

pub fn admin_router() -> Router<AppState> {
    Router::new().route("/install", post(install_game_system))
}

/// What a Game Master choosing a system needs to see in a list.
///
/// Mirrors `crate::interface_packs::InterfacePackSummary` — same four fields,
/// same reason. A summary is what a picker needs; the manifest is served whole
/// by `get_system_manifest` for anything that needs more.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameSystemSummary {
    pub id: String,
    pub title: String,
    pub description: String,
    pub version: String,
}

/// Every system pack in the directory, in title order.
///
/// # Why the directory and not the `game_systems` table
///
/// This route read that table until spec 032 T085, and the table holds **zero
/// rows** — measured, not assumed. Nothing has ever seeded it with the bundled
/// packs, so the honest answer it gave was an empty list, and
/// `apps/web/src/api/gameSystems.ts` compensated with two hand-kept literals
/// naming all seven systems and their titles. That is the hardcoded list
/// SC-004 forbids, and it existed because this function was looking in the
/// wrong place.
///
/// `interface_packs::list_installed` already reads its directory, which is why
/// nothing on the client hardcodes interface pack names. The asymmetry was the
/// bug. A row per installed system earns its place when a system can be
/// installed at runtime; ADR-029 says it cannot.
///
/// A pack that fails to parse is omitted rather than listed — offering a Game
/// Master something that cannot be chosen is worse than not offering it, and
/// it is the same call `list_installed` makes.
pub fn list_installed(systems_dir: &str) -> Vec<GameSystemSummary> {
    let Ok(entries) = std::fs::read_dir(systems_dir) else {
        return Vec::new();
    };

    let mut out: Vec<GameSystemSummary> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path().join("system.json")).ok()?;
            let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;

            // A pack may declare itself a starting point rather than a
            // ruleset — one bundled pack does, describing itself as a
            // blank-slate template meant to be forked. Offering it in a picker
            // beside the real rulesets would be offering a fork of somebody's
            // future work as a thing to play.
            //
            // A *declaration*, deliberately, not a name in this file. The
            // whole point of reading the directory is that shared code knows
            // no system's identity, and comparing the id against a literal
            // here would put back exactly what T085 took out.
            if manifest
                .get("template")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                return None;
            }

            Some(GameSystemSummary {
                id: manifest.get("id")?.as_str()?.to_owned(),
                title: manifest.get("title")?.as_str()?.to_owned(),
                description: manifest
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                version: manifest
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .collect();

    out.sort_by(|a, b| a.title.cmp(&b.title));
    out
}

/// What `/api/systems` answers: the systems, and which one a new world gets.
///
/// The default belongs here rather than in a second route because it is the
/// same question — "what does this deployment offer, and what does it pick" —
/// and because the alternative was the client answering the second half from
/// a literal — `CreateWorldPage` opened with one system's id hardcoded as its
/// initial state, which is shared web code naming a system by another route.
///
/// `null` is a real answer. An operator who configured no default gets a
/// world with no system, which is a state the product handles; guessing one
/// would bind a world to a ruleset nobody chose.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSystems {
    pub systems: Vec<GameSystemSummary>,
    pub default_id: Option<String>,
}

async fn list_systems(State(state): State<AppState>) -> Json<InstalledSystems> {
    let systems = list_installed(&state.directories.systems_dir);
    // A default naming a system this deployment does not have is not a
    // default — offering it would preselect a choice that cannot be honoured.
    let default_id = crate::admin::default_game_system_id(&state)
        .filter(|id| systems.iter().any(|system| &system.id == id));

    Json(InstalledSystems {
        systems,
        default_id,
    })
}

async fn get_system_manifest(
    Path(slug): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let systems_dir_path = PathBuf::from(&state.directories.systems_dir);
    let system_path = systems_dir_path.join(&slug).join("system.json");

    let manifest_content = fs::read_to_string(&system_path).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND, // Or INTERNAL_SERVER_ERROR if file exists but can't be read
            Json(json!({"error": format!("Failed to read manifest file: {}", e)})),
        )
    })?;

    let manifest_json: serde_json::Value =
        serde_json::from_str(&manifest_content).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to parse manifest JSON: {}", e)})),
            )
        })?;

    // Spec 016 (FR-007, SC-003): fail closed rather than serve a manifest
    // with missing/empty legal metadata to a GM. This is the actual path
    // that delivers a bundled pack's manifest today (unlike
    // `pack_system_spec::validate_system_manifest`, used only by the
    // admin-upload/install flow), so this is the real enforcement point.
    pack_system_spec::validate_legal_content(&manifest_json).map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"error": format!("System pack '{slug}' has a non-compliant manifest: {e}")}),
            ),
        )
    })?;

    Ok(Json(manifest_json))
}

async fn download_system_package(
    Path(slug): Path<String>,
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let systems_dir_path = PathBuf::from(&state.directories.systems_dir);
    let zip_path = systems_dir_path.join(&slug).join("boilerplate.zip"); // Assuming boilerplate.zip

    // Check if the file exists
    if !zip_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("System package not found for slug: {}", slug)})),
        ));
    }

    // Read the file content
    let file_content = fs::read(&zip_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to read system package file: {}", e)})),
        )
    })?;

    // Create a response with appropriate headers
    let filename = format!("{}.zip", slug);
    let content_disposition = format!("attachment; filename=\"{}\"", filename);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .body(axum::body::Body::from(file_content))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to construct response: {}", e)})),
            )
        })?;

    Ok(response)
}

/// Serve system files (JavaScript modules, CSS, assets, etc.)
/// Supports paths like /systems/:slug/module/main.mjs, /systems/:slug/styles/main.css
async fn serve_system_file(
    Path((slug, path)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Prevent directory traversal attacks
    if path.contains("..") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid path: directory traversal not allowed"})),
        ));
    }

    let systems_dir_path = PathBuf::from(&state.directories.systems_dir);
    let file_path = systems_dir_path.join(&slug).join(&path);

    // Verify the resolved path is within the system directory
    let canonical_system_dir = systems_dir_path.join(&slug).canonicalize().map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "System directory not found"})),
        )
    })?;

    let canonical_file = file_path.canonicalize().map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "File not found"})),
        )
    })?;

    if !canonical_file.starts_with(&canonical_system_dir) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }

    // Read the file
    let file_content = fs::read(&file_path).await.map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("Failed to read file: {}", e)})),
        )
    })?;

    // Determine MIME type based on file extension
    let content_type = match file_path.extension().and_then(|ext| ext.to_str()) {
        Some("mjs") | Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") => "text/plain; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    };

    // Build response with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=3600") // Cache for 1 hour
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*") // CORS for module loading
        .body(axum::body::Body::from(file_content))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to construct response: {}", e)})),
            )
        })?;

    Ok(response)
}

async fn install_game_system(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    mut multipart: Multipart, // New extractor
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut temp_zip_path: Option<PathBuf> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Failed to read multipart field: {}", e)})),
        )
    })? {
        let name = field
            .name()
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Multipart field name missing"})),
                )
            })?
            .to_string();

        if name == "package" {
            // Assuming the file input field is named "package"
            let file_name = field
                .file_name()
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "File name missing for package upload"})),
                    )
                })?
                .to_string();

            if !file_name.ends_with(".zip") {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Only .zip files are allowed for package uploads"})),
                ));
            }

            let temp_file = tempfile::NamedTempFile::new().map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to create temporary file: {}", e)})),
                )
            })?;
            let current_temp_path = temp_file.path().to_owned(); // Call .path() before into_file()
            let mut temp_async_file = tokio::fs::File::from_std(temp_file.into_file());

            let contents = field.bytes().await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to read file bytes: {}", e)})),
                )
            })?;
            temp_async_file.write_all(&contents).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to write to temporary file: {}", e)})),
                )
            })?;
            temp_zip_path = Some(current_temp_path);
            break; // Assuming only one package file
        }
    }

    let Some(zip_file_path) = temp_zip_path else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No package file found in multipart upload"})),
        ));
    };

    // Extract the zip file
    let extract_dir = tempfile::tempdir().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create temporary directory for extraction: {}", e)})),
        )
    })?;
    let extract_path = extract_dir.path().to_owned();

    let file = std::fs::File::open(&zip_file_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to open zip file for extraction: {}", e)})),
        )
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to read zip archive: {}", e)})),
        )
    })?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to read file from zip archive: {}", e)})),
            )
        })?;
        let outpath = match file.enclosed_name() {
            Some(path) => extract_path.join(path),
            None => continue,
        };

        // Security: Prevent path traversal by ensuring extracted path is within extract_path
        if !outpath.starts_with(&extract_path) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"error": format!("Attempted path traversal detected: {}", outpath.display())}),
                ),
            ));
        }

        if (*file.name()).ends_with('/') {
            // It's a directory
            std::fs::create_dir_all(&outpath).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to create directory during extraction: {}", e)})),
                )
            })?;
        } else {
            // It's a file
            if let Some(p) = outpath.parent()
                && !p.exists()
            {
                std::fs::create_dir_all(p).map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Failed to create parent directory during extraction: {}", e)})),
                        )
                    }
                    )?;
            }
            let mut outfile = std::fs::File::create(&outpath).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        json!({"error": format!("Failed to create file during extraction: {}", e)}),
                    ),
                )
            })?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to copy file during extraction: {}", e)})),
                )
            })?;
        }
    }

    let manifest_file_path = extract_path.join("system.json");
    if !manifest_file_path.exists() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Extracted package does not contain system.json"})),
        ));
    }

    let manifest_content = tokio::fs::read_to_string(&manifest_file_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to read system.json from extracted package: {}", e)})),
            )
        })?;

    // Validate the manifest using the pack_system_spec crate
    validate_system_manifest(&manifest_content).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("System manifest validation failed: {}", e)})),
        )
    })?;

    // Parse the manifest into SystemManifest struct to get the ID and version
    let system_manifest: SystemManifest = serde_json::from_str(&manifest_content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({"error": format!("Failed to parse SystemManifest after validation: {}", e)}),
            ),
        )
    })?;

    let system_slug = system_manifest.id.clone();
    let system_title = system_manifest.title.clone();
    let system_version = system_manifest.version.clone();

    // Ensure the system slug is valid (alphanumeric, -, _, .)
    if !system_slug
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error": format!("Invalid system slug: {}. Slug must be alphanumeric, hyphen, underscore, or dot", system_slug)}),
            ),
        ));
    }

    // Clone pool + slug for first closure
    let pool_for_check = state.db_pool.clone();
    let slug_for_check = system_slug.clone();

    let existing_system_count = tokio::task::spawn_blocking(move || {
        let mut conn = pool_for_check.get().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get DB connection"})),
            )
        })?;
        game_systems::table
            .filter(game_systems::slug.eq(&slug_for_check))
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to query existing systems: {}", e)})),
                )
            })
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to spawn blocking task: {}", e)})),
        )
    })??; // Propagate the error from the blocking task

    if existing_system_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error": format!("System with slug '{}' already exists", system_slug)})),
        ));
    }

    // Move extracted files to permanent storage
    let final_system_path = PathBuf::from(&state.directories.systems_dir).join(&system_slug);

    if final_system_path.exists() {
        // This should ideally not happen if slug check passed, but as a safeguard
        tokio::fs::remove_dir_all(&final_system_path).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to clean up existing system directory: {}", e)})),
            )
        })?;
    }

    tokio::fs::rename(&extract_path, &final_system_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to move extracted system files to permanent storage: {}", e)})),
        )
    })?;

    // Insert into database
    let new_system = NewGameSystem {
        slug: system_slug.clone(),
        title: system_title.clone(),
        manifest_url: format!("/api/systems/{}/manifest.json", system_slug),
        version: system_version.clone(),
        installed_by: auth_user.user_id,
    };

    // Clone pool again for second closure
    let pool_for_insert = state.db_pool.clone();

    tokio::task::spawn_blocking(move || {
        let mut conn = pool_for_insert
            .get()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get DB connection for insertion"}))))?;
        diesel::insert_into(game_systems::table)
            .values(&new_system)
            .execute(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to insert new game system into DB: {}", e)}))))
    }).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to spawn blocking task for DB insertion: {}", e)}))))??; // Propagate the error from the blocking task

    Ok(Json(
        json!({"message": format!("Game system '{}' (v{}) installed successfully!", system_title, system_version)}),
    ))
}
// ============================================================================
// Phase 4.8.1: System-Agnostic Game System Validator Registry
// ============================================================================
// Dynamic loader for game system validators and metadata
// Supports unlimited systems (D&D 5e, Pathfinder, CoC 7e, etc.) without schema migrations

use serde_json::Value;
use std::collections::HashMap;

/// Validator function signature: takes JSON data, returns Result<(), error message>
pub type ValidatorFn = fn(&Value) -> Result<(), String>;

/// Game system validator registry entry
pub struct SystemValidators {
    pub ability_data: Option<ValidatorFn>,
    pub resource_data: Option<ValidatorFn>,
    pub proficiency_data: Option<ValidatorFn>,
    pub trait_data: Option<ValidatorFn>,
    pub spell_data: Option<ValidatorFn>,
}

/// Game system validation registry
pub struct GameSystemRegistry {
    systems: HashMap<String, SystemValidators>,
}

impl GameSystemRegistry {
    /// Create new empty registry
    pub fn new() -> Self {
        Self {
            systems: HashMap::new(),
        }
    }

    /// Register a system's validators
    pub fn register(&mut self, system_id: &str, validators: SystemValidators) {
        self.systems.insert(system_id.to_string(), validators);
    }

    /// Validate data using system-specific validator
    /// Returns Ok(()) if valid, Err(message) if invalid
    pub fn validate(&self, system_id: &str, data_type: &str, data: &Value) -> Result<(), String> {
        let system = self
            .systems
            .get(system_id)
            .ok_or_else(|| format!("System '{}' not registered", system_id))?;

        let validator = match data_type {
            "ability_data" => system.ability_data,
            "resource_data" => system.resource_data,
            "proficiency_data" => system.proficiency_data,
            "trait_data" => system.trait_data,
            "spell_data" => system.spell_data,
            _ => return Err(format!("Unknown data_type: {}", data_type)),
        };

        match validator {
            Some(validate_fn) => validate_fn(data),
            None => Ok(()), // No validator for this data type in this system
        }
    }
}

// ============================================================================
// The registry, assembled from what the packs contributed
// ============================================================================

use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Every bundled system's validators, collected rather than listed.
///
/// # What this replaced
///
/// Seven `register_*_system` functions lived here, each naming a system id as
/// a string literal and wiring five validator function pointers by hand, all
/// called from this initialiser — which already carried a `// In future
/// phases: register_coc7e_system(...)` comment waiting to be the eighth.
/// Adding a system meant editing shared server code that had to know that
/// system's name and the shape of its stored data, which is exactly what spec
/// 032's SC-004 measures and FR-029 forbids.
///
/// Now each pack declares its own contribution next to the validators it is
/// declaring, and this collects them without naming one. A pack whose data
/// shape changes changes one file — its own.
///
/// The one thing that is still a list is `crate::system_packs`, which holds a
/// `use <pack> as _;` line per pack because an unreferenced Rust crate is
/// never linked and its submissions vanish with it. Those lines carry no
/// information about the systems they name, which is why they can be a list
/// and a validator table could not.
pub static GAME_SYSTEMS: Lazy<Mutex<GameSystemRegistry>> = Lazy::new(|| {
    let mut registry = GameSystemRegistry::new();

    for contribution in thunderforge_canvas_core::system_contribution::contributions() {
        registry.register(
            contribution.id,
            SystemValidators {
                ability_data: contribution.ability_data,
                resource_data: contribution.resource_data,
                proficiency_data: contribution.proficiency_data,
                trait_data: contribution.trait_data,
                spell_data: contribution.spell_data,
            },
        );
    }

    Mutex::new(registry)
});

/// The rules a system contributes, built from that system's own manifest.
///
/// `None` for a system that computes nothing, which is a fact about the
/// ruleset rather than a gap: Fate Core has no derived scores to publish.
/// `None` also for a system this build does not have, which is why a caller
/// must treat the two the same way — a world bound to a missing pack still
/// opens, with its stored values intact (FR-019).
pub fn rules_for_system(
    system_id: &str,
    manifest: &serde_json::Value,
) -> Option<Box<dyn thunderforge_canvas_core::system_rules::SystemRules>> {
    let contribution = thunderforge_canvas_core::system_contribution::contribution_for(system_id)?;
    contribution.rules.map(|build| build(manifest))
}

/// Validate actor system data using globally registered validators
/// This is the main entry point from GraphQL mutations
pub fn validate_actor_system_data(
    game_system_id: &str,
    data_type: &str,
    data: &Value,
) -> Result<(), String> {
    let registry = GAME_SYSTEMS.lock().map_err(|e| e.to_string())?;
    registry.validate(game_system_id, data_type, data)
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = GameSystemRegistry::new();
        assert_eq!(registry.systems.len(), 0);
    }

    #[test]
    fn test_registry_register_and_validate() {
        let mut registry = GameSystemRegistry::new();

        // Simple test validator that rejects values > 100
        fn test_validator(data: &Value) -> Result<(), String> {
            if let Some(val) = data.get("test_field").and_then(|v| v.as_i64())
                && val > 100
            {
                return Err("Value too large".to_string());
            }
            Ok(())
        }

        registry.register(
            "test_system",
            SystemValidators {
                ability_data: Some(test_validator),
                resource_data: None,
                proficiency_data: None,
                trait_data: None,
                spell_data: None,
            },
        );

        // Should pass
        let valid_data = serde_json::json!({ "test_field": 50 });
        assert!(
            registry
                .validate("test_system", "ability_data", &valid_data)
                .is_ok()
        );

        // Should fail
        let invalid_data = serde_json::json!({ "test_field": 150 });
        assert!(
            registry
                .validate("test_system", "ability_data", &invalid_data)
                .is_err()
        );

        // Unknown system
        assert!(
            registry
                .validate("unknown_system", "ability_data", &valid_data)
                .is_err()
        );

        // Unknown data type
        assert!(
            registry
                .validate("test_system", "unknown_type", &valid_data)
                .is_err()
        );
    }

    #[test]
    fn test_global_registry_dnd5e_registered() {
        let registry = GAME_SYSTEMS.lock().unwrap();
        // D&D 5e should be registered on first access
        assert!(registry.systems.contains_key("dnd5e"));
    }
}

/// Spec 016 (T005, FR-007): confirms `get_system_manifest` — the actual
/// path that serves a bundled pack's manifest to a GM, distinct from
/// `pack_system_spec::validate_system_manifest`'s admin-upload-only usage
/// — rejects a manifest missing `legal`, and serves one that has it.
#[cfg(test)]
mod manifest_legal_enforcement_tests {
    use super::*;
    use crate::test_support::test_app_state;
    use axum::extract::{Path as AxumPath, State};

    fn state_with_temp_systems_dir() -> (AppState, std::path::PathBuf) {
        let mut state = test_app_state();
        let tmp = std::env::temp_dir().join(format!("tf-systems-test-{}", uuid::Uuid::now_v7()));
        state.directories.systems_dir = tmp.to_str().unwrap().to_string();
        (state, tmp)
    }

    fn write_manifest(systems_dir: &std::path::Path, slug: &str, manifest_json: &str) {
        let pack_dir = systems_dir.join(slug);
        std::fs::create_dir_all(&pack_dir).unwrap();
        std::fs::write(pack_dir.join("system.json"), manifest_json).unwrap();
    }

    /// Spec 032 T085: the list comes from the directory, not the table.
    ///
    /// The point of this test is what it does *not* do — it never touches
    /// `game_systems`. That table holds zero rows on every install, which is
    /// why the client had to carry a hand-kept list of all seven systems; a
    /// pack written into a temp directory and seeded nowhere must be listed.
    #[test]
    fn list_installed_reports_a_pack_that_was_never_seeded_into_the_table() {
        let (_state, systems_dir) = state_with_temp_systems_dir();
        write_manifest(
            &systems_dir,
            "unseeded-pack",
            r#"{"id": "unseeded-pack", "title": "Unseeded Pack",
                "description": "Never inserted anywhere.", "version": "2.1.0"}"#,
        );

        let listed = list_installed(systems_dir.to_str().unwrap());

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "unseeded-pack");
        assert_eq!(listed[0].title, "Unseeded Pack");
        assert_eq!(listed[0].version, "2.1.0");
        assert_eq!(listed[0].description, "Never inserted anywhere.");
    }

    /// Title order, not directory order, which is whatever the filesystem
    /// hands back. A picker that reorders itself between two machines is a
    /// picker nobody can be told where to click.
    #[test]
    fn list_installed_orders_by_title() {
        let (_state, systems_dir) = state_with_temp_systems_dir();
        for (slug, title) in [("zzz", "Aardvark"), ("aaa", "Zeppelin"), ("mmm", "Middle")] {
            write_manifest(
                &systems_dir,
                slug,
                &format!(r#"{{"id": "{slug}", "title": "{title}", "version": "1.0.0"}}"#),
            );
        }

        let listed = list_installed(systems_dir.to_str().unwrap());
        let titles: Vec<&str> = listed.iter().map(|s| s.title.as_str()).collect();

        assert_eq!(titles, vec!["Aardvark", "Middle", "Zeppelin"]);
    }

    /// A directory that is not a readable pack is omitted, not listed with a
    /// blank name. Offering a Game Master something that cannot be chosen is
    /// worse than not offering it.
    #[test]
    fn list_installed_omits_a_pack_it_cannot_read() {
        let (_state, systems_dir) = state_with_temp_systems_dir();
        write_manifest(&systems_dir, "broken", "{ this is not json");
        write_manifest(
            &systems_dir,
            "untitled",
            r#"{"id": "untitled", "version": "1.0.0"}"#,
        );
        write_manifest(
            &systems_dir,
            "fine",
            r#"{"id": "fine", "title": "Fine", "version": "1.0.0"}"#,
        );

        let listed = list_installed(systems_dir.to_str().unwrap());
        let ids: Vec<&str> = listed.iter().map(|s| s.id.as_str()).collect();

        assert_eq!(ids, vec!["fine"]);
    }

    /// A pack may say it is a starting point rather than a ruleset, and be
    /// believed. `basic-game-system` is the one that does.
    ///
    /// The declaration is the mechanism on purpose: shared code omitting a
    /// pack by name would put back exactly the hardcoded knowledge T085 took
    /// out, and this test would pass either way — which is why the second
    /// assertion is here, naming a template this file has never heard of.
    #[test]
    fn list_installed_omits_a_pack_that_declares_itself_a_template() {
        let (_state, systems_dir) = state_with_temp_systems_dir();
        write_manifest(
            &systems_dir,
            "starting-point",
            r#"{"id": "starting-point", "title": "Starting Point",
                "version": "1.0.0", "template": true}"#,
        );
        write_manifest(
            &systems_dir,
            "a-ruleset",
            r#"{"id": "a-ruleset", "title": "A Ruleset", "version": "1.0.0"}"#,
        );

        let listed = list_installed(systems_dir.to_str().unwrap());
        let ids: Vec<&str> = listed.iter().map(|s| s.id.as_str()).collect();

        assert_eq!(ids, vec!["a-ruleset"]);
    }

    /// The bundled packs, listed from the real directory this deployment
    /// ships — the case the client used to answer from a literal.
    #[test]
    fn the_bundled_packs_directory_lists_every_shipping_system() {
        let packs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packs/systems")
            .canonicalize()
            .expect("packs/systems must exist");

        let listed = list_installed(packs.to_str().unwrap());

        // Every directory that is not a declared template, and nothing else.
        let expected = std::fs::read_dir(&packs)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                let text = std::fs::read_to_string(e.path().join("system.json")).unwrap();
                let m: serde_json::Value = serde_json::from_str(&text).unwrap();
                m.get("template").and_then(serde_json::Value::as_bool) != Some(true)
            })
            .count();

        assert_eq!(listed.len(), expected);
        assert!(
            listed.len() >= 7,
            "expected the shipping rulesets, got {listed:?}"
        );
        assert!(
            !listed.iter().any(|s| s.id == "basic-game-system"),
            "the template pack must not be offered as a ruleset"
        );
        assert!(listed.iter().all(|s| !s.title.is_empty()));
    }

    #[tokio::test]
    async fn get_system_manifest_rejects_a_manifest_missing_legal() {
        let (state, systems_dir) = state_with_temp_systems_dir();
        write_manifest(
            &systems_dir,
            "no-legal-pack",
            r#"{"id": "no-legal-pack", "title": "No Legal Pack", "version": "0.1.0"}"#,
        );

        let result = get_system_manifest(AxumPath("no-legal-pack".to_string()), State(state)).await;

        let (status, _) = result.expect_err("manifest missing legal must be rejected");
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn get_system_manifest_serves_a_manifest_with_valid_legal() {
        let (state, systems_dir) = state_with_temp_systems_dir();
        write_manifest(
            &systems_dir,
            "compliant-pack",
            r#"{
                "id": "compliant-pack",
                "title": "Compliant Pack",
                "version": "0.1.0",
                "legal": {
                    "licenseName": "CC-BY-4.0",
                    "attributionText": "Built from an open reference document."
                }
            }"#,
        );

        let result =
            get_system_manifest(AxumPath("compliant-pack".to_string()), State(state)).await;

        let Json(manifest) = result.expect("a compliant manifest must be served");
        assert_eq!(manifest["legal"]["licenseName"], "CC-BY-4.0");
    }

    /// Spec 016 (T006, SC-001): the real, shipped `dnd5e` manifest — not a
    /// synthetic fixture — has a compliant `legal` object.
    #[tokio::test]
    async fn dnd5e_system_json_has_a_compliant_legal_object() {
        let mut state = test_app_state();
        // test_app_state()'s Directories::from(temp_dir()) computes
        // systems_dir under the temp dir, not this repo's real
        // packs/systems — point it at the real one so this exercises the
        // actual shipped manifest, not a fixture.
        let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packs/systems")
            .canonicalize()
            .expect("packs/systems must exist relative to src/server");
        state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

        let result = get_system_manifest(AxumPath("dnd5e".to_string()), State(state)).await;

        let Json(manifest) = result.expect("dnd5e's real manifest must pass legal validation");
        assert_eq!(manifest["legal"]["licenseName"], "CC-BY-4.0");
        assert!(
            manifest["legal"]["attributionText"]
                .as_str()
                .unwrap()
                .contains("System Reference Document")
        );
    }

    /// Spec 018 (T014): the real, shipped `genie` manifest has a compliant
    /// `legal` object declaring original, ThunderForgeVTT-owned content —
    /// mirrors dnd5e_system_json_has_a_compliant_legal_object above.
    #[tokio::test]
    async fn genie_system_json_has_a_compliant_legal_object() {
        let mut state = test_app_state();
        let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packs/systems")
            .canonicalize()
            .expect("packs/systems must exist relative to src/server");
        state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

        let result = get_system_manifest(AxumPath("genie".to_string()), State(state)).await;

        let Json(manifest) = result.expect("genie's real manifest must pass legal validation");
        assert_eq!(
            manifest["legal"]["licenseName"],
            "ThunderForgeVTT Original Content"
        );
        assert!(
            manifest["legal"]["trademarkRestrictions"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    /// Spec 016 (Edge Cases, "no external license at all"): the real,
    /// shipped `basic-game-system` manifest — a minimal, generic starter
    /// template pack with no third-party-derived content — has a compliant
    /// `legal` object declaring original, ThunderForgeVTT-owned content.
    /// Mirrors genie_system_json_has_a_compliant_legal_object above.
    #[tokio::test]
    async fn basic_game_system_json_has_a_compliant_legal_object() {
        let mut state = test_app_state();
        let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packs/systems")
            .canonicalize()
            .expect("packs/systems must exist relative to src/server");
        state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

        let result =
            get_system_manifest(AxumPath("basic-game-system".to_string()), State(state)).await;

        let Json(manifest) =
            result.expect("basic-game-system's real manifest must pass legal validation");
        assert_eq!(
            manifest["legal"]["licenseName"],
            "ThunderForgeVTT Original Content"
        );
        assert!(
            manifest["legal"]["trademarkRestrictions"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    /// Shared helper for the five research-digest-backed packs below —
    /// mirrors dnd5e/genie's own compliance tests but parameterized, since
    /// all five follow the identical assertion shape (real manifest,
    /// pointed at the actual packs/systems dir, checked against the
    /// licenseName recorded in the corresponding research digest).
    async fn assert_manifest_has_license(system_id: &str, expected_license_name: &str) {
        let mut state = test_app_state();
        let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packs/systems")
            .canonicalize()
            .expect("packs/systems must exist relative to src/server");
        state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

        let result = get_system_manifest(AxumPath(system_id.to_string()), State(state)).await;

        let Json(manifest) = result
            .unwrap_or_else(|_| panic!("{system_id}'s real manifest must pass legal validation"));
        assert_eq!(manifest["legal"]["licenseName"], expected_license_name);
    }

    /// Spec: research/system_pathfinder2e.json's `legal.licenseName`.
    #[tokio::test]
    async fn pathfinder2e_system_json_has_a_compliant_legal_object() {
        assert_manifest_has_license("pathfinder2e", "Open RPG Creative License (ORC)").await;
    }

    /// Spec: research/system_cypher_system.json's `legal.licenseName`.
    #[tokio::test]
    async fn cypher_system_json_has_a_compliant_legal_object() {
        assert_manifest_has_license("cypher_system", "Cypher System Open License").await;
    }

    /// Spec: research/system_fate_core.json's `legal.licenseName`.
    #[tokio::test]
    async fn fate_core_system_json_has_a_compliant_legal_object() {
        assert_manifest_has_license(
            "fate_core",
            "Creative Commons Attribution 3.0 Unported license",
        )
        .await;
    }

    /// Spec: research/system_blades_in_the_dark.json's `legal.licenseName`.
    #[tokio::test]
    async fn blades_in_the_dark_system_json_has_a_compliant_legal_object() {
        assert_manifest_has_license(
            "blades_in_the_dark",
            "Creative Commons Attribution 3.0 Unported (CC BY 3.0)",
        )
        .await;
    }

    /// Spec: research/system_year_zero_engine.json's `legal.licenseName`.
    #[tokio::test]
    async fn year_zero_engine_system_json_has_a_compliant_legal_object() {
        assert_manifest_has_license("year_zero_engine", "Year Zero Engine Free Tabletop License")
            .await;
    }
}
