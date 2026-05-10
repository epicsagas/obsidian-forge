use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{info, warn};

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PromptLibrary {
    pub prompts: Option<Prompts>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Prompts {
    pub question_generation: Option<QuestionGeneration>,
    pub classification: Option<Classification>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QuestionGeneration {
    pub learning: Option<PromptEntry>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Classification {
    pub category: Option<PromptEntry>,
    pub tags: Option<PromptEntry>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PromptEntry {
    pub template: Option<String>,
}

pub struct LoadedPrompts {
    pub questions_template: String,
    pub category_template: String,
    pub tags_template: String,
}

pub fn load_prompts() -> LoadedPrompts {
    load_prompts_from(None)
}

pub fn load_prompts_from(vault_root: Option<&Path>) -> LoadedPrompts {
    let mut search_paths: Vec<PathBuf> = Vec::new();
    if let Some(root) = vault_root {
        search_paths.push(root.join("prompts/prompt-library.yaml"));
    }
    search_paths.push(PathBuf::from("prompts/prompt-library.yaml"));
    search_paths.push(PathBuf::from("../prompts/prompt-library.yaml"));

    let path = search_paths.iter().find(|p| p.exists());

    if let Some(path) = path {
        info!("Found prompt library at: {}", path.display());
        if let Some(lib) = fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_yaml::from_str::<PromptLibrary>(&s).ok())
        {
            return LoadedPrompts {
                questions_template: lib
                    .prompts
                    .as_ref()
                    .and_then(|p| p.question_generation.as_ref())
                    .and_then(|q| q.learning.as_ref())
                    .and_then(|e| e.template.clone())
                    .unwrap_or_else(default_questions),
                category_template: lib
                    .prompts
                    .as_ref()
                    .and_then(|p| p.classification.as_ref())
                    .and_then(|c| c.category.as_ref())
                    .and_then(|e| e.template.clone())
                    .unwrap_or_else(default_category),
                tags_template: lib
                    .prompts
                    .as_ref()
                    .and_then(|p| p.classification.as_ref())
                    .and_then(|c| c.tags.as_ref())
                    .and_then(|e| e.template.clone())
                    .unwrap_or_else(default_tags),
            };
        }
        warn!("Failed to parse prompt library from {}", path.display());
    }

    LoadedPrompts {
        questions_template: default_questions(),
        category_template: default_category(),
        tags_template: default_tags(),
    }
}

fn default_questions() -> String {
    "Generate {count} learning questions about this text as a JSON array of strings:\n{content}"
        .into()
}

fn default_category() -> String {
    r#"Classify this text into PARA + Zettelkasten candidates.
Follow the ontology:
- Project: Active endeavors with a goal.
- Area: Ongoing responsibilities.
- Resource: Reference material or interests.
- ConceptSeed: Atomic ideas for Zettelkasten.

Identify related project names and existing concept links if applicable.

Output valid JSON only:
{
  "candidate_type": "Project | Area | Resource | ConceptSeed",
  "candidate_project": ["ProjectName"],
  "candidate_area": ["AreaName"],
  "candidate_concepts": ["ConceptLink"],
  "recommended_action": "move | link_existing | promote_to_zettel",
  "reasoning": "Why this classification?",
  "subcategory": "Technical | Reference | fleeting | literature | permanent",
  "detail": "Articles-Papers | Books-Notes | Tutorials-Guides | Cheat-Sheets"
}

Subcategory is required for Resources and ConceptSeed.
Detail is only required for Resources/Reference.

Text:
{content}"#
        .into()
}

fn default_tags() -> String {
    "Analyze the following text and output {min_tags}-{max_tags} relevant tags as a JSON array.\nExisting tags: {existing_tags}\nText:\n{content}".into()
}
