use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::OnceLock,
};
use tracing::info;

use crate::ai::AiClient;
use crate::config::ForgeConfig;

use super::wikilinks::build_vault_graph;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCluster {
    pub canonical: String,
    pub aliases: Vec<String>,
    pub document_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TagNormalizationResult {
    pub clusters: Vec<TagCluster>,
    pub total_tags_before: usize,
    pub total_tags_after: usize,
}

impl std::fmt::Display for TagNormalizationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Tag Normalization ===")?;
        writeln!(
            f,
            "Tags before: {} -> after: {}",
            self.total_tags_before, self.total_tags_after
        )?;
        if !self.clusters.is_empty() {
            writeln!(f, "\nClusters:")?;
            for c in &self.clusters {
                if c.aliases.len() > 1 {
                    writeln!(
                        f,
                        "  {} ({} docs) <- {}",
                        c.canonical,
                        c.document_count,
                        c.aliases.join(", ")
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn fm_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").expect("valid frontmatter regex"))
}

fn tags_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^tags:\s*\[.*?\]").expect("valid tags regex"))
}

pub fn extract_all_tags(
    vault_root: &Path,
    config: &ForgeConfig,
) -> Result<BTreeMap<String, Vec<String>>> {
    let graph = build_vault_graph(vault_root, config)?;
    let fm = fm_re();
    let tag_line_re = Regex::new(r"(?m)^tags:\s*\[(.*?)\]")?;

    let mut tag_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for file in &graph.all_files {
        let path = vault_root.join(file);
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(caps) = fm.captures(&content) {
            let yaml = caps.get(1).unwrap().as_str();
            if let Some(tag_caps) = tag_line_re.captures(yaml) {
                let tags_str = tag_caps.get(1).unwrap().as_str();
                let tags: Vec<String> = tags_str
                    .split(',')
                    .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();

                if !tags.is_empty() {
                    tag_map.insert(file.clone(), tags);
                }
            }
        }
    }

    Ok(tag_map)
}

pub fn compute_tag_cooccurrence(
    tag_map: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut tag_docs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (doc, tags) in tag_map {
        for tag in tags {
            tag_docs
                .entry(normalize_tag(tag))
                .or_default()
                .insert(doc.clone());
        }
    }

    tag_docs
}

pub fn cluster_tags_by_cooccurrence(
    tag_docs: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<TagCluster> {
    let tags: Vec<&String> = tag_docs.keys().collect();
    let mut parent: BTreeMap<String, String> = BTreeMap::new();

    let find = |parent: &BTreeMap<String, String>, x: &str| -> String {
        let mut root = x.to_string();
        while let Some(p) = parent.get(&root) {
            if p == &root {
                break;
            }
            root = p.clone();
        }
        root
    };

    for i in 0..tags.len() {
        for j in (i + 1)..tags.len() {
            let a = tags[i];
            let b = tags[j];

            let docs_a = tag_docs.get(a);
            let docs_b = tag_docs.get(b);

            if let (Some(da), Some(db)) = (docs_a, docs_b) {
                let intersection = da.intersection(db).count();
                let union = da.union(db).count();
                if union == 0 {
                    continue;
                }
                let jaccard = intersection as f64 / union as f64;

                if jaccard > 0.6 {
                    let root_a = find(&parent, a);
                    let root_b = find(&parent, b);
                    if root_a != root_b {
                        let count_a = tag_docs.get(&root_a).map(|s| s.len()).unwrap_or(0);
                        let count_b = tag_docs.get(&root_b).map(|s| s.len()).unwrap_or(0);
                        let (winner, loser) = if count_a >= count_b {
                            (&root_a, &root_b)
                        } else {
                            (&root_b, &root_a)
                        };
                        parent.insert(loser.clone(), winner.clone());
                    }
                }
            }
        }
    }

    let mut cluster_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tag in &tags {
        let root = find(&parent, tag);
        cluster_map.entry(root).or_default().push((*tag).clone());
    }

    cluster_map
        .into_iter()
        .map(|(canonical, aliases)| {
            let doc_count = tag_docs.get(&canonical).map(|s| s.len()).unwrap_or(0);
            TagCluster {
                canonical,
                aliases,
                document_count: doc_count,
            }
        })
        .collect()
}

pub async fn normalize_tags(
    vault_root: &Path,
    config: &ForgeConfig,
    dry_run: bool,
) -> Result<TagNormalizationResult> {
    let tag_map = extract_all_tags(vault_root, config)?;
    let total_before: usize = tag_map.values().map(|v| v.len()).sum();

    if tag_map.is_empty() {
        info!("No tags found in vault");
        return Ok(TagNormalizationResult {
            clusters: Vec::new(),
            total_tags_before: 0,
            total_tags_after: 0,
        });
    }

    let tag_docs = compute_tag_cooccurrence(&tag_map);
    let clusters = cluster_tags_by_cooccurrence(&tag_docs);

    let ai = AiClient::from_config(&config.ai);
    let ai_clusters = suggest_hierarchy_with_ai(&ai, &tag_docs).await;

    let mut final_clusters = clusters;
    if let Ok(ai_suggestions) = ai_clusters {
        merge_ai_suggestions(&mut final_clusters, &ai_suggestions);
    }

    let canonical_map = build_canonical_map(&final_clusters);
    let total_after: usize = tag_map
        .values()
        .map(|tags| {
            tags.iter()
                .map(|t| {
                    canonical_map
                        .get(&normalize_tag(t))
                        .cloned()
                        .unwrap_or_else(|| normalize_tag(t))
                })
                .collect::<BTreeSet<String>>()
                .len()
        })
        .sum();

    if !dry_run {
        apply_tag_normalization(vault_root, &tag_map, &canonical_map)?;
    }

    Ok(TagNormalizationResult {
        clusters: final_clusters,
        total_tags_before: total_before,
        total_tags_after: total_after,
    })
}

fn normalize_tag(tag: &str) -> String {
    tag.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

#[derive(Deserialize, Default)]
struct AiTagSuggestions {
    #[serde(default)]
    clusters: Vec<AiTagCluster>,
}

#[derive(Deserialize)]
struct AiTagCluster {
    canonical: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[allow(dead_code)]
    suggested_parent: Option<String>,
}

async fn suggest_hierarchy_with_ai(
    ai: &AiClient,
    tag_docs: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<AiTagCluster>> {
    let tags_with_counts: Vec<(String, usize)> = tag_docs
        .iter()
        .map(|(tag, docs)| (tag.clone(), docs.len()))
        .filter(|(_, count)| *count >= 2)
        .collect();

    if tags_with_counts.is_empty() {
        return Ok(Vec::new());
    }

    let tag_list = tags_with_counts
        .iter()
        .map(|(tag, count)| format!("- {} ({} docs)", tag, count))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Analyze these tags from an Obsidian vault and suggest a normalized hierarchical structure.\n\n\
         Rules:\n\
         - Group similar tags under a common prefix (e.g., \"rust\" and \"rust-lang\" become \"lang/rust\")\n\
         - Use lowercase with hyphens\n\
         - Preserve the most common spelling as the canonical form\n\n\
         Current tags:\n{}\n\n\
         Output JSON only:\n\
         {{\"clusters\": [{{\"canonical\": \"...\", \"aliases\": [...], \"suggested_parent\": \"...\"}}]}}",
        tag_list
    );

    let result: AiTagSuggestions = ai.generate_json(&prompt).await.unwrap_or_default();
    Ok(result.clusters)
}

fn merge_ai_suggestions(clusters: &mut Vec<TagCluster>, ai_suggestions: &[AiTagCluster]) {
    for suggestion in ai_suggestions {
        let canonical = &suggestion.canonical;
        if let Some(existing) = clusters.iter_mut().find(|c| {
            c.aliases
                .iter()
                .any(|a| normalize_tag(a) == normalize_tag(canonical))
        }) {
            for alias in &suggestion.aliases {
                let norm = normalize_tag(alias);
                if !existing.aliases.iter().any(|a| normalize_tag(a) == norm) {
                    existing.aliases.push(alias.clone());
                }
            }
        } else if !suggestion.aliases.is_empty() {
            clusters.push(TagCluster {
                canonical: canonical.clone(),
                aliases: suggestion.aliases.clone(),
                document_count: 0,
            });
        }
    }
}

fn build_canonical_map(clusters: &[TagCluster]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for cluster in clusters {
        for alias in &cluster.aliases {
            map.insert(normalize_tag(alias), cluster.canonical.clone());
        }
    }
    map
}

fn apply_tag_normalization(
    vault_root: &Path,
    tag_map: &BTreeMap<String, Vec<String>>,
    canonical_map: &BTreeMap<String, String>,
) -> Result<()> {
    let fm = fm_re();
    let mut changed = 0u32;

    for (file, tags) in tag_map {
        let path = vault_root.join(file);
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let normalized: BTreeSet<String> = tags
            .iter()
            .map(|t| {
                canonical_map
                    .get(&normalize_tag(t))
                    .cloned()
                    .unwrap_or_else(|| normalize_tag(t))
            })
            .collect();

        let new_tags_str = normalized.into_iter().collect::<Vec<_>>().join(", ");

        if let Some(caps) = fm.captures(&content) {
            let yaml = caps.get(1).unwrap().as_str();
            let body = caps.get(2).unwrap().as_str();

            let new_yaml = if tags_re().is_match(yaml) {
                tags_re()
                    .replace(yaml, format!("tags: [{}]", new_tags_str).as_str())
                    .to_string()
            } else {
                format!("{}\ntags: [{}]", yaml, new_tags_str)
            };

            let new_content = format!("---\n{}---\n{}", new_yaml, body);
            if new_content != content {
                fs::write(&path, &new_content)?;
                changed += 1;
            }
        }
    }

    if changed > 0 {
        info!("Normalized tags in {} files", changed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_tag() {
        assert_eq!(normalize_tag("Rust"), "rust");
        assert_eq!(normalize_tag("Machine Learning"), "machine-learning");
        assert_eq!(normalize_tag("rust-lang"), "rust-lang");
    }

    #[test]
    fn test_cluster_tags_basic() {
        let mut tag_docs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        tag_docs.insert(
            "rust".into(),
            BTreeSet::from(["a.md".into(), "b.md".into(), "c.md".into()]),
        );
        tag_docs.insert(
            "rust-lang".into(),
            BTreeSet::from(["a.md".into(), "b.md".into()]),
        );
        tag_docs.insert("python".into(), BTreeSet::from(["d.md".into()]));

        let clusters = cluster_tags_by_cooccurrence(&tag_docs);

        let rust_cluster = clusters.iter().find(|c| c.canonical == "rust");
        assert!(rust_cluster.is_some());
        let rc = rust_cluster.unwrap();
        assert!(rc.aliases.contains(&"rust".to_string()));
        assert!(rc.aliases.contains(&"rust-lang".to_string()));
    }

    #[test]
    fn test_build_canonical_map() {
        let clusters = vec![TagCluster {
            canonical: "lang/rust".into(),
            aliases: vec!["rust".into(), "rust-lang".into()],
            document_count: 5,
        }];

        let map = build_canonical_map(&clusters);
        assert_eq!(map.get("rust"), Some(&"lang/rust".to_string()));
        assert_eq!(map.get("rust-lang"), Some(&"lang/rust".to_string()));
    }
}
