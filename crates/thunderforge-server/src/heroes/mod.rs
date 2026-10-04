//! Spec 044 phase (d): what the server knows about hero specs.
//!
//! Very little, on purpose. The catalogue — parts, choices, colours — is code
//! in `packages/heroes` and ships with the client. The server stores the spec
//! that drew an actor's image beside that image (ADR-106) and checks it at the
//! door against the JSON Schema the package emits (contract B7); it never
//! draws one.

pub mod spec_schema;

#[cfg(test)]
mod spec_schema_tests;
