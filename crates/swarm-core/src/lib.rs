//! Deterministic, renderer-independent simulation core for Swarm Space.
//!
//! This transitional include keeps the core implementation in one source file
//! while the Bevy viewer is migrated away from it. The public API is now owned
//! by this crate and can later be compiled for WASM or a headless server.
include!("../../../src/simulation.rs");
