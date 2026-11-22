//! Núcleo do suny's tf2 supplier.
//!
//! Este crate não conhece a janela (Tauri): tudo aqui roda e é testado sem interface.

pub mod analysis;
pub mod archive;
pub mod art;
pub mod casual;
pub mod commands;
pub mod contents;
pub mod error;
pub mod gamebanana;
pub mod gameicons;
pub mod installer;
pub mod library;
pub mod localmod;
pub mod logs;
pub mod replaces;
pub mod remotezip;
pub mod scanner;
pub mod steam;
pub mod system;
pub mod update;
pub mod vanilla;

pub use error::{Error, Result};

/// AppID do Team Fortress 2 no Steam.
pub const TF2_APP_ID: u32 = 440;
/// ID do Team Fortress 2 no GameBanana.
pub const TF2_GAMEBANANA_ID: u32 = 297;
