//! Generates the canvas's Tauri context. The app manifest lists the canvas's
//! own commands, so that Tauri checks them against the capabilities too;
//! without it, any page of the app could call every command.

use std::error::Error;

use tauri_build::{AppManifest, Attributes};

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(&["list_boards", "open_board"])),
    )?;
    Ok(())
}
