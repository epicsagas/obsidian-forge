use anyhow::{bail, Context, Result};
use futures::stream::{self, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::ai::AiClient;
use crate::config::ForgeConfig;
use crate::prompts::load_prompts;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Frontmatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcategory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub questions: Option<Vec<String>>,
}

pub fn is_markdown(path: &Path) -> bool {
    path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false)
}

pub fn is_pdf(path: &Path) -> bool {
    path.is_file() && path.extension().map(|e| e == "pdf").unwrap_or(false)
}

pub async fn process_all(vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    let inbox = vault_root.join(&config.vault.inbox_dir);
    if !inbox.exists() {
        bail!("Inbox not found at '{}'.", inbox.display());
    }

    info!("process_all started, inbox={}", inbox.display());

    // Convert PDFs (async)
    let pdf_results: Vec<_> = (fs::read_dir(&inbox)?)
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if is_pdf(&path) {
                Some(path)
            } else {
                None
            }
        })
        .map(|path| async move {
            match crate::converter::convert_pdf_to_md(&path, vault_root, config).await {
                Ok(md_path) => info!("PDF converted: {} -> {}", path.display(), md_path.display()),
                Err(e) => warn!("PDF conversion failed: {}: {:?}", path.display(), e),
            }
        })
        .collect::<Vec<_>>();

    // Run all PDF conversions concurrently
    futures::future::join_all(pdf_results).await;

    // Collect all markdown files to process
    let mut md_files: Vec<PathBuf> = Vec::new();

    // Scan inbox
    for entry in fs::read_dir(&inbox)?.flatten() {
        let path = entry.path();
        if is_markdown(&path) {
            md_files.push(path);
        }
    }

    // Scan temp_conversions
    let temp_folder = vault_root.join("temp_conversions");
    if temp_folder.exists() {
        for entry in WalkDir::new(&temp_folder)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path().to_path_buf();
            if is_markdown(&path) {
                md_files.push(path);
            }
        }
    }

    info!("Processing {} markdown files concurrently", md_files.len());

    // Process files concurrently with buffer_unordered
    let concurrency_limit = config.ai.max_concurrent.unwrap_or(5);
    stream::iter(md_files)
        .map(|path| {
            let path = path.clone();
            async move { process_one(&path, config, vault_root).await }
        })
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<_>>()
        .await;

    Ok(())
}

pub async fn process_one(path: &Path, config: &ForgeConfig, vault_root: &Path) -> Result<()> {
    info!("Processing: {}", path.display());

    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("read {}", path.display()))?;
    let (fm, body) = split_frontmatter(&content)?;

    if fm.as_ref().and_then(|f| f.status.as_deref()) == Some("processed") {
        debug!("Already processed, skipping: {}", path.display());
        return Ok(());
    }

    let ollama = AiClient::from_config(&config.ai);
    let prompts = load_prompts();

    // Prepare prompts
    let q_prompt = prompts
        .questions_template
        .replace("{count}", "3")
        .replace("{content}", &body);
    let t_prompt = prompts
        .tags_template
        .replace("{min_tags}", "3")
        .replace("{max_tags}", "5")
        .replace("{existing_tags}", "[]")
        .replace("{content}", &body);

    // Execute independent AI calls in parallel using tokio::join!
    let (summary, questions, gen_tags) = tokio::join!(
        ollama.summarize(&body, 200),
        ollama.generate_json::<Vec<String>>(&q_prompt),
        ollama.generate_json::<Vec<String>>(&t_prompt)
    );

    let summary = summary.unwrap_or_default();
    let questions = questions.unwrap_or_default();
    let gen_tags = gen_tags.unwrap_or_else(|e| {
        warn!("Tag generation failed: {:?}", e);
        vec![]
    });

    let title = body
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").to_string())
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });

    let (category, subcategory, detail) =
        classify_by_title_or_ai(&title, &body, &ollama, &prompts).await;
    info!(
        "Classification: {} / {} / {}",
        category, subcategory, detail
    );

    let mut new_fm = fm.unwrap_or_default();
    new_fm.status = Some("processed".to_string());
    new_fm.summary = Some(summary);
    new_fm.questions = Some(questions);
    new_fm.category = Some(category.clone());
    new_fm.subcategory = Some(subcategory.clone());
    new_fm.detail = Some(detail.clone());
    new_fm.tags = Some(merge_vec(new_fm.tags.take().unwrap_or_default(), gen_tags));
    new_fm.processed_at = Some(iso_now());

    let updated = join_frontmatter(&new_fm, &body);
    tokio::fs::write(path, &updated)
        .await
        .with_context(|| format!("write {}", path.display()))?;

    move_to_para(path, &category, &subcategory, &detail, config, vault_root)?;

    info!("Done: {}", path.display());
    Ok(())
}

async fn classify_by_title_or_ai(
    title: &str,
    body: &str,
    ollama: &AiClient,
    prompts: &crate::prompts::LoadedPrompts,
) -> (String, String, String) {
    let title_lower = title.to_lowercase();
    let how_to = ["how to", "how-to", "guide", "setup", "install", "configure"];
    let research = ["paper", "research", "study", "survey", "analysis"];

    if how_to.iter().any(|kw| title_lower.contains(kw)) {
        return (
            "Resources".into(),
            "Reference".into(),
            "Tutorials-Guides".into(),
        );
    }
    if research.iter().any(|kw| title_lower.contains(kw)) {
        return (
            "Resources".into(),
            "Reference".into(),
            "Articles-Papers".into(),
        );
    }

    #[derive(serde::Deserialize, Default)]
    struct Cat {
        category: Option<String>,
        subcategory: Option<String>,
        detail: Option<String>,
    }
    let c_prompt = prompts.category_template.replace("{content}", body);
    let cat: Cat = ollama.generate_json(&c_prompt).await.unwrap_or_default();
    (
        cat.category.unwrap_or_else(|| "Resources".into()),
        cat.subcategory.unwrap_or_else(|| "Reference".into()),
        cat.detail.unwrap_or_else(|| "Articles-Papers".into()),
    )
}

fn move_to_para(
    path: &Path,
    category: &str,
    subcategory: &str,
    detail: &str,
    config: &ForgeConfig,
    vault_root: &Path,
) -> Result<()> {
    let dest_dir = resolve_dest_dir(vault_root, category, subcategory, detail, config);
    fs::create_dir_all(&dest_dir)?;

    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("invalid file path: {}", path.display()))?;
    let dest = dest_dir.join(file_name);
    if path != dest {
        fs::rename(path, &dest)?;
        info!("File moved: {} -> {}", path.display(), dest.display());
    }
    Ok(())
}

fn resolve_dest_dir(
    vault_root: &Path,
    category: &str,
    subcategory: &str,
    detail: &str,
    _config: &ForgeConfig,
) -> PathBuf {
    match category {
        c if c.eq_ignore_ascii_case("Projects") => vault_root.join("01-Projects"),
        c if c.eq_ignore_ascii_case("Areas") => vault_root.join("02-Areas"),
        c if c.eq_ignore_ascii_case("Archive") => vault_root.join("99-Archives"),
        c if c.eq_ignore_ascii_case("Resources") => match subcategory {
            s if s.eq_ignore_ascii_case("Technical") => vault_root.join("03-Resources/Technical"),
            s if s.eq_ignore_ascii_case("Ideas") => vault_root.join("10-Zettelkasten"),
            s if s.eq_ignore_ascii_case("Reference") => {
                let d = match detail {
                    d if d.eq_ignore_ascii_case("Books-Notes") => "Books-Notes",
                    d if d.eq_ignore_ascii_case("Tutorials-Guides") => "Tutorials-Guides",
                    d if d.eq_ignore_ascii_case("Cheat-Sheets") => "Cheat-Sheets",
                    _ => "Articles-Papers",
                };
                vault_root.join("03-Resources/Reference").join(d)
            }
            _ => vault_root.join("03-Resources/Reference/Articles-Papers"),
        },
        _ => vault_root.join("99-Archives"),
    }
}

fn split_frontmatter(input: &str) -> Result<(Option<Frontmatter>, String)> {
    let re = Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").unwrap();
    if let Some(caps) = re.captures(input) {
        let yaml = caps.get(1).unwrap().as_str();
        let body = caps.get(2).unwrap().as_str().to_string();
        let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap_or_default();
        Ok((Some(fm), body))
    } else {
        Ok((None, input.to_string()))
    }
}

fn join_frontmatter(fm: &Frontmatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(fm).unwrap_or_default();
    format!("---\n{}---\n{}", yaml, body)
}

fn merge_vec(mut a: Vec<String>, b: Vec<String>) -> Vec<String> {
    for v in b {
        if !a.iter().any(|x| x == &v) {
            a.push(v);
        }
    }
    a
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339()
}
