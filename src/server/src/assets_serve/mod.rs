//! Authenticated asset byte routes.
//!
//! Each submodule owns one entity's `GET` routes for the bytes it stored:
//! canvas assets, scene previews, lore images and actor portraits. Every one
//! of them looks the row up first and authorises the caller against it before
//! streaming anything.
//!
//! That is the whole reason this is a directory and not four flat files
//! sitting beside [`crate::static_files`]. Both once had `serve` in the name
//! and they have opposite trust models: `static_files` is a pair of
//! unauthenticated `ServeDir` mounts, and everything here is a permission
//! check that happens to end in bytes.

pub mod actor;
pub mod canvas;
pub mod lore;
pub mod scene;
