use crate::models::{GameSystem, NewGameSystem};
use crate::schema::game_systems;
use crate::state::AppState;
use crate::auth_middleware::AuthenticatedUser;
use axum::{
    extract::{Extension, Multipart, Path, State},
    http::{header, StatusCode},
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use diesel::prelude::*;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use pack_system_spec::{validate_system_manifest, SystemManifest};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_systems))
        .route("/:slug/manifest.json", get(get_system_manifest))
        .route("/:slug/download", get(download_system_package))
}

pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/install", post(install_game_system))
}

async fn list_systems(State(state): State<AppState>) -> Result<Json<Vec<GameSystem>>, (StatusCode, Json<serde_json::Value>)> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| {
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

    let manifest_content = fs::read_to_string(&system_path)
        .await
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND, // Or INTERNAL_SERVER_ERROR if file exists but can't be read
                Json(json!({"error": format!("Failed to read manifest file: {}", e)})),
            )
        })?;

    let manifest_json: serde_json::Value = serde_json::from_str(&manifest_content)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to parse manifest JSON: {}", e)})),
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
        let name = field.name().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Multipart field name missing"})),
            )
        })?.to_string();

        if name == "package" { // Assuming the file input field is named "package"
            let file_name = field.file_name().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "File name missing for package upload"})),
                )
            })?.to_string();

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
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "No package file found in multipart upload"}))));
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
                Json(json!({"error": format!("Attempted path traversal detected: {}", outpath.display())})),
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
                && !p.exists() {
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
                    Json(json!({"error": format!("Failed to create file during extraction: {}", e)})),
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
    let system_manifest: SystemManifest = serde_json::from_str(&manifest_content)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to parse SystemManifest after validation: {}", e)})),
            )
        })?;

    let system_slug = system_manifest.id.clone();
    let system_title = system_manifest.title.clone();
    let system_version = system_manifest.version.clone();

    // Ensure the system slug is valid (alphanumeric, -, _, .)
    if !system_slug.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Invalid system slug: {}. Slug must be alphanumeric, hyphen, underscore, or dot", system_slug)})),
        ));
    }

    // Clone pool + slug for first closure
    let pool_for_check = state.db_pool.clone();
    let slug_for_check = system_slug.clone();

    let existing_system_count = tokio::task::spawn_blocking(move || {
        let mut conn = pool_for_check
            .get()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get DB connection"}))))?;
        game_systems::table
            .filter(game_systems::slug.eq(&slug_for_check))
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to query existing systems: {}", e)}))))
    }).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to spawn blocking task: {}", e)}))))??; // Propagate the error from the blocking task

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

    Ok(Json(json!({"message": format!("Game system '{}' (v{}) installed successfully!", system_title, system_version)})))
}