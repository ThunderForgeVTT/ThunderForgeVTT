use crate::auth_middleware::AuthenticatedUser;
use crate::models::{GameSystem, NewGameSystem};
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

async fn list_systems(
    State(state): State<AppState>,
) -> Result<Json<Vec<GameSystem>>, (StatusCode, Json<serde_json::Value>)> {
    let mut conn = state.db_pool.get().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get DB connection: {}", e)})),
        )
    })?;

    let systems = tokio::task::spawn_blocking(move || {
        game_systems::table
            .order(game_systems::title.asc())
            .select(GameSystem::as_select())
            .load::<GameSystem>(&mut conn)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to spawn blocking task: {}", e)})),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to query game systems: {}", e)})),
        )
    })?;

    Ok(Json(systems))
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
// D&D 5e System Registration
// ============================================================================

/// Initialize D&D 5e validators in the registry
pub fn register_dnd5e_system(registry: &mut GameSystemRegistry) {
    use dnd5e_server::{
        validate_ability_data_for_registry, validate_proficiency_data_for_registry,
        validate_resource_data_for_registry, validate_spell_data_for_registry,
        validate_trait_data_for_registry,
    };

    registry.register(
        "dnd5e",
        SystemValidators {
            ability_data: Some(validate_ability_data_for_registry),
            resource_data: Some(validate_resource_data_for_registry),
            proficiency_data: Some(validate_proficiency_data_for_registry),
            trait_data: Some(validate_trait_data_for_registry),
            spell_data: Some(validate_spell_data_for_registry),
        },
    );
}

// ============================================================================
// Genie System Registration
// ============================================================================

/// Initialize Genie validators in the registry.
///
/// Genie has no spell_data slot (it has no spellcasting) and reuses the
/// registry's trait_data slot for conditions/Patron/size_category
/// (spec 018 data-model.md — see genie-server's validators.rs doc comments).
pub fn register_genie_system(registry: &mut GameSystemRegistry) {
    use genie_server::{
        validate_ability_data_for_registry, validate_proficiency_data_for_registry,
        validate_resource_data_for_registry, validate_trait_data_for_registry,
    };

    registry.register(
        "genie",
        SystemValidators {
            ability_data: Some(validate_ability_data_for_registry),
            resource_data: Some(validate_resource_data_for_registry),
            proficiency_data: Some(validate_proficiency_data_for_registry),
            trait_data: Some(validate_trait_data_for_registry),
            spell_data: None,
        },
    );
}

// ============================================================================
// Pathfinder 2e / Cypher System / Fate Core / Blades in the Dark / Year Zero
// Engine System Registration
// ============================================================================
//
// All five follow register_genie_system's exact pattern: no spell_data slot
// (none of the five research digests found a spellcasting-specific data
// shape distinct from generic resource_data), full ability/resource/
// proficiency/trait_data validation from each pack's own validators.rs.

pub fn register_pathfinder2e_system(registry: &mut GameSystemRegistry) {
    use pathfinder2e_server::{
        validate_ability_data_for_registry, validate_proficiency_data_for_registry,
        validate_resource_data_for_registry, validate_trait_data_for_registry,
    };
    registry.register(
        "pathfinder2e",
        SystemValidators {
            ability_data: Some(validate_ability_data_for_registry),
            resource_data: Some(validate_resource_data_for_registry),
            proficiency_data: Some(validate_proficiency_data_for_registry),
            trait_data: Some(validate_trait_data_for_registry),
            spell_data: None,
        },
    );
}

pub fn register_cypher_system(registry: &mut GameSystemRegistry) {
    use cypher_server::{
        validate_ability_data_for_registry, validate_proficiency_data_for_registry,
        validate_resource_data_for_registry, validate_trait_data_for_registry,
    };
    registry.register(
        "cypher_system",
        SystemValidators {
            ability_data: Some(validate_ability_data_for_registry),
            resource_data: Some(validate_resource_data_for_registry),
            proficiency_data: Some(validate_proficiency_data_for_registry),
            trait_data: Some(validate_trait_data_for_registry),
            spell_data: None,
        },
    );
}

pub fn register_fate_core_system(registry: &mut GameSystemRegistry) {
    use fate_server::{
        validate_ability_data_for_registry, validate_proficiency_data_for_registry,
        validate_resource_data_for_registry, validate_trait_data_for_registry,
    };
    registry.register(
        "fate_core",
        SystemValidators {
            // Fate has no fixed ability scores (research.md); ability_data
            // still validates (accepts any object, per fate-server's
            // validators.rs) so an empty ability_data block is always valid.
            ability_data: Some(validate_ability_data_for_registry),
            resource_data: Some(validate_resource_data_for_registry),
            proficiency_data: Some(validate_proficiency_data_for_registry),
            trait_data: Some(validate_trait_data_for_registry),
            spell_data: None,
        },
    );
}

pub fn register_blades_in_the_dark_system(registry: &mut GameSystemRegistry) {
    use blades_server::{
        validate_ability_data_for_registry, validate_proficiency_data_for_registry,
        validate_resource_data_for_registry, validate_trait_data_for_registry,
    };
    registry.register(
        "blades_in_the_dark",
        SystemValidators {
            ability_data: Some(validate_ability_data_for_registry),
            resource_data: Some(validate_resource_data_for_registry),
            proficiency_data: Some(validate_proficiency_data_for_registry),
            trait_data: Some(validate_trait_data_for_registry),
            spell_data: None,
        },
    );
}

pub fn register_year_zero_engine_system(registry: &mut GameSystemRegistry) {
    use yze_server::{
        validate_ability_data_for_registry, validate_proficiency_data_for_registry,
        validate_resource_data_for_registry, validate_trait_data_for_registry,
    };
    registry.register(
        "year_zero_engine",
        SystemValidators {
            ability_data: Some(validate_ability_data_for_registry),
            resource_data: Some(validate_resource_data_for_registry),
            proficiency_data: Some(validate_proficiency_data_for_registry),
            trait_data: Some(validate_trait_data_for_registry),
            spell_data: None,
        },
    );
}

// ============================================================================
// Lazy-Loaded Global Registry (Singleton Pattern)
// ============================================================================

use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Global registry instance - initialized once, accessed many times
pub static GAME_SYSTEMS: Lazy<Mutex<GameSystemRegistry>> = Lazy::new(|| {
    let mut registry = GameSystemRegistry::new();

    // 🎮 Register all available systems on first access
    register_dnd5e_system(&mut registry);
    register_genie_system(&mut registry);
    register_pathfinder2e_system(&mut registry);
    register_cypher_system(&mut registry);
    register_fate_core_system(&mut registry);
    register_blades_in_the_dark_system(&mut registry);
    register_year_zero_engine_system(&mut registry);
    // In future phases: register_coc7e_system(&mut registry);

    Mutex::new(registry)
});

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
