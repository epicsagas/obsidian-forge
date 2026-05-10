use anyhow::{Context, Result, bail};
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

    // AI Suggestions (Rule A: Suggest, don't move)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_project: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_area: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_concepts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,

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
            if is_pdf(&path) { Some(path) } else { None }
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
    let mut current_fm = fm.unwrap_or_default();

    // Check current status
    let status = current_fm.status.as_deref().unwrap_or("inbox");

    // Case 1: Human has confirmed the suggestions -> MOVE FILE
    if status == "confirmed" {
        let (category, subcategory, detail) = resolve_confirmed_targets(&current_fm);
        info!("Moving confirmed file: {}", path.display());

        // Move first to avoid marking "processed" if move fails
        let dest = move_to_para(path, &category, &subcategory, &detail, config, vault_root)?;

        current_fm.status = Some("processed".to_string());
        let updated = join_frontmatter(&current_fm, &body);
        tokio::fs::write(&dest, &updated).await?;

        return Ok(());
    }

    // Case 2: Already waiting for review or already processed -> SKIP
    if status == "needs_review" || status == "processed" {
        debug!("Skipping (status={}): {}", status, path.display());
        return Ok(());
    }

    // Case 3: New or unclassified file -> ANALYZE
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

    // Execute independent AI calls in parallel
    let (summary, questions, gen_tags, candidates) = tokio::join!(
        ollama.summarize(&body, 200),
        ollama.generate_json::<Vec<String>>(&q_prompt),
        ollama.generate_json::<Vec<String>>(&t_prompt),
        get_ai_candidates(&body, &ollama, &prompts)
    );

    let summary = summary.unwrap_or_default();
    let questions = questions.unwrap_or_default();
    let gen_tags = gen_tags.unwrap_or_else(|e| {
        warn!("Tag generation failed: {:?}", e);
        vec![]
    });
    let cand = candidates.unwrap_or_default();

    current_fm.status = Some("needs_review".to_string());
    current_fm.summary = Some(summary);
    current_fm.questions = Some(questions);
    const DEFAULT_CANDIDATE_TYPE: &str = "Resource";
    current_fm.candidate_type = Some(
        cand.candidate_type
            .unwrap_or_else(|| DEFAULT_CANDIDATE_TYPE.into()),
    );
    current_fm.candidate_project = cand.candidate_project;
    current_fm.candidate_area = cand.candidate_area;
    current_fm.candidate_concepts = cand.candidate_concepts;
    const VALID_ACTIONS: &[&str] = &["move", "link_existing", "promote_to_zettel"];
    current_fm.recommended_action = cand
        .recommended_action
        .filter(|a| VALID_ACTIONS.iter().any(|v| a.eq_ignore_ascii_case(v)));
    current_fm.reasoning = cand.reasoning;
    current_fm.subcategory = cand.subcategory;
    current_fm.detail = cand.detail;
    current_fm.tags = Some(merge_vec(
        current_fm.tags.take().unwrap_or_default(),
        gen_tags,
    ));
    current_fm.processed_at = Some(iso_now());

    let updated = join_frontmatter(&current_fm, &body);
    tokio::fs::write(path, &updated)
        .await
        .with_context(|| format!("write {}", path.display()))?;

    info!(
        "AI Analysis complete, waiting for human review: {}",
        path.display()
    );
    Ok(())
}

#[derive(serde::Deserialize, Default)]
struct AiCandidates {
    candidate_type: Option<String>,
    candidate_project: Option<Vec<String>>,
    candidate_area: Option<Vec<String>>,
    candidate_concepts: Option<Vec<String>>,
    recommended_action: Option<String>,
    reasoning: Option<String>,
    subcategory: Option<String>,
    detail: Option<String>,
}

fn resolve_confirmed_targets(fm: &Frontmatter) -> (String, String, String) {
    let category = fm
        .category
        .clone()
        .or_else(|| {
            fm.candidate_type.as_ref().map(|t| match t.as_str() {
                "ConceptSeed" => "Zettelkasten".into(),
                "Project" => "Projects".into(),
                "Area" => "Areas".into(),
                "Resource" => "Resources".into(),
                other => other.into(),
            })
        })
        .unwrap_or_else(|| "Resources".into());

    let subcategory = fm
        .subcategory
        .clone()
        .unwrap_or_else(|| match category.as_str() {
            "Zettelkasten" => "fleeting".into(),
            _ => "Reference".into(),
        });

    let detail = fm
        .detail
        .clone()
        .unwrap_or_else(|| match category.as_str() {
            "Resources" => "Articles-Papers".into(),
            _ => String::new(),
        });

    (category, subcategory, detail)
}

async fn get_ai_candidates(
    body: &str,
    ollama: &AiClient,
    prompts: &crate::prompts::LoadedPrompts,
) -> Result<AiCandidates> {
    let c_prompt = prompts.category_template.replace("{content}", body);
    ollama.generate_json(&c_prompt).await
}

fn move_to_para(
    path: &Path,
    category: &str,
    subcategory: &str,
    detail: &str,
    config: &ForgeConfig,
    vault_root: &Path,
) -> Result<PathBuf> {
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
    Ok(dest)
}

fn resolve_dest_dir(
    vault_root: &Path,
    category: &str,
    subcategory: &str,
    detail: &str,
    config: &ForgeConfig,
) -> PathBuf {
    let zk = &config.vault.zettelkasten_dir;
    match category {
        c if c.eq_ignore_ascii_case("Projects") => vault_root.join("01-Projects"),
        c if c.eq_ignore_ascii_case("Areas") => vault_root.join("02-Areas"),
        c if c.eq_ignore_ascii_case("Archive") => vault_root.join("99-Archives"),
        c if c.eq_ignore_ascii_case("Zettelkasten") => {
            let sub = match subcategory {
                s if s.eq_ignore_ascii_case("literature") => "literature",
                s if s.eq_ignore_ascii_case("permanent") => "permanent",
                _ => "fleeting",
            };
            vault_root.join(zk).join(sub)
        }
        c if c.eq_ignore_ascii_case("Resources") => match subcategory {
            s if s.eq_ignore_ascii_case("Technical") => vault_root.join("03-Resources/Technical"),
            // "Ideas" is no longer in the default AI prompt but kept for custom prompt compatibility
            s if s.eq_ignore_ascii_case("Ideas") => vault_root.join(zk).join("fleeting"),
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
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").expect("valid frontmatter regex")
    });
    if let Some(caps) = re.captures(input) {
        let yaml = caps
            .get(1)
            .expect("capture group 1 always present")
            .as_str();
        let body = caps
            .get(2)
            .expect("capture group 2 always present")
            .as_str()
            .to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn create(path: &Path) -> std::result::Result<Self, std::io::Error> {
            if path.exists() {
                fs::remove_dir_all(path)?;
            }
            fs::create_dir_all(path)?;
            Ok(TempDir(path.to_path_buf()))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_split_frontmatter_with_frontmatter() {
        let input = "---\nstatus: processed\ntags: [rust, test]\n---\n# Hello\n\nBody text.";
        let (fm, body) = split_frontmatter(input).unwrap();
        assert!(fm.is_some());
        let fm = fm.unwrap();
        assert_eq!(fm.status.as_deref(), Some("processed"));
        assert_eq!(body, "# Hello\n\nBody text.");
    }

    #[test]
    fn test_split_frontmatter_without_frontmatter() {
        let input = "# Hello\n\nNo frontmatter here.";
        let (fm, body) = split_frontmatter(input).unwrap();
        assert!(fm.is_none());
        assert_eq!(body, input);
    }

    #[test]
    fn test_split_frontmatter_empty_input() {
        let (fm, body) = split_frontmatter("").unwrap();
        assert!(fm.is_none());
        assert_eq!(body, "");
    }

    #[test]
    fn test_join_frontmatter_roundtrip() {
        let fm = Frontmatter {
            status: Some("processed".into()),
            tags: Some(vec!["rust".into(), "test".into()]),
            ..Default::default()
        };
        let body = "# Title\n\nContent.";
        let joined = join_frontmatter(&fm, body);
        assert!(joined.starts_with("---\n"));
        assert!(joined.contains("status: processed"));
        assert!(joined.contains(body));
    }

    #[test]
    fn test_merge_vec_no_duplicates() {
        let a = vec!["rust".to_string(), "test".to_string()];
        let b = vec!["test".to_string(), "new".to_string()];
        let result = merge_vec(a, b);
        assert_eq!(result, vec!["rust", "test", "new"]);
    }

    #[test]
    fn test_merge_vec_empty() {
        let result = merge_vec(vec![], vec!["a".into(), "b".into()]);
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn test_is_markdown() {
        use std::fs::File;
        let dir = std::env::temp_dir();
        let md = dir.join("obsidian_forge_test_note.md");
        let txt = dir.join("obsidian_forge_test_note.txt");
        File::create(&md).unwrap();
        File::create(&txt).unwrap();
        assert!(is_markdown(&md));
        assert!(!is_markdown(&txt));
        let _ = std::fs::remove_file(&md);
        let _ = std::fs::remove_file(&txt);
    }

    #[test]
    fn test_resolve_dest_dir_zettelkasten_fleeting() {
        let vault = PathBuf::from("/vault");
        let cfg = ForgeConfig::default_for("v");
        let dest = resolve_dest_dir(&vault, "Zettelkasten", "fleeting", "", &cfg);
        assert_eq!(dest, vault.join("10-Zettelkasten/fleeting"));
    }

    #[test]
    fn test_resolve_dest_dir_zettelkasten_literature() {
        let vault = PathBuf::from("/vault");
        let cfg = ForgeConfig::default_for("v");
        let dest = resolve_dest_dir(&vault, "Zettelkasten", "literature", "", &cfg);
        assert_eq!(dest, vault.join("10-Zettelkasten/literature"));
    }

    #[test]
    fn test_resolve_dest_dir_zettelkasten_permanent() {
        let vault = PathBuf::from("/vault");
        let cfg = ForgeConfig::default_for("v");
        let dest = resolve_dest_dir(&vault, "Zettelkasten", "permanent", "", &cfg);
        assert_eq!(dest, vault.join("10-Zettelkasten/permanent"));
    }

    #[test]
    fn test_resolve_dest_dir_resources_ideas_routes_to_fleeting() {
        let vault = PathBuf::from("/vault");
        let cfg = ForgeConfig::default_for("v");
        let dest = resolve_dest_dir(&vault, "Resources", "Ideas", "", &cfg);
        assert_eq!(dest, vault.join("10-Zettelkasten/fleeting"));
    }

    #[test]
    fn test_resolve_dest_dir_books_notes() {
        let vault = PathBuf::from("/vault");
        let cfg = ForgeConfig::default_for("v");
        let dest = resolve_dest_dir(&vault, "Resources", "Reference", "Books-Notes", &cfg);
        assert_eq!(dest, vault.join("03-Resources/Reference/Books-Notes"));
    }

    #[test]
    fn test_resolve_dest_dir_zettelkasten_unknown_falls_back_to_fleeting() {
        let vault = PathBuf::from("/vault");
        let cfg = ForgeConfig::default_for("v");
        let dest = resolve_dest_dir(&vault, "Zettelkasten", "unknown-garbage", "", &cfg);
        assert_eq!(dest, vault.join("10-Zettelkasten/fleeting"));
    }

    #[tokio::test]
    async fn test_process_one_confirmed_moves_file() -> Result<()> {
        let test_root = std::env::current_dir()?
            .join("target")
            .join("test_inbox_move");
        let _guard = TempDir::create(&test_root)?;

        let vault_root = test_root.as_path();
        let inbox_dir = vault_root.join("00-Inbox");
        fs::create_dir_all(&inbox_dir)?;

        let note_path = inbox_dir.join("test_note.md");
        let content = "---\nstatus: confirmed\ncategory: Projects\n---\n# Test Note\nBody";
        fs::write(&note_path, content)?;

        let mut config = ForgeConfig::default_for("test");
        config.vault.inbox_dir = "00-Inbox".to_string();

        process_one(&note_path, &config, vault_root).await?;

        let expected_path = vault_root.join("01-Projects").join("test_note.md");
        assert!(expected_path.exists());
        assert!(!note_path.exists());

        let new_content = fs::read_to_string(expected_path)?;
        assert!(new_content.contains("status: processed"));

        Ok(())
    }

    #[tokio::test]
    async fn test_process_one_needs_review_skips() -> Result<()> {
        let test_root = std::env::current_dir()?
            .join("target")
            .join("test_needs_review_skip");
        let _guard = TempDir::create(&test_root)?;

        let vault_root = test_root.as_path();
        let inbox_dir = vault_root.join("00-Inbox");
        fs::create_dir_all(&inbox_dir)?;

        let note_path = inbox_dir.join("review_note.md");
        let content = "---\nstatus: needs_review\nsummary: test\n---\n# Review Note\nBody";
        fs::write(&note_path, content)?;

        let config = ForgeConfig::default_for("test");
        process_one(&note_path, &config, vault_root).await?;

        assert!(note_path.exists(), "needs_review file should stay in inbox");
        let result = fs::read_to_string(&note_path)?;
        assert!(result.contains("status: needs_review"));

        Ok(())
    }

    #[tokio::test]
    async fn test_process_one_processed_skips() -> Result<()> {
        let test_root = std::env::current_dir()?
            .join("target")
            .join("test_processed_skip");
        let _guard = TempDir::create(&test_root)?;

        let vault_root = test_root.as_path();
        let inbox_dir = vault_root.join("00-Inbox");
        fs::create_dir_all(&inbox_dir)?;

        let note_path = inbox_dir.join("done_note.md");
        let content = "---\nstatus: processed\n---\n# Done Note\nBody";
        fs::write(&note_path, content)?;

        let config = ForgeConfig::default_for("test");
        process_one(&note_path, &config, vault_root).await?;

        assert!(note_path.exists(), "processed file should stay untouched");
        let result = fs::read_to_string(&note_path)?;
        assert!(result.contains("status: processed"));

        Ok(())
    }

    #[test]
    fn test_resolve_confirmed_targets_concept_seed() {
        let fm = Frontmatter {
            candidate_type: Some("ConceptSeed".into()),
            ..Default::default()
        };
        let (cat, sub, _detail) = resolve_confirmed_targets(&fm);
        assert_eq!(cat, "Zettelkasten");
        assert_eq!(sub, "fleeting");
    }

    #[test]
    fn test_resolve_confirmed_targets_project() {
        let fm = Frontmatter {
            candidate_type: Some("Project".into()),
            ..Default::default()
        };
        let (cat, _, _) = resolve_confirmed_targets(&fm);
        assert_eq!(cat, "Projects");
    }

    #[test]
    fn test_resolve_confirmed_targets_explicit_category_wins() {
        let fm = Frontmatter {
            category: Some("Resources".into()),
            candidate_type: Some("Project".into()),
            ..Default::default()
        };
        let (cat, _, _) = resolve_confirmed_targets(&fm);
        assert_eq!(cat, "Resources");
    }

    #[test]
    fn test_resolve_confirmed_targets_area() {
        let fm = Frontmatter {
            candidate_type: Some("Area".into()),
            ..Default::default()
        };
        let (cat, _, _) = resolve_confirmed_targets(&fm);
        assert_eq!(cat, "Areas");
    }

    #[test]
    fn test_resolve_confirmed_targets_zettelkasten_detail_is_empty() {
        let fm = Frontmatter {
            candidate_type: Some("ConceptSeed".into()),
            ..Default::default()
        };
        let (_, _, detail) = resolve_confirmed_targets(&fm);
        assert!(detail.is_empty(), "Zettelkasten should have no detail");
    }

    #[test]
    fn test_resolve_confirmed_targets_resources_detail_default() {
        let fm = Frontmatter {
            candidate_type: Some("Resource".into()),
            ..Default::default()
        };
        let (cat, _, detail) = resolve_confirmed_targets(&fm);
        assert_eq!(cat, "Resources");
        assert_eq!(detail, "Articles-Papers");
    }

    #[test]
    fn test_classify_book_keywords() {
        let title_hits = [
            "Book review: Rust in Action",
            "ISBN 978-3-16",
            "book summary of DDIA",
        ];
        let title_misses = ["Chapter 3: Kubernetes", "Reading sensor data"];
        let book = ["book review", "book note", "book summary", "isbn"];
        for title in &title_hits {
            let t = title.to_lowercase();
            assert!(
                book.iter().any(|kw| t.contains(kw)),
                "should match: {}",
                title
            );
        }
        for title in &title_misses {
            let t = title.to_lowercase();
            assert!(
                !book.iter().any(|kw| t.contains(kw)),
                "should NOT match: {}",
                title
            );
        }
    }

    #[test]
    fn test_is_pdf() {
        use std::fs::File;
        let dir = std::env::temp_dir();
        let pdf = dir.join("obsidian_forge_test_doc.pdf");
        let md = dir.join("obsidian_forge_test_doc.md");
        File::create(&pdf).unwrap();
        File::create(&md).unwrap();
        assert!(is_pdf(&pdf));
        assert!(!is_pdf(&md));
        let _ = std::fs::remove_file(&pdf);
        let _ = std::fs::remove_file(&md);
    }
}
