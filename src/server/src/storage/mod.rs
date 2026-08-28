//! Spec 002: canvas image asset object storage (RustFS), the single
//! mechanism backing both paste-to-canvas images and migrated map-import
//! backgrounds (FR-018). See `docs/adrs/20260820-039-*.md`.

pub mod backfill;
pub mod rustfs;
pub mod transcode;
