use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tokio::process::Command as TokioCommand;
use tracing::{info, warn};

use crate::config::ForgeConfig;

/// Convert a PDF to Markdown. Tries marker_single, falls back to pdftotext.
/// This is an async function to avoid blocking the async runtime.
pub async fn convert_pdf_to_md(
    pdf_path: &Path,
    vault_root: &Path,
    config: &ForgeConfig,
) -> Result<PathBuf> {
    let stem = pdf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect::<String>();

    let stem = if stem.is_empty() {
        "unknown".to_string()
    } else {
        stem.replace("..", "").replace('/', "").replace('\\', "")
    };

    let inbox = vault_root.join(&config.vault.inbox_dir);
    let archive = vault_root
        .join(&config.vault.archive_dir)
        .join("PDF-Archive");

    // Try marker_single
    if is_command_available("marker_single").await {
        info!("Using marker_single for PDF -> MD: {}", pdf_path.display());
        let status = TokioCommand::new("marker_single")
            .arg("--output_dir")
            .arg(&inbox)
            .arg(pdf_path)
            .status()
            .await
            .context("spawn marker_single")?;

        if status.success() {
            let generated_folder = inbox.join(&stem);
            let generated_md = generated_folder.join(format!("{}.md", &stem));

            if generated_folder.exists() && generated_md.exists() {
                let temp_folder = vault_root.join("temp_conversions").join(&stem);
                fs::create_dir_all(temp_folder.parent().unwrap()).ok();
                if temp_folder.exists() {
                    fs::remove_dir_all(&temp_folder)?;
                }
                fs::rename(&generated_folder, &temp_folder)?;
                archive_pdf(pdf_path, &archive);
                return Ok(temp_folder.join(format!("{}.md", &stem)));
            }
        }
        warn!("marker_single failed for {}", pdf_path.display());
    }

    // Fallback to pdftotext
    if is_command_available("pdftotext").await {
        info!("Using pdftotext for PDF -> MD: {}", pdf_path.display());
        let output_path = inbox.join(format!("{}.md", &stem));
        let status = TokioCommand::new("pdftotext")
            .arg("-layout")
            .arg(pdf_path)
            .arg(&output_path)
            .status()
            .await
            .context("spawn pdftotext")?;

        if status.success() {
            archive_pdf(pdf_path, &archive);
            return Ok(output_path);
        }
        bail!("pdftotext failed with status: {}", status);
    }

    bail!("Neither marker_single nor pdftotext available. Install one of them.");
}

fn archive_pdf(pdf_path: &Path, archive: &Path) {
    if !pdf_path.exists() {
        return;
    }
    fs::create_dir_all(archive).ok();
    let file_name = match pdf_path.file_name() {
        Some(name) => name,
        None => return,
    };
    let dest = archive.join(file_name);
    match fs::rename(pdf_path, &dest) {
        Ok(_) => info!("PDF archived: {}", dest.display()),
        Err(e) => warn!("Failed to archive PDF: {:?}", e),
    }
}

async fn is_command_available(cmd: &str) -> bool {
    TokioCommand::new("which")
        .arg(cmd)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}
