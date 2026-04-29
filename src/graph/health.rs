use anyhow::Result;
use std::path::Path;

use crate::config::ForgeConfig;

use super::wikilinks::build_vault_graph;

#[derive(Debug, Clone)]
pub struct GraphHealth {
    pub total_notes: usize,
    pub total_links: usize,
    pub orphan_count: usize,
    pub avg_links_per_note: f64,
    pub connected_components: usize,
    pub hub_notes: Vec<(String, usize)>,
    pub broken_links: Vec<(String, String)>,
}

impl std::fmt::Display for GraphHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Graph Health Report ===")?;
        writeln!(f, "Total notes:            {}", self.total_notes)?;
        writeln!(f, "Total links:            {}", self.total_links)?;
        writeln!(f, "Avg links per note:     {:.1}", self.avg_links_per_note)?;
        writeln!(f, "Connected components:   {}", self.connected_components)?;
        writeln!(f, "Orphan notes:           {}", self.orphan_count)?;
        writeln!(f, "Broken links:           {}", self.broken_links.len())?;

        if !self.hub_notes.is_empty() {
            writeln!(f, "\n--- Hub Notes (most linked) ---")?;
            for (path, count) in &self.hub_notes {
                writeln!(f, "  {} ({} incoming)", path, count)?;
            }
        }

        if !self.broken_links.is_empty() {
            writeln!(f, "\n--- Broken Links ---")?;
            for (source, target) in &self.broken_links {
                writeln!(f, "  {} -> {} (not found)", source, target)?;
            }
        }

        Ok(())
    }
}

pub fn graph_health(vault_root: &Path, config: &ForgeConfig) -> Result<GraphHealth> {
    let graph = build_vault_graph(vault_root, config)?;

    let total_notes = graph.all_files.len();
    let total_links = graph.total_links();
    let avg_links = if total_notes > 0 {
        total_links as f64 / total_notes as f64
    } else {
        0.0
    };

    Ok(GraphHealth {
        total_notes,
        total_links,
        orphan_count: graph.orphan_count(),
        avg_links_per_note: avg_links,
        connected_components: graph.connected_components(),
        hub_notes: graph.hub_notes(10),
        broken_links: graph.broken_links(),
    })
}
