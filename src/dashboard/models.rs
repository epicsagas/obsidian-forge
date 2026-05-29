use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteCard {
    pub path: String,
    pub title: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub vitality: u8,
    pub zone: Zone,
    pub layer: Option<String>,
    pub word_count: u32,
    pub modified_at: String,
    pub incoming_links: usize,
    pub outgoing_links: usize,
    pub has_mermaid: bool,
    pub is_orphan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Zone {
    Inbox,
    Projects(String),
    Areas(String),
    Resources,
    Zettelkasten,
    Archives,
}

impl Zone {
    pub fn label(&self) -> String {
        match self {
            Zone::Inbox => "Inbox".to_string(),
            Zone::Projects(name) => format!("Projects/{}", name),
            Zone::Areas(name) => format!("Areas/{}", name),
            Zone::Resources => "Resources".to_string(),
            Zone::Zettelkasten => "Zettelkasten".to_string(),
            Zone::Archives => "Archives".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSummary {
    pub tag: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionPanel {
    pub orphans: Vec<NoteCard>,
    pub stale: Vec<NoteCard>,
    pub untagged: Vec<NoteCard>,
    pub inbox_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardState {
    pub notes: Vec<NoteCard>,
    pub tags: Vec<TagSummary>,
    pub needs_attention: AttentionPanel,
    pub vault_name: String,
    pub total_notes: usize,
    pub orphan_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    pub name: String,
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeFilter {
    Today,
    ThisWeek,
    ThisMonth,
    All,
}
