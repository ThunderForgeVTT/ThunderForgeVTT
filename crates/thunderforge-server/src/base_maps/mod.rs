//! Spec 088 (US2): the maps a new world can open on.
//!
//! The base maps are files in the server image, not rows. They are the
//! example maps in `examples/maps`, already run through the importer by
//! `thunderforge-demo-maps` (see `specs/088-first-session-feedback/contracts/
//! base-maps.md`):
//!
//! ```text
//! <dir>/maps.json          every map's size, grid, walls and lights
//! <dir>/credit.json        whose they are, and under what licence
//! <dir>/NOTICE.txt         the licence notice
//! <dir>/<id>.webp          the background
//! <dir>/<id>.thumb.webp    its thumbnail
//! ```
//!
//! The directory is read once, at start-up. Nothing about it can stop the
//! server: a missing directory, an unreadable listing or a missing credit
//! is one `warn` and an empty set, and the create-world form then offers
//! only **None** (FR-022). The credit is required, not decorative: the maps
//! are CC BY-SA 4.0, so a set that cannot name its author is not offered.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::map_import::{LightInsert, WallInsert};

pub mod apply;
pub mod graphql;
pub mod routes;

/// The map a new world opens on when the form's choice is left alone
/// (Open item 1).
pub const DEFAULT_BASE_MAP_ID: &str = "grassy-path-ambush";

/// `THUNDERFORGE_BASE_MAPS_DEFAULT=none`: no map unless one is asked for.
pub const NO_DEFAULT: &str = "none";

/// What every place that shows a base map says about ShareAlike (FR-030).
pub const SHARE_ALIKE: &str = "The copies here are offered under the same licence.";

/// Whose the maps are, read from `credit.json` (FR-028).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapCredit {
    pub author: String,
    pub licence: String,
    pub licence_url: String,
    pub source: String,
    pub catalog: String,
}

impl MapCredit {
    /// `examples/maps/credit.json`, as the server was built with it. The
    /// same file the directory carries; this copy is what a scene's credit
    /// falls back to when the directory is gone.
    pub fn bundled() -> MapCredit {
        serde_json::from_str(include_str!("../../../../examples/maps/credit.json"))
            .expect("examples/maps/credit.json is a MapCredit")
    }
}

/// One map, as `maps.json` lists it.
#[derive(Debug, Clone)]
pub struct BaseMap {
    /// The file stem: `grassy-path-ambush`.
    pub id: String,
    /// The stem, title-cased: `Grassy Path Ambush`.
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub grid_size: u32,
    pub ambient_light: String,
    /// Walls and doors, in the order an import inserts them.
    pub walls: Vec<WallInsert>,
    pub lights: Vec<LightInsert>,
    /// Whether `<id>.thumb.webp` is on disk.
    pub has_thumbnail: bool,
}

impl BaseMap {
    pub fn thumbnail_url(&self) -> String {
        format!("/api/base-maps/{}.thumb.webp", self.id)
    }
}

/// The loaded directory. `Default` is the empty set.
#[derive(Debug, Clone, Default)]
pub struct BaseMaps {
    dir: PathBuf,
    maps: Vec<BaseMap>,
    credit: Option<MapCredit>,
    /// The operator's choice of default, when one was made: a map's id, or
    /// `none`. Unset means [`DEFAULT_BASE_MAP_ID`].
    default_choice: Option<String>,
}

/// Which of a map's two pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picture {
    Full,
    Thumbnail,
}

impl BaseMaps {
    /// Reads the directory, or answers the empty set with one `warn` saying
    /// why. `None` (no directory configured) is the empty set, silently.
    pub fn load(dir: Option<&Path>) -> BaseMaps {
        let Some(dir) = dir else {
            return BaseMaps::default();
        };
        match read_dir(dir) {
            Ok(maps) => maps,
            Err(reason) => {
                tracing::warn!(
                    base_maps_dir = %dir.display(),
                    reason = %reason,
                    "no base maps: a new world can only start blank"
                );
                BaseMaps::default()
            }
        }
    }

    /// Every map, in the listing's order.
    pub fn list(&self) -> &[BaseMap] {
        &self.maps
    }

    pub fn get(&self, id: &str) -> Option<&BaseMap> {
        self.maps.iter().find(|map| map.id == id)
    }

    /// Which map a world asked for without naming one opens on: a map's id,
    /// or `none` for a blank scene. `None` keeps [`DEFAULT_BASE_MAP_ID`].
    ///
    /// The e2e harness sets `none`, so the hundreds of worlds its specs make
    /// keep the blank Starting Scene they were written against, while the
    /// maps themselves are still offered and can still be chosen.
    pub fn with_default(mut self, choice: Option<&str>) -> BaseMaps {
        self.default_choice = choice.map(str::trim).map(str::to_owned);
        if let Some(id) = self.default_choice.as_deref() {
            if id != NO_DEFAULT && self.get(id).is_none() {
                tracing::warn!(
                    base_map = %id,
                    "the configured default base map is not in the set; new worlds start blank"
                );
            }
        }
        self
    }

    /// The default map's id, when it is in the set.
    pub fn default_id(&self) -> Option<&str> {
        let id = match self.default_choice.as_deref() {
            None => DEFAULT_BASE_MAP_ID,
            Some(NO_DEFAULT) | Some("") => return None,
            Some(id) => id,
        };
        self.get(id).map(|map| map.id.as_str())
    }

    /// The credit. Present whenever the set is not empty.
    pub fn credit(&self) -> Option<&MapCredit> {
        self.credit.as_ref()
    }

    /// Where one of a known map's pictures is. Only an id from the loaded
    /// set reaches the filesystem, so a request cannot name a path.
    pub fn picture_path(&self, id: &str, picture: Picture) -> Option<PathBuf> {
        let map = self.get(id)?;
        match picture {
            Picture::Full => Some(self.dir.join(format!("{}.webp", map.id))),
            Picture::Thumbnail if map.has_thumbnail => {
                Some(self.dir.join(format!("{}.thumb.webp", map.id)))
            }
            Picture::Thumbnail => None,
        }
    }

    /// `NOTICE.txt`, when the set is not empty.
    pub fn notice_path(&self) -> Option<PathBuf> {
        (!self.maps.is_empty()).then(|| self.dir.join("NOTICE.txt"))
    }

    /// The set a test needs, without a directory.
    #[cfg(test)]
    pub fn for_tests(dir: PathBuf, maps: Vec<BaseMap>, credit: MapCredit) -> BaseMaps {
        BaseMaps {
            dir,
            maps,
            credit: Some(credit),
            default_choice: None,
        }
    }
}

/// `grassy-path-ambush` → `Grassy Path Ambush`.
pub fn title_case(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedMap {
    name: String,
    width: u32,
    height: u32,
    grid_size: u32,
    #[serde(default = "bright")]
    ambient_light: String,
    #[serde(default)]
    walls: Vec<ListedWall>,
    #[serde(default)]
    lights: Vec<ListedLight>,
}

fn bright() -> String {
    "bright".to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedWall {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    blocks_vision: bool,
    blocks_movement: bool,
    door_state: String,
    /// Spec 088 FR-094: an edge wall `import_offline` added.
    #[serde(default)]
    perimeter: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedLight {
    x: f64,
    y: f64,
    radius: f64,
    intensity: f64,
    color: String,
    casts_shadows: bool,
}

/// The wall table's door states, as the `'static` strings `WallInsert`
/// carries.
fn door_state(value: &str) -> Option<&'static str> {
    match value {
        "none" => Some("none"),
        "closed" => Some("closed"),
        "open" => Some("open"),
        "locked" => Some("locked"),
        _ => None,
    }
}

fn read_dir(dir: &Path) -> Result<BaseMaps, String> {
    let listing_path = dir.join("maps.json");
    let listing =
        std::fs::read(&listing_path).map_err(|e| format!("{}: {e}", listing_path.display()))?;
    let listed: Vec<ListedMap> =
        serde_json::from_slice(&listing).map_err(|e| format!("{}: {e}", listing_path.display()))?;

    let credit_path = dir.join("credit.json");
    let credit: MapCredit = std::fs::read(&credit_path)
        .map_err(|e| format!("{}: {e}", credit_path.display()))
        .and_then(|bytes| {
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", credit_path.display()))
        })?;

    let mut seen = BTreeMap::new();
    let mut maps = Vec::new();
    for entry in listed {
        let id = entry.name;
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            tracing::warn!(base_map = %id, "base map left out: its name is not a plain id");
            continue;
        }
        if !dir.join(format!("{id}.webp")).is_file() {
            tracing::warn!(base_map = %id, "base map left out: its image is missing");
            continue;
        }
        if seen.insert(id.clone(), ()).is_some() {
            tracing::warn!(base_map = %id, "base map left out: listed twice");
            continue;
        }
        let mut walls = Vec::with_capacity(entry.walls.len());
        let mut bad_wall = false;
        for wall in entry.walls {
            let Some(state) = door_state(&wall.door_state) else {
                bad_wall = true;
                break;
            };
            walls.push(WallInsert {
                x1: wall.x1,
                y1: wall.y1,
                x2: wall.x2,
                y2: wall.y2,
                blocks_vision: wall.blocks_vision,
                blocks_movement: wall.blocks_movement,
                door_state: state,
                perimeter: wall.perimeter,
            });
        }
        if bad_wall {
            tracing::warn!(base_map = %id, "base map left out: a wall has an unknown door state");
            continue;
        }
        let lights = entry
            .lights
            .into_iter()
            .map(|light| LightInsert {
                x: light.x,
                y: light.y,
                radius: light.radius,
                intensity: light.intensity,
                color: light.color,
                casts_shadows: light.casts_shadows,
            })
            .collect();
        maps.push(BaseMap {
            name: title_case(&id),
            has_thumbnail: dir.join(format!("{id}.thumb.webp")).is_file(),
            id,
            width: entry.width,
            height: entry.height,
            grid_size: entry.grid_size,
            ambient_light: entry.ambient_light,
            walls,
            lights,
        });
    }

    if maps.is_empty() {
        return Err("maps.json lists no map whose image is present".to_string());
    }
    Ok(BaseMaps {
        dir: dir.to_path_buf(),
        maps,
        credit: Some(credit),
        default_choice: None,
    })
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "create_world_tests.rs"]
mod create_world_tests;
