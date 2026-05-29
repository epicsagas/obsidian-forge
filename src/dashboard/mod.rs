pub mod commands;
pub mod models;
pub mod scoring;

use anyhow::Result;
use std::sync::{Arc, RwLock};

use crate::config::GlobalConfig;

use models::DashboardState;

pub struct AppState {
    pub cache: Arc<RwLock<Option<DashboardState>>>,
}

pub fn launch_dashboard() -> Result<()> {
    let global = GlobalConfig::load()?;
    let vaults = global.enabled_vaults();

    if vaults.is_empty() {
        anyhow::bail!("No vaults registered. Run `of vault add <path>` first.");
    }

    let state = AppState {
        cache: Arc::new(RwLock::new(None)),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_vaults,
            commands::get_dashboard,
            commands::open_in_obsidian,
        ])
        .run(tauri::generate_context!())
        .map_err(|e| anyhow::anyhow!("Tauri error: {}", e))?;

    Ok(())
}
