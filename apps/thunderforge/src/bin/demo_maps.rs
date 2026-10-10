//! Turns the example maps into the files the demo serves, with no database
//! and no server.
//!
//!     cargo run -q -p thunderforge --bin thunderforge-demo-maps \
//!         --features demo-maps -- examples/maps apps/demo/public/maps
//!
//! # Why this exists
//!
//! Spec 074's demo is a world that lives in a browser tab, and its scenes are
//! the maps in `examples/maps`. Each has to arrive as a scene would hold it:
//! a background inside the texture cap, a grid that still fits that
//! background, and walls, doors and lights placed on it. The importer already
//! knows how, so this runs the importer (`map_import::import_offline`) and
//! writes down what it would have stored.
//!
//! For every `<name>.dd2vtt` it writes `<name>.webp`, `<name>.thumb.webp`
//! when a thumbnail could be made, and one `maps.json` listing every map's
//! size, grid, walls and lights in the field names the GraphQL API uses.
//! When the source holds a `credit.json`, it is copied across with a
//! `NOTICE.txt` beside it, so the maps never travel without their credit.
//!
//! `synthetic-*` files are left out: they are parser fixtures written by
//! hand, not maps.
//!
//! # Why a binary of its own, behind a feature
//!
//! The same reason as `thunderforge-schema`: the builds that run the server
//! should not link an executable none of them run.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(source), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: thunderforge-demo-maps <maps directory> <output directory>");
        return ExitCode::from(2);
    };
    match run(Path::new(&source), Path::new(&out)) {
        Ok(count) => {
            eprintln!("[demo-maps] {count} maps in {out}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("[demo-maps] {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(source: &Path, out: &Path) -> Result<usize, String> {
    std::fs::create_dir_all(out).map_err(|e| at(out, &e))?;

    // Sorted, so the same maps always produce the same `maps.json`.
    let mut files: Vec<PathBuf> = std::fs::read_dir(source)
        .map_err(|e| at(source, &e))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "dd2vtt"))
        .filter(|path| !name_of(path).starts_with("synthetic-"))
        .collect();
    files.sort();

    let mut maps = Vec::new();
    for path in &files {
        let name = name_of(path);
        let raw = std::fs::read(path).map_err(|e| at(path, &e))?;
        let map =
            thunderforge_server::map_import::import_offline(&raw).map_err(|e| at(path, &e))?;

        write(&out.join(format!("{name}.webp")), &map.background_webp)?;
        if let Some(preview) = &map.preview_webp {
            write(&out.join(format!("{name}.thumb.webp")), preview)?;
        }

        let mut entry = map.geometry_json();
        entry["name"] = name.clone().into();
        entry["hasPreview"] = map.preview_webp.is_some().into();
        entry["byteSize"] = map.background_webp.len().into();
        maps.push(entry);
        eprintln!(
            "[demo-maps] {name}: {}x{} at {}px, {} walls, {} lights",
            map.width,
            map.height,
            map.grid_size,
            map.walls.len(),
            map.lights.len()
        );
    }

    let listing = serde_json::Value::Array(maps).to_string();
    write(&out.join("maps.json"), listing.as_bytes())?;

    let credit_path = source.join("credit.json");
    if credit_path.exists() {
        let raw = std::fs::read(&credit_path).map_err(|e| at(&credit_path, &e))?;
        let credit: serde_json::Value =
            serde_json::from_slice(&raw).map_err(|e| at(&credit_path, &e))?;
        write(&out.join("credit.json"), &raw)?;
        write(&out.join("NOTICE.txt"), notice(&credit).as_bytes())?;
    }
    Ok(files.len())
}

/// The same notice the demo's `prepare-static.mjs` writes.
fn notice(credit: &serde_json::Value) -> String {
    let field = |name: &str| credit[name].as_str().unwrap_or_default().to_owned();
    let licence = field("licence");
    [
        format!("The maps in this directory are by {},", field("author")),
        format!("licensed under {licence} ({}).", field("licenceUrl")),
        format!("Source: {}", field("source")),
        format!("More of them: {}", field("catalog")),
        String::new(),
        "They were resized and re-encoded to fit a scene; nothing else was changed.".to_owned(),
        format!("These copies are offered under the same licence, {licence}."),
        String::new(),
    ]
    .join("\n")
}

fn name_of(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| at(path, &e))
}

fn at(path: &Path, error: &dyn std::fmt::Display) -> String {
    format!("{}: {error}", path.display())
}
