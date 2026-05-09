use anyhow::Result;
use rayon::prelude::*;
use regex::Regex;
use std::{
    collections::BTreeSet,
    fs,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use tracing::{debug, info};

use crate::config::ForgeConfig;

use super::scan::ProjectProfile;

pub fn auto_tag_documents(profiles: &[ProjectProfile], config: &ForgeConfig) -> Result<()> {
    static FM_RE: OnceLock<Regex> = OnceLock::new();
    static TAGS_RE: OnceLock<Regex> = OnceLock::new();
    let fm_re = FM_RE.get_or_init(|| {
        Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").expect("valid frontmatter regex")
    });
    let tags_re = TAGS_RE.get_or_init(|| Regex::new(r"(?m)^tags:\s*\[").expect("valid tags regex"));

    let tagged_count = AtomicUsize::new(0);

    profiles.par_iter().for_each(|profile| {
        let project_name = profile.name.clone();
        let project_tags = config.graph.concepts.clone();

        profile.docs.iter().for_each(|doc_path| {
            let content = match fs::read_to_string(doc_path) {
                Ok(c) => c,
                Err(_) => return,
            };

            if tags_re.is_match(&content) {
                return;
            }

            let content_lower = content.to_lowercase();
            let mut tags: BTreeSet<String> = BTreeSet::new();
            tags.insert(project_name.clone());

            for concept in &project_tags {
                if concept.keywords.iter().any(|kw| content_lower.contains(kw)) {
                    for tag in &concept.tags {
                        if tag != "evergreen" {
                            tags.insert(tag.clone());
                        }
                    }
                }
            }

            let stem = doc_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            match stem {
                "PRD" => {
                    tags.insert("prd".into());
                }
                "ARCHITECTURE" => {
                    tags.insert("architecture".into());
                }
                "CONVENTIONS" => {
                    tags.insert("conventions".into());
                }
                "DECISIONS" => {
                    tags.insert("decisions".into());
                    tags.insert("adr".into());
                }
                "PROGRESS" => {
                    tags.insert("progress".into());
                }
                "DEBT" => {
                    tags.insert("tech-debt".into());
                }
                "SECRETS_MAP" => {
                    tags.insert("secrets".into());
                }
                _ => {}
            }

            if tags.is_empty() {
                return;
            }

            let tags_str = tags.into_iter().collect::<Vec<_>>().join(", ");

            let new_content = if let Some(caps) = fm_re.captures(&content) {
                let yaml = caps
                    .get(1)
                    .expect("capture group 1 always present")
                    .as_str();
                let body = caps
                    .get(2)
                    .expect("capture group 2 always present")
                    .as_str();
                if yaml.contains("tags:") {
                    return;
                }
                format!("---\n{}\ntags: [{}]\n---\n{}", yaml, tags_str, body)
            } else {
                format!("---\ntags: [{}]\n---\n\n{}", tags_str, content)
            };

            let _ = fs::write(doc_path, &new_content);
            tagged_count.fetch_add(1, Ordering::Relaxed);
            debug!("Auto-tagged: {}", doc_path.display());
        });
    });

    let count = tagged_count.load(Ordering::Relaxed);
    if count > 0 {
        info!("Auto-tagged {} documents", count);
    }
    Ok(())
}
