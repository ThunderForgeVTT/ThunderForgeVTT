//! Mapforge: a standalone map-asset service.
//!
//! # Why this is not part of Crucible
//!
//! Crucible (ADR-047) is **session adjudication** — the server deciding
//! whether a proposed move is legal, for anti-cheat and as the prerequisite
//! for elastic per-session scaling. Serving map imagery has nothing to do with
//! that, and folding it in would dissolve the single responsibility that ADR
//! justifies. What is borrowed from Crucible is its *shape*: a library holding
//! all routing so tests can drive the router in-process, plus a thin binary
//! that only reads a port and serves.
//!
//! # Why this is not the main server either
//!
//! The real asset path is authenticated, world-scoped and backed by RustFS.
//! That is correct for production and hostile to experimentation: to see
//! whether a tiling scheme works you would need a session, a world, a
//! membership row and an S3 bucket. This service deliberately has **no auth
//! and no object store** — it reads `examples/maps/*.dd2vtt` straight off disk
//! and treats that directory as its content store. It exists so texture
//! ceilings, pyramid depth and tile sizes can be hammered on against a real
//! HTTP backend, over a real network path, with real bytes.
//!
//! What it shares with production is the part that matters: the same tiling
//! geometry the engine will consume, and the same texture-dimension ceiling
//! reasoning. What it omits is everything that makes production slow to poke
//! at.

pub mod server;
pub mod source;
pub mod tiles;
