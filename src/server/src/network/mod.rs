//! Network layer for real-time communication.
//!
//! This module handles:
//! - PostgreSQL LISTEN background task (listener.rs)
//! - Axum WebSocket handlers (ws.rs)
//! - Event broadcasting to connected clients

pub mod listener;
pub mod ws;

pub use listener::spawn_listen_task;
pub use ws::websocket_handler;
