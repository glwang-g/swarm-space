//! Deterministic, renderer-independent simulation core for Swarm Space.
//!
//! The renderer-independent world kernel for Swarm Space. The public API is
//! owned by this crate and can be compiled for WASM or a headless server.
pub mod bots;

mod simulation;
pub use simulation::*;
