use serde::{Deserialize, Serialize};

use crate::search::pinyin_index::PinyinProjection;

/// Category of an indexed launcher entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    App,
    UwpApp,
    SystemCmd,
    Plugin,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::UwpApp => "uwpApp",
            Self::SystemCmd => "systemCmd",
            Self::Plugin => "plugin",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "uwpApp" => Self::UwpApp,
            "systemCmd" => Self::SystemCmd,
            "plugin" => Self::Plugin,
            _ => Self::App,
        }
    }
}

/// Raw entry produced by scanners or loaded from SQLite cache before pinyin precomputation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSearchItem {
    pub id: String,
    pub kind: ItemKind,
    pub name: String,
    pub subtitle: String,
    pub exec_path: String,
    pub exec_args: String,
    pub working_dir: String,
    pub icon_key: String,
    pub keywords: Vec<String>,
}

/// Indexed entry held in memory for sub-millisecond continuous substring search.
#[derive(Debug, Clone)]
pub struct IndexedItem {
    pub raw: RawSearchItem,
    pub name_lower: String,
    pub name_char_len: usize,
    /// Lowercased `name` with whitespace stripped, paired with original character index map.
    pub name_compact: String,
    pub name_compact_char_map: Vec<usize>,
    pub keywords_lower: Vec<String>,
    pub pinyin: PinyinProjection,
    pub frecency: f64,
}

impl IndexedItem {
    pub fn from_raw(raw: RawSearchItem, frecency: f64) -> Self {
        let name_lower = raw.name.to_lowercase();
        let name_char_len = raw.name.chars().count();
        let mut name_compact = String::with_capacity(name_lower.len());
        let mut name_compact_char_map = Vec::with_capacity(name_char_len);
        for (orig_idx, ch) in raw.name.chars().enumerate() {
            if !ch.is_whitespace() {
                for lower_ch in ch.to_lowercase() {
                    name_compact.push(lower_ch);
                    name_compact_char_map.push(orig_idx);
                }
            }
        }
        let keywords_lower = raw.keywords.iter().map(|k| k.to_lowercase()).collect();
        let pinyin = PinyinProjection::build(&raw.name);
        Self {
            raw,
            name_lower,
            name_char_len,
            name_compact,
            name_compact_char_map,
            keywords_lower,
            pinyin,
            frecency,
        }
    }
}

/// Search hit serialized across IPC to the React + cmdk frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub id: String,
    pub kind: ItemKind,
    pub name: String,
    pub subtitle: String,
    pub exec_path: String,
    pub icon_key: String,
    pub score: f64,
    /// Original character indices in `name` to highlight in the UI.
    pub match_indices: Vec<usize>,
}
