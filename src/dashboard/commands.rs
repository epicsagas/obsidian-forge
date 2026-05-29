use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use tauri::State;
use walkdir::WalkDir;

use crate::config::{ForgeConfig, GlobalConfig};
use crate::dashboard::models::*;
use crate::dashboard::scoring::compute_vitality;
use crate::graph::wikilinks::build_vault_graph;
use crate::vault_utils::frontmatter_re;

use super::AppState;

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_vaults() -> Result<Vec<VaultInfo>, String> {
    let global = GlobalConfig::load().map_err(|e| e.to_string())?;
    let vaults = global
        .vaults
        .iter()
        .map(|v| VaultInfo {
            name: v.name.clone(),
            path: v.path.clone(),
            enabled: v.enabled,
        })
        .collect();
    Ok(vaults)
}

#[tauri::command]
pub fn get_dashboard(
    state: State<'_, AppState>,
    vault_name: String,
) -> Result<DashboardState, String> {
    eprintln!("[dashboard] get_dashboard called: vault={}", vault_name);

    let global = GlobalConfig::load().map_err(|e| {
        eprintln!("[dashboard] config load error: {}", e);
        format!("config load: {}", e)
    })?;
    let entry = global
        .find_vault(&vault_name)
        .ok_or_else(|| format!("Vault '{}' not found", vault_name))?;
    let vault_path = PathBuf::from(&entry.path);

    if !vault_path.exists() {
        return Err(format!("Vault path does not exist: {}", vault_path.display()));
    }

    let config = ForgeConfig::load(&vault_path).map_err(|e| {
        eprintln!("[dashboard] vault config error: {}", e);
        format!("vault config: {}", e)
    })?;

    eprintln!("[dashboard] building dashboard for: {}", vault_path.display());

    let dashboard = build_dashboard(&vault_path, &config).map_err(|e| {
        eprintln!("[dashboard] build_dashboard error: {}", e);
        format!("build_dashboard: {}", e)
    })?;

    eprintln!(
        "[dashboard] built: {} notes, {} orphans",
        dashboard.total_notes, dashboard.orphan_count
    );

    {
        let mut cache = state.cache.write().map_err(|e| format!("cache: {}", e))?;
        *cache = Some(dashboard.clone());
    }

    Ok(dashboard)
}

#[tauri::command]
pub fn open_in_obsidian(path: String) -> Result<(), String> {
    // obsidian://open?vault=VaultName&file=Path
    let url = format!("obsidian://open?file={}", urlencoding(&path));
    open_url(&url).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Dashboard builder
// ---------------------------------------------------------------------------

fn build_dashboard(
    vault: &Path,
    config: &ForgeConfig,
) -> Result<DashboardState> {
    let system_dirs = config.all_system_dirs();

    // Scan all notes
    let raw_notes = scan_notes(vault, &system_dirs)?;

    // Build graph for link counts
    let graph = build_vault_graph(vault, config)?;

    // Hub notes (top 10 by incoming links) — used implicitly via incoming_links count
    let _hubs: HashMap<String, usize> = graph
        .hub_notes(10)
        .into_iter()
        .map(|(p, c)| (p, c))
        .collect();

    // Build NoteCards
    let mut cards: Vec<NoteCard> = raw_notes
        .into_iter()
        .map(|raw| {
            let rel_path = raw.relative_path.clone();
            let incoming = graph.incoming.get(&rel_path).map(|s| s.len()).unwrap_or(0);
            let outgoing = graph.outgoing.get(&rel_path).map(|s| s.len()).unwrap_or(0);
            let is_orphan = incoming == 0 && outgoing == 0;

            let mut card = NoteCard {
                path: raw.relative_path,
                title: raw.title,
                summary: raw.summary,
                tags: raw.tags,
                vitality: 0, // computed below
                zone: raw.zone,
                layer: raw.layer,
                word_count: raw.word_count,
                modified_at: raw.modified_at,
                incoming_links: incoming,
                outgoing_links: outgoing,
                has_mermaid: raw.has_mermaid,
                is_orphan,
            };
            card.vitality = compute_vitality(&card);
            card
        })
        .collect();

    // Sort by vitality desc, then by modified_at desc
    cards.sort_by(|a, b| {
        b.vitality
            .cmp(&a.vitality)
            .then_with(|| b.modified_at.cmp(&a.modified_at))
    });

    // Tag summaries
    let mut tag_counts: HashMap<String, u32> = HashMap::new();
    for card in &cards {
        for tag in &card.tags {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    let mut tags: Vec<TagSummary> = tag_counts
        .into_iter()
        .map(|(tag, count)| TagSummary { tag, count })
        .collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count));

    // Attention panel
    let all_cards = &cards; // unfiltered for attention
    let orphan_count = all_cards.iter().filter(|c| c.is_orphan).count();
    let needs_attention = AttentionPanel {
        orphans: all_cards
            .iter()
            .filter(|c| c.is_orphan)
            .take(10)
            .cloned()
            .collect(),
        stale: all_cards
            .iter()
            .filter(|c| days_since_modified(&c.modified_at) > 30)
            .take(10)
            .cloned()
            .collect(),
        untagged: all_cards
            .iter()
            .filter(|c| c.tags.is_empty())
            .take(10)
            .cloned()
            .collect(),
        inbox_count: {
            let inbox = vault.join(&config.vault.inbox_dir);
            if inbox.exists() {
                fs::read_dir(&inbox).map(|r| r.count()).unwrap_or(0)
            } else {
                0
            }
        },
    };

    let total = cards.len();

    Ok(DashboardState {
        notes: cards,
        tags,
        needs_attention,
        vault_name: config.vault.name.clone(),
        total_notes: total,
        orphan_count,
    })
}

// ---------------------------------------------------------------------------
// Note scanning
// ---------------------------------------------------------------------------

struct RawNote {
    relative_path: String,
    title: String,
    summary: Option<String>,
    tags: Vec<String>,
    zone: Zone,
    layer: Option<String>,
    word_count: u32,
    modified_at: String,
    has_mermaid: bool,
}

fn scan_notes(vault: &Path, system_dirs: &[String]) -> Result<Vec<RawNote>> {
    let system_set: std::collections::HashSet<&str> =
        system_dirs.iter().map(|s| s.as_str()).collect();

    let mut notes = Vec::new();

    for entry in WalkDir::new(vault)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 1 {
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with('.') || system_set.contains(name) {
                        return false;
                    }
                }
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let rel = path
            .strip_prefix(vault)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| {
                let secs: u64 = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_secs();
                chrono::DateTime::from_timestamp(secs as i64, 0)
            })
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_default();

        // Parse frontmatter
        let (fm_str, body) = if let Some(caps) = frontmatter_re().captures(&content) {
            (
                caps.get(1).map(|m| m.as_str()).unwrap_or(""),
                caps.get(2).map(|m| m.as_str()).unwrap_or(""),
            )
        } else {
            ("", content.as_str())
        };

        let fm: serde_yaml::Value = serde_yaml::from_str(fm_str).unwrap_or_default();

        // Tags
        let tags = extract_tags(&fm);

        // Layer
        let layer = tags
            .iter()
            .find(|t| t.starts_with("layer/"))
            .cloned();

        // Title: first H1 or filename
        let title = content
            .lines()
            .find(|l| l.starts_with("# "))
            .map(|l| l.trim_start_matches("# ").trim().to_string())
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            });

        // Summary: frontmatter `summary` or first paragraph
        let summary = fm
            .get("summary")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                body.lines()
                    .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                    .map(|l| {
                        let s = l.trim();
                        if s.len() > 200 {
                            format!("{}...", &s[..200])
                        } else {
                            s.to_string()
                        }
                    })
            });

        let word_count = body.split_whitespace().count() as u32;
        let has_mermaid = body.contains("```mermaid");

        // Zone classification
        let zone = classify_zone(&rel);

        notes.push(RawNote {
            relative_path: rel,
            title,
            summary,
            tags,
            zone,
            layer,
            word_count,
            modified_at: modified,
            has_mermaid,
        });
    }

    Ok(notes)
}

fn extract_tags(fm: &serde_yaml::Value) -> Vec<String> {
    fm.get("tags")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn classify_zone(rel_path: &str) -> Zone {
    let path = Path::new(rel_path);

    if let Some(first) = path.components().next().and_then(|c| c.as_os_str().to_str()) {
        match first {
            "00-Inbox" => Zone::Inbox,
            "01-Projects" => Zone::Archives, // Projects is just MOC hub, not storage
            "02-Areas" => {
                let area = path
                    .components()
                    .nth(1)
                    .and_then(|c| c.as_os_str().to_str())
                    .unwrap_or("general");
                Zone::Areas(area.to_string())
            }
            "03-Resources" => Zone::Resources,
            "10-Zettelkasten" => Zone::Zettelkasten,
            "99-Archives" => {
                // 99-Archives/projects/<name>/... → Zone::Projects(name)
                let mut comps = path.components();
                let _ = comps.next(); // 99-Archives
                if let Some(sub) = comps.next().and_then(|c| c.as_os_str().to_str()) {
                    if sub == "projects" {
                        if let Some(name) = comps.next().and_then(|c| c.as_os_str().to_str()) {
                            return Zone::Projects(name.to_string());
                        }
                    }
                }
                Zone::Archives
            }
            _ => Zone::Archives,
        }
    } else {
        Zone::Archives
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn days_since_modified(iso_date: &str) -> u32 {
    let date_str = iso_date.split('T').next().unwrap_or(iso_date);
    let parts: Vec<u32> = date_str
        .split('-')
        .filter_map(|p| p.parse().ok())
        .collect();

    if parts.len() != 3 {
        return 999;
    }

    let note_date = chrono::NaiveDate::from_ymd_opt(parts[0] as i32, parts[1], parts[2])
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    let today = chrono::Local::now().date_naive();
    (today - note_date).num_days().max(0) as u32
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('#', "%23")
        .replace('?', "%3F")
}

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).status()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).status()?;
    }
    #[cfg(target_os = "windows")]
    {
        opener::open(url)?;
    }
    Ok(())
}
