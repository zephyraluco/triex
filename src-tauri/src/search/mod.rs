pub mod frecency;
pub mod index;
pub mod matcher;
pub mod pinyin_index;

use std::collections::HashMap;

use crate::search::frecency::compute_frecency;
use crate::search::index::{IndexedItem, RawSearchItem, SearchResultItem};
use crate::search::matcher::{build_pattern, create_matcher, score_item};
use crate::storage::apps_repo::UsageRecord;
use crate::storage::now_unix_secs;

/// In-memory fuzzy search engine combining nucleo-matcher, Chinese pinyin projections,
/// and frecency ranking.
pub struct SearchEngine {
    items: Vec<IndexedItem>,
    item_positions: HashMap<String, usize>,
    matcher: nucleo_matcher::Matcher,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            item_positions: HashMap::new(),
            matcher: create_matcher(),
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Replaces the entire in-memory index while preserving frecency scores from usage history.
    pub fn rebuild(
        &mut self,
        raw_items: Vec<RawSearchItem>,
        usage_map: &HashMap<String, UsageRecord>,
        excluded_apps: &[String],
    ) {
        let now = now_unix_secs();
        let excluded_lower: Vec<String> = excluded_apps
            .iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        let mut items = Vec::with_capacity(raw_items.len());
        let mut item_positions = HashMap::with_capacity(raw_items.len());

        for raw in raw_items {
            let name_lower = raw.name.to_lowercase();
            if excluded_lower
                .iter()
                .any(|ex| name_lower == *ex || raw.id.to_lowercase() == *ex)
            {
                continue;
            }
            if item_positions.contains_key(&raw.id) {
                continue;
            }
            let frecency = usage_map
                .get(&raw.id)
                .map(|rec| compute_frecency(*rec, now))
                .unwrap_or(0.0);
            item_positions.insert(raw.id.clone(), items.len());
            items.push(IndexedItem::from_raw(raw, frecency));
        }

        self.items = items;
        self.item_positions = item_positions;
    }

    /// Updates frecency score for a single item immediately after it is launched.
    pub fn update_item_usage(&mut self, item_id: &str, record: UsageRecord) {
        if let Some(&idx) = self.item_positions.get(item_id)
            && let Some(item) = self.items.get_mut(idx)
        {
            item.frecency = compute_frecency(record, now_unix_secs());
        }
    }

    pub fn get_raw_item(&self, item_id: &str) -> Option<&RawSearchItem> {
        self.item_positions
            .get(item_id)
            .and_then(|&idx| self.items.get(idx))
            .map(|item| &item.raw)
    }

    /// Searches indexed items and returns at most `limit` top-ranked hits.
    /// When `query` is blank, returns pinned items first, then frequently/recently used items.
    pub fn search(
        &mut self,
        query: &str,
        limit: usize,
        pinned_ids: &[String],
    ) -> Vec<SearchResultItem> {
        let trimmed = query.trim();
        let cap = limit.clamp(1, 500);

        if trimmed.is_empty() {
            let mut defaults: Vec<&IndexedItem> = self.items.iter().collect();
            defaults.sort_by(|a, b| {
                let a_pinned = pinned_ids.iter().any(|id| id == &a.raw.id);
                let b_pinned = pinned_ids.iter().any(|id| id == &b.raw.id);
                b_pinned
                    .cmp(&a_pinned)
                    .then_with(|| b.frecency.total_cmp(&a.frecency))
                    .then_with(|| a.name_lower.cmp(&b.name_lower))
            });
            return defaults
                .into_iter()
                .take(cap)
                .map(|item| SearchResultItem {
                    id: item.raw.id.clone(),
                    kind: item.raw.kind,
                    name: item.raw.name.clone(),
                    subtitle: item.raw.subtitle.clone(),
                    exec_path: item.raw.exec_path.clone(),
                    icon_key: item.raw.icon_key.clone(),
                    score: item.frecency,
                    match_indices: Vec::new(),
                })
                .collect();
        }

        let query_lower = trimmed.to_lowercase();
        let pattern = build_pattern(trimmed);
        let mut utf32_buf = Vec::with_capacity(64);
        let mut indices_buf = Vec::with_capacity(32);

        let mut results: Vec<SearchResultItem> = self
            .items
            .iter()
            .filter_map(|item| {
                score_item(
                    &mut self.matcher,
                    &pattern,
                    &query_lower,
                    item,
                    &mut utf32_buf,
                    &mut indices_buf,
                )
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.name.len().cmp(&b.name.len()))
                .then_with(|| a.name.cmp(&b.name))
        });
        results.truncate(cap);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::index::ItemKind;

    fn sample_items() -> Vec<RawSearchItem> {
        vec![
            RawSearchItem {
                id: "app:wechat".into(),
                kind: ItemKind::App,
                name: "微信".into(),
                subtitle: "C:\\Program Files\\WeChat\\WeChat.exe".into(),
                exec_path: "C:\\Program Files\\WeChat\\WeChat.exe".into(),
                exec_args: String::new(),
                working_dir: String::new(),
                icon_key: "wechat".into(),
                keywords: vec!["wechat".into()],
            },
            RawSearchItem {
                id: "app:vscode".into(),
                kind: ItemKind::App,
                name: "Visual Studio Code".into(),
                subtitle: "C:\\Users\\zeal\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe"
                    .into(),
                exec_path: "Code.exe".into(),
                exec_args: String::new(),
                working_dir: String::new(),
                icon_key: "vscode".into(),
                keywords: vec!["vscode".into(), "code".into()],
            },
            RawSearchItem {
                id: "app:calc".into(),
                kind: ItemKind::UwpApp,
                name: "计算器".into(),
                subtitle: "Windows UWP App".into(),
                exec_path: "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App".into(),
                exec_args: String::new(),
                working_dir: String::new(),
                icon_key: "calc".into(),
                keywords: vec!["calculator".into(), "calc".into()],
            },
        ]
    }

    #[test]
    fn search_should_match_chinese_pinyin_initials_and_highlight_original_chars() {
        let mut engine = SearchEngine::new();
        engine.rebuild(sample_items(), &HashMap::new(), &[]);

        let hits = engine.search("wx", 10, &[]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "app:wechat");
        assert_eq!(hits[0].match_indices, vec![0, 1]);
    }

    #[test]
    fn search_should_match_chinese_full_pinyin() {
        let mut engine = SearchEngine::new();
        engine.rebuild(sample_items(), &HashMap::new(), &[]);

        let hits = engine.search("jisuanqi", 10, &[]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "app:calc");
        assert_eq!(hits[0].match_indices, vec![0, 1, 2]);
    }

    #[test]
    fn search_should_match_english_acronym() {
        let mut engine = SearchEngine::new();
        engine.rebuild(sample_items(), &HashMap::new(), &[]);

        let hits = engine.search("vsc", 10, &[]);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].id, "app:vscode");
    }

    #[test]
    fn search_should_promote_frequently_launched_item_when_empty_query() {
        let mut engine = SearchEngine::new();
        let mut usage = HashMap::new();
        usage.insert(
            "app:calc".to_string(),
            UsageRecord {
                launch_count: 15,
                last_used_at: now_unix_secs(),
            },
        );
        engine.rebuild(sample_items(), &usage, &[]);

        let hits = engine.search("", 10, &[]);
        assert_eq!(hits[0].id, "app:calc");
    }
}
