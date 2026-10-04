//! Network layer for real-time communication.
//!
//! This module handles:
//! - PostgreSQL LISTEN background task (listener.rs)
//! - Event broadcasting to connected clients

pub mod listener;

pub use listener::spawn_listen_task;
pub use listener::spawn_presence_listener_task;
