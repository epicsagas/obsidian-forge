use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use tracing::{debug, info};

use crate::ai::AiClient;
use crate::config::ForgeConfig;

use super::wikilinks::VaultGraph;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RelationType {
    Extends,
    DependsOn,
    Contradicts,
    SimilarTo,
    References,
    RelatedTo,
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelationType::Extends => write!(f, "extends"),
            RelationType::DependsOn => write!(f, "depends-on"),
            RelationType::Contradicts => write!(f, "contradicts"),
            RelationType::SimilarTo => write!(f, "similar-to"),
            RelationType::References => write!(f, "references"),
            RelationType::RelatedTo => write!(f, "related-to"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub source: String,
    pub target: String,
    pub relation: RelationType,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationshipManifest {
    #[serde(default)]
    pub relationships: Vec<Relationship>,
}

#[derive(Deserialize, Default)]
struct AiRelationships {
    #[serde(default)]
    relationships: Vec<AiRelation>,
}

#[derive(Deserialize)]
struct AiRelation {
    source_file: String,
    target_file: String,
    relation: RelationType,
    #[serde(default)]
    confidence: f32,
}

pub async fn extract_relationships(
    vault_root: &Path,
    config: &ForgeConfig,
    graph: &VaultGraph,
) -> Result<Vec<Relationship>> {
    let pairs = collect_candidate_pairs(graph);
    if pairs.is_empty() {
        info!("No candidate note pairs found for relationship extraction");
        return Ok(Vec::new());
    }

    info!(
        "Extracting relationships from {} candidate pairs",
        pairs.len()
    );

    let summaries = build_summary_cache(vault_root, graph);
    let ai = AiClient::from_config(&config.ai);
    let concurrency = config.ai.max_concurrent.unwrap_or(5);

    let batch_size = 15usize;
    let batches: Vec<Vec<(String, String)>> =
        pairs.chunks(batch_size).map(|c| c.to_vec()).collect();

    let all_relationships: Vec<Relationship> = stream::iter(batches)
        .map(|batch| {
            let ai = ai.clone();
            let summaries = summaries.clone();
            async move { process_batch(&ai, &batch, &summaries).await }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect();

    info!("Extracted {} relationships", all_relationships.len());

    Ok(all_relationships)
}

fn collect_candidate_pairs(graph: &VaultGraph) -> Vec<(String, String)> {
    let mut seen = BTreeSet::new();
    let mut pairs = Vec::new();

    for (source, targets) in &graph.outgoing {
        for target in targets {
            if seen.contains(&(target.clone(), source.clone())) {
                continue;
            }
            seen.insert((source.clone(), target.clone()));

            pairs.push((source.clone(), target.clone()));

            if let Some(second_hop) = graph.outgoing.get(target) {
                for hop_target in second_hop {
                    if hop_target != source && !seen.contains(&(hop_target.clone(), source.clone()))
                    {
                        seen.insert((source.clone(), hop_target.clone()));
                        pairs.push((source.clone(), hop_target.clone()));
                    }
                }
            }
        }
    }

    pairs
}

fn build_summary_cache(vault_root: &Path, graph: &VaultGraph) -> BTreeMap<String, String> {
    let mut cache = BTreeMap::new();
    for file in &graph.all_files {
        let path = vault_root.join(file);
        if let Ok(content) = fs::read_to_string(&path) {
            let summary: String = content
                .lines()
                .filter(|l| !l.starts_with("---") && !l.starts_with('#') && !l.is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(150)
                .collect();
            cache.insert(file.clone(), summary);
        }
    }
    cache
}

async fn process_batch(
    ai: &AiClient,
    batch: &[(String, String)],
    summaries: &BTreeMap<String, String>,
) -> Vec<Relationship> {
    let pairs_text = batch
        .iter()
        .enumerate()
        .map(|(i, (s, t))| {
            let s_sum = summaries.get(s).map(|x| x.as_str()).unwrap_or("N/A");
            let t_sum = summaries.get(t).map(|x| x.as_str()).unwrap_or("N/A");
            format!(
                "{}. {} (summary: {}) <-> {} (summary: {})",
                i + 1,
                s,
                truncate(s_sum, 100),
                t,
                truncate(t_sum, 100)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Analyze these note pairs and identify semantic relationships.\n\n\
         Valid types: extends, depends-on, contradicts, similar-to, references, related-to\n\n\
         Output JSON only:\n\
         {{\"relationships\": [{{\"source_file\": \"...\", \"target_file\": \"...\", \
         \"relation\": \"...\", \"confidence\": 0.0-1.0}}]}}\n\n\
         Note pairs:\n{}",
        pairs_text
    );

    match ai.generate_json::<AiRelationships>(&prompt).await {
        Ok(result) => result
            .relationships
            .into_iter()
            .filter(|r| r.confidence >= 0.5)
            .map(|r| Relationship {
                source: r.source_file,
                target: r.target_file,
                relation: r.relation,
                confidence: r.confidence,
            })
            .collect(),
        Err(e) => {
            debug!("AI relationship batch failed: {:?}", e);
            Vec::new()
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .take_while(|(i, _)| *i < max)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max.min(s.len()));
        s[..end].to_string()
    }
}

pub fn save_relationships_manifest(
    vault_root: &Path,
    relationships: &[Relationship],
) -> Result<()> {
    let manifest = RelationshipManifest {
        relationships: relationships.to_vec(),
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    let dir = vault_root.join(".obsidian-forge");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("relationships.json"), json)?;
    info!(
        "Saved {} relationships to .obsidian-forge/relationships.json",
        relationships.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_type_serde() {
        let rt = RelationType::DependsOn;
        let json = serde_json::to_string(&rt).unwrap();
        assert_eq!(json, "\"depends-on\"");
        let parsed: RelationType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, RelationType::DependsOn);
    }

    #[test]
    fn test_relation_type_display() {
        assert_eq!(RelationType::SimilarTo.to_string(), "similar-to");
        assert_eq!(RelationType::Extends.to_string(), "extends");
    }

    #[test]
    fn test_relationship_manifest_roundtrip() {
        let manifest = RelationshipManifest {
            relationships: vec![Relationship {
                source: "a.md".into(),
                target: "b.md".into(),
                relation: RelationType::References,
                confidence: 0.9,
            }],
        };
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let parsed: RelationshipManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.relationships.len(), 1);
        assert_eq!(parsed.relationships[0].relation, RelationType::References);
    }

    #[test]
    fn test_collect_candidate_pairs() {
        let mut graph = VaultGraph::default();
        graph.all_files.insert("a.md".into());
        graph.all_files.insert("b.md".into());
        graph.all_files.insert("c.md".into());

        graph
            .outgoing
            .insert("a.md".into(), BTreeSet::from(["b.md".into()]));
        graph
            .outgoing
            .insert("b.md".into(), BTreeSet::from(["c.md".into()]));

        let pairs = collect_candidate_pairs(&graph);
        assert!(pairs.contains(&("a.md".into(), "b.md".into())));
        assert!(pairs.contains(&("a.md".into(), "c.md".into())));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello world", 5), "hello");
        assert_eq!(truncate("hi", 10), "hi");
        assert_eq!(truncate("", 10), "");
    }
}
