use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{fs, path::Path, sync::mpsc::channel};
use tracing::{error, info, warn};

use crate::{config::ForgeConfig, converter, notes};

pub async fn watch_inbox(vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    let inbox = vault_root.join(&config.vault.inbox_dir);
    if !inbox.exists() {
        warn!("Inbox not found. Creating: {}", inbox.display());
        fs::create_dir_all(&inbox)?;
    }
    info!("Watching inbox: {}", inbox.display());

    initial_scan(&inbox, vault_root, config).await?;

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(&inbox, RecursiveMode::NonRecursive)?;

    // Bridge std::sync::mpsc → tokio::sync::mpsc so we don't block the async runtime.
    let (async_tx, mut async_rx) = tokio::sync::mpsc::unbounded_channel::<notify::Result<Event>>();
    std::thread::spawn(move || {
        loop {
            match rx.recv() {
                Ok(event) => {
                    if async_tx.send(event).is_err() { break; }
                }
                Err(_) => break,
            }
        }
    });

    // Keep watcher alive for the lifetime of this function.
    let _watcher = watcher;

    while let Some(event) = async_rx.recv().await {
        match event {
            Ok(Event { kind, paths, .. }) => {
                if matches!(kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    // Process each file concurrently by spawning tokio tasks
                    for p in paths {
                        let p = p.clone();
                        let vault_root = vault_root.to_path_buf();
                        let config = config.clone();
                        tokio::spawn(async move {
                            handle_file_event(&p, &vault_root, &config).await;
                        });
                    }
                }
            }
            Err(e) => error!("Watcher error: {:?}", e),
        }
    }

    Ok(())
}

async fn handle_file_event(p: &Path, vault_root: &Path, config: &ForgeConfig) {
    if notes::is_pdf(p) {
        match converter::convert_pdf_to_md(p, vault_root, config).await {
            Ok(md_path) => {
                info!("PDF converted -> {}", md_path.display());
                if let Err(e) = notes::process_one(&md_path, config, vault_root).await {
                    if !e.to_string().contains("No such file or directory") {
                        error!("Processing failed: {:?}", e);
                    }
                }
            }
            Err(e) => error!("PDF conversion failed: {:?}", e),
        }
    } else if notes::is_markdown(p) {
        if let Err(e) = notes::process_one(p, config, vault_root).await {
            if !e.to_string().contains("No such file or directory") {
                error!("Processing failed: {:?}", e);
            }
        }
    }
}

async fn initial_scan(inbox: &Path, vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    info!("Initial scan: {}", inbox.display());
    for entry in fs::read_dir(inbox)? {
        let path = entry?.path();
        handle_file_event(&path, vault_root, config).await;
    }
    Ok(())
}
