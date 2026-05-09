use anyhow::Result;
use rayon::prelude::*;
use std::{fs, path::Path};
use tracing::{debug, info};

use crate::moc::replace_section;

use super::scan::ProjectProfile;

pub fn inject_backlinks(
    vault_root: &Path,
    profiles: &[ProjectProfile],
    zk_dir: &str,
) -> Result<()> {
    let zk_path = vault_root.join(zk_dir);

    let zk_files: Vec<(String, String)> = match fs::read_dir(&zk_path) {
        Ok(entries) => entries
            .flat_map(|e| e.ok())
            .filter_map(|entry| {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) != Some("md") {
                    return None;
                }
                let stem = p.file_stem().and_then(|s| s.to_str())?.to_string();
                let name_lower = stem.replace('-', " ").to_lowercase();
                Some((stem, name_lower))
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    profiles.par_iter().for_each(|profile| {
        let hub_link = format!("[[{}/{}]]", profile.name, profile.name);

        profile.docs.iter().for_each(|doc_path| {
            let content = match fs::read_to_string(doc_path) {
                Ok(c) => c,
                Err(_) => return,
            };

            if content.lines().count() < 3 {
                return;
            }

            let content_lower = content.to_lowercase();
            let mut related: Vec<String> = Vec::new();

            for (stem, name_lower) in &zk_files {
                let words: Vec<&str> = name_lower.split_whitespace().collect();
                if words.len() >= 2 && words.iter().all(|w| content_lower.contains(w)) {
                    related.push(format!("- [[{}/{}]]", zk_dir, stem));
                }
            }

            let mut see_also = format!("## See Also\n- {}", hub_link);
            for link in &related {
                see_also.push('\n');
                see_also.push_str(link);
            }
            see_also.push('\n');

            let new_content = if content.contains("## See Also") {
                replace_section(&content, "## See Also", &see_also)
            } else {
                format!("{}\n\n{}", content.trim_end(), see_also)
            };

            if content != new_content {
                let _ = fs::write(doc_path, &new_content);
                debug!("Backlink injected: {}", doc_path.display());
            }
        });
    });

    info!("Backlink injection complete");
    Ok(())
}
