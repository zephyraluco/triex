use crate::search::index::{IndexedItem, SearchResultItem};

const SCORE_EXACT: f64 = 1050.0;
const SCORE_PREFIX: f64 = 1000.0;
const SCORE_CONTAINS: f64 = 100.0;

const FIELD_OFFSET_NAME: f64 = 0.0;
const FIELD_OFFSET_PINYIN_ABBR: f64 = -5.0;
const FIELD_OFFSET_PINYIN_FULL: f64 = -10.0;
const FIELD_OFFSET_KEYWORD: f64 = -20.0;

const PARTIAL_PENALTY: f64 = 30.0;
const COMPACT_PENALTY: f64 = 10.0;
const POSITION_PENALTY: f64 = 2.0;
const LENGTH_PENALTY: f64 = 1.0;

struct CandidateHit {
    score: f64,
    match_indices: Vec<usize>,
}

/// Evaluates a single indexed item using continuous substring matching across name,
/// whitespace-stripped name, pinyin initials/acronyms, full pinyin (syllable-boundary aligned),
/// and keywords/aliases.
pub fn score_item(
    query_lower: &str,
    query_compact: &str,
    item: &IndexedItem,
) -> Option<SearchResultItem> {
    let mut best: Option<CandidateHit> = None;
    let source_len = item.name_char_len as f64;

    // 1. Direct continuous substring match on lowercased name
    if let Some((start_char, end_char, total_chars)) =
        find_best_substring(&item.name_lower, query_lower, None)
    {
        let shape = match_shape(start_char, end_char, total_chars);
        let score = shape + FIELD_OFFSET_NAME
            - (start_char as f64) * POSITION_PENALTY
            - source_len * LENGTH_PENALTY;
        let mapped: Vec<usize> = (start_char..end_char).collect();
        update_best(&mut best, score, mapped);
    }

    // 1b. Whitespace-stripped continuous match on name (e.g. "vscode" or "visualstudio" -> "Visual Studio Code")
    if !query_compact.is_empty()
        && item.name_compact != item.name_lower
        && let Some((start_char, end_char, total_chars)) =
            find_best_substring(&item.name_compact, query_compact, None)
    {
        let shape = match_shape(start_char, end_char, total_chars);
        let orig_pos = item
            .name_compact_char_map
            .get(start_char)
            .copied()
            .unwrap_or(start_char);
        let score = shape + FIELD_OFFSET_NAME - COMPACT_PENALTY
            - (orig_pos as f64) * POSITION_PENALTY
            - source_len * LENGTH_PENALTY;
        let mapped = map_char_range(start_char, end_char, &item.name_compact_char_map);
        update_best(&mut best, score, mapped);
    }

    // 2. Pinyin initials / English acronym continuous match (e.g. "wx" -> "微信", "vsc" -> "Visual Studio Code")
    if !query_compact.is_empty()
        && !item.pinyin.abbr.is_empty()
        && let Some((start_char, end_char, total_chars)) =
            find_best_substring(&item.pinyin.abbr, query_compact, None)
    {
        let shape = match_shape(start_char, end_char, total_chars);
        let orig_pos = item
            .pinyin
            .abbr_char_map
            .get(start_char)
            .copied()
            .unwrap_or(start_char);
        let score = shape + FIELD_OFFSET_PINYIN_ABBR
            - (orig_pos as f64) * POSITION_PENALTY
            - source_len * LENGTH_PENALTY;
        let mapped = map_char_range(start_char, end_char, &item.pinyin.abbr_char_map);
        update_best(&mut best, score, mapped);
    }

    // 3. Full pinyin continuous match aligned to syllable/character start boundaries (e.g. "weixin" -> "微信")
    if !query_compact.is_empty()
        && !item.pinyin.full.is_empty()
        && let Some((start_char, end_char, total_chars)) = find_best_substring(
            &item.pinyin.full,
            query_compact,
            Some(&item.pinyin.full_is_start),
        )
    {
        let partial = !item
            .pinyin
            .full_is_end
            .get(end_char.saturating_sub(1))
            .copied()
            .unwrap_or(true);
        let shape = if start_char == 0 && end_char == total_chars && !partial {
            SCORE_EXACT
        } else if start_char == 0 {
            SCORE_PREFIX
        } else {
            SCORE_CONTAINS
        };
        let orig_pos = item
            .pinyin
            .full_char_map
            .get(start_char)
            .copied()
            .unwrap_or(start_char);
        let score = shape + FIELD_OFFSET_PINYIN_FULL
            - if partial { PARTIAL_PENALTY } else { 0.0 }
            - (orig_pos as f64) * POSITION_PENALTY
            - source_len * LENGTH_PENALTY;
        let mapped = map_char_range(start_char, end_char, &item.pinyin.full_char_map);
        update_best(&mut best, score, mapped);
    }

    // 4. Keyword / alias continuous match (e.g. "wechat", "cmd", "calc")
    for keyword in &item.keywords_lower {
        if let Some((start_char, end_char, total_chars)) =
            find_best_substring(keyword, query_lower, None)
        {
            let shape = match_shape(start_char, end_char, total_chars);
            let score = shape + FIELD_OFFSET_KEYWORD
                - (start_char as f64) * POSITION_PENALTY
                - source_len * LENGTH_PENALTY;
            update_best(&mut best, score, Vec::new());
        }
    }

    best.map(|hit| SearchResultItem {
        id: item.raw.id.clone(),
        kind: item.raw.kind,
        name: item.raw.name.clone(),
        subtitle: item.raw.subtitle.clone(),
        exec_path: item.raw.exec_path.clone(),
        icon_key: item.raw.icon_key.clone(),
        score: hit.score + item.frecency,
        match_indices: hit.match_indices,
    })
}

fn match_shape(start_char: usize, end_char: usize, total_chars: usize) -> f64 {
    if start_char == 0 && end_char == total_chars {
        SCORE_EXACT
    } else if start_char == 0 {
        SCORE_PREFIX
    } else {
        SCORE_CONTAINS
    }
}

/// Finds the earliest valid continuous substring match in `haystack`, returning
/// `(start_char_idx, end_char_idx, total_haystack_chars)`.
/// When `start_boundaries` is provided, only matches whose start character aligns to a boundary are accepted.
fn find_best_substring(
    haystack: &str,
    needle: &str,
    start_boundaries: Option<&[bool]>,
) -> Option<(usize, usize, usize)> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }

    let needle_chars = needle.chars().count();
    let total_chars = haystack.chars().count();
    let mut search_from = 0usize;

    while let Some(rel_byte) = haystack[search_from..].find(needle) {
        let abs_byte = search_from + rel_byte;
        let start_char = haystack[..abs_byte].chars().count();
        let valid_start = match start_boundaries {
            Some(flags) => flags.get(start_char).copied().unwrap_or(false),
            None => true,
        };
        if valid_start {
            return Some((start_char, start_char + needle_chars, total_chars));
        }

        // Advance past the current character to continue searching for a boundary-aligned hit.
        let next_step = haystack[abs_byte..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        search_from = abs_byte + next_step;
        if search_from >= haystack.len() {
            break;
        }
    }

    None
}

fn map_char_range(start: usize, end: usize, char_map: &[usize]) -> Vec<usize> {
    let mut mapped = Vec::with_capacity(end.saturating_sub(start));
    for idx in start..end {
        if let Some(&orig) = char_map.get(idx)
            && mapped.last().copied() != Some(orig)
        {
            mapped.push(orig);
        }
    }
    mapped
}

fn update_best(best: &mut Option<CandidateHit>, score: f64, match_indices: Vec<usize>) {
    let replace = match best {
        None => true,
        Some(existing) => score > existing.score,
    };
    if replace {
        *best = Some(CandidateHit {
            score,
            match_indices,
        });
    }
}
