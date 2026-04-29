pub mod autotag;
pub mod backlinks;
pub mod bridges;
pub mod health;
pub mod orphans;
pub mod relationships;
pub mod scan;
pub mod tags;
pub mod wikilinks;

use anyhow::Result;
use std::path::Path;
use tracing::info;

use crate::config::ForgeConfig;

pub use health::{graph_health, GraphHealth};
pub use orphans::{auto_link_orphans, detect_orphans};
pub use relationships::{
    extract_relationships, save_relationships_manifest, Relationship, RelationshipManifest,
    RelationType,
};
pub use tags::{normalize_tags, TagNormalizationResult};
pub use wikilinks::{build_vault_graph, VaultGraph, Wikilink};

/// Run the full graph strengthening pipeline (existing behavior).
pub fn strengthen_graph(vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    info!("Graph strengthening started");

    let profiles = scan::scan_all_projects(vault_root, config)?;
    info!("Scanned {} projects", profiles.len());

    let bridges = bridges::detect_bridges(&profiles, config);
    info!("Detected {} cross-project concepts", bridges.len());

    if config.graph.bridge_notes {
        bridges::generate_bridge_notes(vault_root, &bridges, &config.vault.zettelkasten_dir)?;
    }

    if config.graph.backlinks {
        backlinks::inject_backlinks(vault_root, &profiles, &config.vault.zettelkasten_dir)?;
    }

    if config.graph.related_projects {
        bridges::update_related_projects(
            vault_root,
            &profiles,
            &bridges,
            &config.vault.zettelkasten_dir,
        )?;
    }

    if config.graph.auto_tags {
        autotag::auto_tag_documents(&profiles, config)?;
    }

    info!("Graph strengthening complete");
    Ok(())
}
