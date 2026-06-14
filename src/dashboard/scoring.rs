use crate::dashboard::models::NoteCard;

/// Compute vitality score (1-5) from note metadata.
///
/// Weights:
///   Recency:     30% — recently modified notes are active
///   Connectivity: 25% — linked notes are structurally important
///   Richness:    20% — cross-cutting tags connect domains
///   Substance:   15% — longer notes are more developed
///   Hub bonus:   10% — top hub notes get a boost
pub fn compute_vitality(note: &NoteCard) -> u8 {
    let recency = recency_score(&note.modified_at);
    let connectivity = connectivity_score(note.incoming_links + note.outgoing_links);
    let richness = richness_score(note.tags.len());
    let substance = substance_score(note.word_count);
    let hub = if note.incoming_links >= 3 { 1.0 } else { 0.0 };

    let raw =
        recency * 0.30 + connectivity * 0.25 + richness * 0.20 + substance * 0.15 + hub * 0.10;

    // Clamp to 1-5
    let score = (raw * 5.0).round() as u8;
    score.clamp(1, 5)
}

/// Recency: 1.0 if modified today, decays to 0.0 over 90 days.
fn recency_score(modified_at: &str) -> f64 {
    let days = days_since(modified_at);
    (1.0 - (days as f64 / 90.0).min(1.0)).max(0.0)
}

/// Connectivity: 1.0 at 10+ links, scales linearly.
fn connectivity_score(link_count: usize) -> f64 {
    ((link_count as f64) / 10.0).min(1.0)
}

/// Richness: 1.0 at 7 tags (vault convention max), scales linearly.
fn richness_score(tag_count: usize) -> f64 {
    ((tag_count as f64) / 7.0).min(1.0)
}

/// Substance: 1.0 at 1000+ words, scales linearly.
fn substance_score(word_count: u32) -> f64 {
    ((word_count as f64) / 1000.0).min(1.0)
}

/// Parse ISO 8601 date and return days since. Falls back to large number on parse failure.
fn days_since(iso_date: &str) -> u32 {
    // Handle common formats: "2026-05-29T12:00:00+09:00" or "2026-05-29"
    let date_str = iso_date.split('T').next().unwrap_or(iso_date);
    let parts: Vec<u32> = date_str.split('-').filter_map(|p| p.parse().ok()).collect();

    if parts.len() != 3 {
        return 90; // fallback: treat as stale
    }

    // Simple days-since calculation (good enough for scoring)
    let (year, month, day) = (parts[0], parts[1], parts[2]);
    let now = chrono::Local::now();
    let note_date = chrono::NaiveDate::from_ymd_opt(year as i32, month, day)
        .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    let today = now.date_naive();
    (today - note_date).num_days().max(0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card(days_ago: u32, links: usize, tags: usize, words: u32) -> NoteCard {
        let date = chrono::Local::now()
            .date_naive()
            .checked_sub_signed(chrono::TimeDelta::days(days_ago as i64))
            .unwrap()
            .to_string();
        let mut t = vec!["topics/test".to_string(); tags];
        t.truncate(tags);
        NoteCard {
            path: "test.md".to_string(),
            title: "Test".to_string(),
            summary: None,
            tags: (0..tags).map(|i| format!("tag{}", i)).collect(),
            vitality: 0,
            zone: crate::dashboard::models::Zone::Inbox,
            layer: None,
            word_count: words,
            modified_at: date,
            incoming_links: links,
            outgoing_links: 0,
            has_mermaid: false,
            is_orphan: false,
        }
    }

    #[test]
    fn test_fresh_well_linked_scores_high() {
        let card = make_card(0, 10, 5, 1000);
        let score = compute_vitality(&card);
        assert!(score >= 4, "Expected high vitality, got {}", score);
    }

    #[test]
    fn test_stale_unlinked_scores_low() {
        let card = make_card(90, 0, 0, 50);
        let score = compute_vitality(&card);
        assert!(score <= 2, "Expected low vitality, got {}", score);
    }

    #[test]
    fn test_hub_bonus() {
        let card = make_card(0, 3, 0, 100);
        let score = compute_vitality(&card);
        assert!(score >= 2, "Hub bonus should boost score, got {}", score);
    }
}
