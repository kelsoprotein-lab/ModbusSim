//! Shared egui widgets and helpers for ModbusSlave / ModbusMaster applications.
//!
//! This crate will host the virtual register table, log panel, dialogs,
//! value formatting helpers and any other UI primitives reused by both
//! front-ends. For S0 it only exposes the placeholder module layout.

pub mod fonts;
pub mod format;
pub mod hero_anim;
pub mod log_panel;
pub mod project;
pub mod theme;
pub mod ui;
pub mod value_panel;

/// Phosphor icon constants (re-export for convenience).
pub use egui_phosphor::regular as icons;
